use crate::models::{IpaMetadata, ProfileSummary};
use anyhow::{Context, Result, bail, ensure};
use plist::Value;
use std::{
    collections::HashSet,
    fs::File,
    io::{Cursor, Read, Write},
    path::Path,
};

pub const MAX_IPA_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_UNPACKED: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ENTRY: u64 = 512 * 1024 * 1024;
const MAX_PLIST: u64 = 2 * 1024 * 1024;

fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && !name.starts_with('/') && !name.contains(['\\', ':', '\0']),
        "IPA zawiera niebezpieczną ścieżkę."
    );
    for part in name.trim_end_matches('/').split('/') {
        ensure!(
            !part.is_empty() && part != "." && part != ".." && !part.ends_with([' ', '.']),
            "IPA zawiera niebezpieczną ścieżkę."
        );
        let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let numbered = base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9');
        ensure!(
            !numbered && !["CON", "PRN", "AUX", "NUL"].contains(&base.as_str()),
            "IPA zawiera nazwę zastrzeżoną w Windows."
        );
    }
    Ok(())
}

fn read_entry(archive: &mut zip::ZipArchive<File>, name: &str, limit: u64) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .context("Brakuje wymaganego pliku w IPA.")?;
    ensure!(
        entry.size() <= limit,
        "Plik w IPA przekracza dozwolony rozmiar."
    );
    let mut bytes = Vec::new();
    (&mut entry).take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "Plik w IPA przekracza dozwolony rozmiar."
    );
    Ok(bytes)
}

fn text(dict: &plist::Dictionary, key: &str) -> Option<String> {
    dict.get(key)?.as_string().map(str::to_owned)
}

fn profile_summary(bytes: &[u8]) -> Option<ProfileSummary> {
    // This extracts display metadata only. It does not authenticate the CMS signature.
    let start = bytes.windows(5).position(|part| part == b"<?xml")?;
    let end = bytes[start..]
        .windows(8)
        .position(|part| part == b"</plist>")?
        + start
        + 8;
    let value = Value::from_reader(Cursor::new(&bytes[start..end])).ok()?;
    let dict = value.as_dictionary()?;
    let expiration: Option<std::time::SystemTime> = dict
        .get("ExpirationDate")
        .and_then(Value::as_date)
        .map(Into::into);
    let expires_at =
        expiration.map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
    let expired = expiration.is_some_and(|time| time <= std::time::SystemTime::now());
    Some(ProfileSummary {
        name: text(dict, "Name").unwrap_or_else(|| "Profil aplikacji".into()),
        expires_at,
        expired,
        team_id: dict
            .get("TeamIdentifier")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_string)
            .map(str::to_owned),
        metadata_only: true,
    })
}

fn word(bytes: &[u8], at: usize, little: bool) -> Result<u32> {
    let value: [u8; 4] = bytes
        .get(at..at + 4)
        .context("Niepełny nagłówek Mach-O.")?
        .try_into()?;
    Ok(if little {
        u32::from_le_bytes(value)
    } else {
        u32::from_be_bytes(value)
    })
}

fn check_macho(bytes: &[u8], depth: usize) -> Result<()> {
    ensure!(depth <= 1, "Nieprawidłowy kontener Mach-O.");
    let magic = bytes.get(..4).context("Brakuje nagłówka aplikacji.")?;
    if [
        b"\xca\xfe\xba\xbe".as_slice(),
        b"\xca\xfe\xba\xbf".as_slice(),
    ]
    .contains(&magic)
    {
        let count = word(bytes, 4, false)? as usize;
        ensure!(count > 0 && count <= 32, "Nieprawidłowy nagłówek FAT.");
        let stride = if magic[3] == 0xbf { 32 } else { 20 };
        for index in 0..count {
            let at = 8 + index * stride + 8;
            let offset = if stride == 32 {
                u64::from_be_bytes(bytes.get(at..at + 8).context("Niepełny FAT.")?.try_into()?)
            } else {
                word(bytes, at, false)? as u64
            };
            let offset = usize::try_from(offset)?;
            check_macho(
                bytes.get(offset..).context("Nieprawidłowy offset FAT.")?,
                depth + 1,
            )?;
        }
        return Ok(());
    }
    let (little, is64) = match magic {
        [0xcf, 0xfa, 0xed, 0xfe] => (true, true),
        [0xce, 0xfa, 0xed, 0xfe] => (true, false),
        [0xfe, 0xed, 0xfa, 0xcf] => (false, true),
        [0xfe, 0xed, 0xfa, 0xce] => (false, false),
        _ => bail!("Plik wykonywalny nie jest obsługiwanym Mach-O."),
    };
    let count = word(bytes, 16, little)? as usize;
    let commands_size = word(bytes, 20, little)? as usize;
    let mut offset = if is64 { 32 } else { 28 };
    let end = offset + commands_size;
    ensure!(
        count <= 4096 && end <= bytes.len(),
        "Nieprawidłowe komendy Mach-O."
    );
    for _ in 0..count {
        let command = word(bytes, offset, little)?;
        let size = word(bytes, offset + 4, little)? as usize;
        ensure!(
            size >= 8 && offset + size <= end,
            "Nieprawidłowe komendy Mach-O."
        );
        if command == 0x21 || command == 0x2c {
            ensure!(
                size >= 20 && word(bytes, offset + 16, little)? == 0,
                "IPA jest zaszyfrowana (DRM) i nie jest obsługiwana."
            );
        }
        offset += size;
    }
    ensure!(offset == end, "Nieprawidłowy rozmiar komend Mach-O.");
    Ok(())
}

pub fn inspect(path: &Path) -> Result<IpaMetadata> {
    ensure!(
        path.metadata()?.len() <= MAX_IPA_BYTES,
        "IPA przekracza limit 2 GiB."
    );
    let mut archive =
        zip::ZipArchive::new(File::open(path)?).context("Nieprawidłowe archiwum IPA.")?;
    ensure!(archive.len() <= 50_000, "IPA zawiera zbyt wiele plików.");
    let mut names = HashSet::new();
    let mut apps = HashSet::new();
    let mut unpacked = 0u64;
    let mut extensions = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        validate_name(entry.name())?;
        ensure!(
            names.insert(entry.name().trim_end_matches('/').to_lowercase()),
            "IPA zawiera powtarzające się ścieżki."
        );
        let kind = entry.unix_mode().unwrap_or(0) & 0o170000;
        ensure!(
            [0, 0o040000, 0o100000].contains(&kind),
            "IPA zawiera dowiązanie lub plik specjalny."
        );
        ensure!(
            entry.size() <= MAX_ENTRY,
            "Pojedynczy plik IPA przekracza limit 512 MiB."
        );
        unpacked = unpacked
            .checked_add(entry.size())
            .context("Zbyt duża IPA.")?;
        ensure!(
            unpacked <= MAX_UNPACKED,
            "IPA po rozpakowaniu przekracza 4 GiB."
        );
        ensure!(
            entry.size() <= 8 * 1024 * 1024 || entry.size() / entry.compressed_size().max(1) <= 300,
            "Nadmierny stopień kompresji IPA."
        );
        let parts: Vec<_> = entry.name().split('/').collect();
        if parts.len() >= 3 && parts[0] == "Payload" && parts[1].ends_with(".app") {
            apps.insert(parts[1].to_owned());
        }
        if entry.name().ends_with(".appex/Info.plist")
            || (parts.len() > 3 && parts[2] == "Watch" && entry.name().ends_with(".app/Info.plist"))
        {
            extensions += 1;
        }
    }
    ensure!(
        apps.len() == 1,
        "IPA musi zawierać dokładnie jedną aplikację Payload/*.app."
    );
    let app_directory = format!("Payload/{}", apps.into_iter().next().unwrap());
    let info = Value::from_reader(Cursor::new(read_entry(
        &mut archive,
        &format!("{app_directory}/Info.plist"),
        MAX_PLIST,
    )?))
    .context("Nieprawidłowy Info.plist.")?;
    let dict = info
        .as_dictionary()
        .context("Info.plist nie jest słownikiem.")?;
    let bundle_id = text(dict, "CFBundleIdentifier").context("Brak identyfikatora aplikacji.")?;
    ensure!(
        !bundle_id.is_empty()
            && bundle_id.len() <= 255
            && bundle_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)),
        "Nieprawidłowy identyfikator aplikacji."
    );
    let executable = text(dict, "CFBundleExecutable").context("Brak pliku wykonywalnego.")?;
    validate_name(&executable)?;
    ensure!(
        !executable.contains('/'),
        "Nieprawidłowa nazwa pliku wykonywalnego."
    );
    if let Some(platforms) = dict
        .get("CFBundleSupportedPlatforms")
        .and_then(Value::as_array)
    {
        ensure!(
            platforms.iter().any(|v| v.as_string() == Some("iPhoneOS")),
            "IPA nie jest przeznaczona na fizycznego iPhone’a."
        );
    }
    check_macho(
        &read_entry(
            &mut archive,
            &format!("{app_directory}/{executable}"),
            MAX_ENTRY,
        )?,
        0,
    )?;
    let profile = if archive
        .index_for_name(&format!("{app_directory}/embedded.mobileprovision"))
        .is_some()
    {
        profile_summary(&read_entry(
            &mut archive,
            &format!("{app_directory}/embedded.mobileprovision"),
            MAX_PLIST,
        )?)
    } else {
        None
    };
    Ok(IpaMetadata {
        name: text(dict, "CFBundleDisplayName")
            .or_else(|| text(dict, "CFBundleName"))
            .unwrap_or_else(|| bundle_id.clone()),
        bundle_id,
        version: text(dict, "CFBundleShortVersionString").unwrap_or_else(|| "?".into()),
        build: text(dict, "CFBundleVersion").context("Brak numeru buildu aplikacji.")?,
        minimum_ios: text(dict, "MinimumOSVersion"),
        has_signature_files: archive
            .index_for_name(&format!("{app_directory}/_CodeSignature/CodeResources"))
            .is_some(),
        app_directory,
        profile,
        extensions,
        unpacked_bytes: unpacked,
    })
}

pub fn extract_app(
    path: &Path,
    destination: &Path,
    metadata: &IpaMetadata,
) -> Result<std::path::PathBuf> {
    // Revalidate immediately before extraction; do not use the signing library's unchecked ZIP extractor.
    let checked = inspect(path)?;
    ensure!(
        checked.bundle_id == metadata.bundle_id && checked.build == metadata.build,
        "Zmieniono pakiet IPA."
    );
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    let mut written = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if !entry
            .name()
            .starts_with(&format!("{}/", metadata.app_directory))
        {
            continue;
        }
        let output = destination.join(
            entry
                .enclosed_name()
                .context("Niebezpieczna ścieżka IPA.")?,
        );
        if entry.is_dir() {
            std::fs::create_dir_all(&output)?;
            continue;
        }
        std::fs::create_dir_all(output.parent().context("Brak katalogu.")?)?;
        let mut file = File::create(output)?;
        let expected_size = entry.size();
        let limit = expected_size.min(MAX_ENTRY).min(MAX_UNPACKED - written);
        let copied = std::io::copy(&mut (&mut entry).take(limit + 1), &mut file)?;
        ensure!(
            copied <= limit && copied == expected_size,
            "Rzeczywisty rozmiar pliku nie zgadza się z nagłówkiem IPA."
        );
        written += copied;
        file.flush()?;
    }
    Ok(destination.join(&metadata.app_directory))
}

fn signing_entitlements(bytes: &[u8], depth: usize) -> Result<Vec<String>> {
    ensure!(depth <= 1, "Nieprawidłowy FAT.");
    let magic = bytes.get(..4).context("Brak nagłówka Mach-O.")?;
    if magic == b"\xca\xfe\xba\xbe" || magic == b"\xca\xfe\xba\xbf" {
        let stride = if magic[3] == 0xbf { 32 } else { 20 };
        let mut keys = vec![];
        for index in 0..word(bytes, 4, false)? as usize {
            let at = 8 + index * stride + 8;
            let offset = if stride == 32 {
                u64::from_be_bytes(bytes.get(at..at + 8).context("Niepełny FAT.")?.try_into()?)
            } else {
                word(bytes, at, false)? as u64
            };
            keys.extend(signing_entitlements(
                bytes
                    .get(usize::try_from(offset)?..)
                    .context("Nieprawidłowy FAT.")?,
                depth + 1,
            )?);
        }
        return Ok(keys);
    }
    let little = matches!(magic, [0xcf, 0xfa, 0xed, 0xfe] | [0xce, 0xfa, 0xed, 0xfe]);
    let mut offset = if magic == b"\xcf\xfa\xed\xfe" || magic == b"\xfe\xed\xfa\xcf" {
        32
    } else {
        28
    };
    for _ in 0..word(bytes, 16, little)? {
        let command = word(bytes, offset, little)?;
        let size = word(bytes, offset + 4, little)? as usize;
        if command == 0x1d {
            ensure!(size >= 16, "Nieprawidłowy opis podpisu Mach-O.");
            let start = word(bytes, offset + 8, little)? as usize;
            let length = word(bytes, offset + 12, little)? as usize;
            let signature = bytes
                .get(start..start.checked_add(length).context("Nieprawidłowy podpis.")?)
                .context("Niepełny podpis Mach-O.")?;
            if word(signature, 0, false)? != 0xfade0cc0 {
                return Ok(vec![]);
            }
            let count = word(signature, 8, false)? as usize;
            ensure!(count <= 128, "Nieprawidłowy podpis Mach-O.");
            let mut has_der = false;
            for index in 0..count {
                let slot = word(signature, 12 + index * 8, false)?;
                let start = word(signature, 16 + index * 8, false)? as usize;
                if slot == 7 {
                    has_der = true;
                }
                if slot != 5 {
                    continue;
                }
                ensure!(
                    word(signature, start, false)? == 0xfade7171,
                    "Nieprawidłowe entitlementy."
                );
                let length = word(signature, start + 4, false)? as usize;
                ensure!(
                    (8..=2 * 1024 * 1024).contains(&length),
                    "Nieprawidłowe entitlementy."
                );
                let xml = signature
                    .get(
                        start + 8
                            ..start
                                .checked_add(length)
                                .context("Nieprawidłowe entitlementy.")?,
                    )
                    .context("Niepełne entitlementy.")?;
                let value = Value::from_reader(Cursor::new(xml))?;
                return Ok(value
                    .as_dictionary()
                    .context("Nieprawidłowy słownik entitlementów.")?
                    .keys()
                    .cloned()
                    .collect());
            }
            if has_der {
                return Ok(vec!["entitlements-der-only".into()]);
            }
        }
        offset += size;
    }
    Ok(vec![])
}

pub fn check_basic_signing(app: &Path) -> Result<()> {
    let info = Value::from_file(app.join("Info.plist"))?;
    let name = info
        .as_dictionary()
        .and_then(|d| d.get("CFBundleExecutable"))
        .and_then(Value::as_string)
        .context("Brak pliku wykonywalnego.")?;
    validate_name(name)?;
    ensure!(!name.contains('/'), "Nieprawidłowy plik wykonywalny.");
    let bytes = std::fs::read(app.join(name))?;
    check_macho(&bytes, 0)?;
    let basic = [
        "application-identifier",
        "com.apple.developer.team-identifier",
        "get-task-allow",
        "keychain-access-groups",
        "beta-reports-active",
    ];
    let unsupported: Vec<_> = signing_entitlements(&bytes, 0)?
        .into_iter()
        .filter(|key| !basic.contains(&key.as_str()))
        .collect();
    ensure!(
        unsupported.is_empty(),
        "IPA wymaga dodatkowych entitlementów: {}. Ta wersja nie zmieni ich po cichu; użyj wcześniej poprawnie podpisanej IPA.",
        unsupported.join(", ")
    );
    Ok(())
}
