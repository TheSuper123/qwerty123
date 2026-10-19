use anyhow::{Context, Result, ensure};
use isideload::util::storage::SideloadingStorage;
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub struct Vault {
    root: PathBuf,
    key: Arc<aead::LessSafeKey>,
}

impl Vault {
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
        }
        let master = load_master(root)?;
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, &master)
                .map_err(|_| anyhow::anyhow!("Nie można otworzyć magazynu kluczy."))?,
        );
        Ok(Self {
            root: root.into(),
            key: Arc::new(key),
        })
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.join(hex::encode(Sha256::digest(name.as_bytes())))
    }
    fn put(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let mut nonce = [0u8; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| anyhow::anyhow!("Brak bezpiecznej losowości."))?;
        let mut encrypted = bytes.to_vec();
        self.key
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(name.as_bytes()),
                &mut encrypted,
            )
            .map_err(|_| anyhow::anyhow!("Błąd szyfrowania."))?;
        let mut output = nonce.to_vec();
        output.extend(encrypted);
        let mut temp = tempfile::NamedTempFile::new_in(&self.root)?;
        use std::io::Write;
        temp.write_all(&output)?;
        temp.as_file().sync_all()?;
        temp.persist(self.path(name))
            .map_err(|_| anyhow::anyhow!("Nie można zapisać zaszyfrowanego rekordu."))?;
        Ok(())
    }
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let path = self.path(name);
        let mut encrypted = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        ensure!(encrypted.len() >= 28, "Uszkodzony magazyn sekretów.");
        let nonce: [u8; 12] = encrypted[..12].try_into()?;
        let value = self
            .key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(name.as_bytes()),
                &mut encrypted[12..],
            )
            .map_err(|_| anyhow::anyhow!("Nie można uwierzytelnić rekordu magazynu sekretów."))?;
        Ok(Some(value.to_vec()))
    }
}

#[cfg(windows)]
fn load_master(_root: &Path) -> Result<Vec<u8>> {
    let entry = keyring::Entry::new("org.mostserver.vault", "master-key")?;
    match entry.get_secret() {
        Ok(key) => {
            ensure!(
                key.len() == 32,
                "Nieprawidłowy klucz w Windows Credential Manager."
            );
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = vec![0u8; 32];
            SystemRandom::new()
                .fill(&mut key)
                .map_err(|_| anyhow::anyhow!("Brak bezpiecznej losowości."))?;
            entry.set_secret(&key)?;
            Ok(key)
        }
        Err(_) => anyhow::bail!(
            "Windows Credential Manager nie jest dostępny. Odblokuj sesję użytkownika; Most nie zapisze klucza jawnym tekstem."
        ),
    }
}

#[cfg(not(windows))]
fn load_master(root: &Path) -> Result<Vec<u8>> {
    use std::io::Write;
    let path = root.join(".development-key");
    match std::fs::read(&path) {
        Ok(key) => {
            ensure!(key.len() == 32, "Uszkodzony klucz lokalny.");
            Ok(key)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut key = vec![0u8; 32];
            SystemRandom::new()
                .fill(&mut key)
                .map_err(|_| anyhow::anyhow!("Brak bezpiecznej losowości."))?;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(path)?.write_all(&key)?;
            Ok(key)
        }
        Err(e) => Err(e.into()),
    }
}

impl SideloadingStorage for Vault {
    fn store(&self, key: &str, value: &str) -> Result<(), rootcause::Report> {
        self.put(key, value.as_bytes())
            .map_err(|_| rootcause::report!("Nie można zapisać sekretu w magazynie systemowym."))
    }
    fn retrieve(&self, key: &str) -> Result<Option<String>, rootcause::Report> {
        self.get(key)
            .and_then(|v| {
                v.map(|bytes| String::from_utf8(bytes).context("Nieprawidłowy rekord."))
                    .transpose()
            })
            .map_err(|_| rootcause::report!("Nie można odczytać sekretu z magazynu systemowego."))
    }
    fn store_data(&self, key: &str, value: &[u8]) -> Result<(), rootcause::Report> {
        self.put(key, value)
            .map_err(|_| rootcause::report!("Nie można zapisać sekretu w magazynie systemowym."))
    }
    fn retrieve_data(&self, key: &str) -> Result<Option<Vec<u8>>, rootcause::Report> {
        self.get(key)
            .map_err(|_| rootcause::report!("Nie można odczytać sekretu z magazynu systemowego."))
    }
    fn delete(&self, key: &str) -> Result<(), rootcause::Report> {
        match std::fs::remove_file(self.path(key)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(rootcause::report!("Nie można usunąć sekretu.")),
        }
    }
}
