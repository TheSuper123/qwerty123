use crate::{device, ipa, models::Job, state::AppState};
use anyhow::{Context, Result, ensure};
use isideload::{dev::devices::DevicesApi, util::device::IdeviceInfo};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{path::Path, sync::Arc};

pub fn update(state: &AppState, job: &mut Job, stage: &str, message: &str) -> Result<()> {
    job.state = stage.into();
    job.message = message.into();
    job.updated_at = crate::models::now();
    state
        .store
        .lock()
        .map_err(|_| anyhow::anyhow!("Blokada bazy danych."))?
        .update_job(job)
}

fn assert_hash(path: &Path, expected: &str) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    ensure!(
        hex::encode(hash.finalize()) == expected,
        "Zmieniono zapisany plik IPA. Zaimportuj oryginał ponownie."
    );
    Ok(())
}

pub async fn execute(state: Arc<AppState>, job: &mut Job) -> Result<()> {
    update(
        &state,
        job,
        "validating",
        "Sprawdzanie integralności IPA i urządzenia…",
    )?;
    let (artifact, path, temp_root) = {
        let store = state
            .store
            .lock()
            .map_err(|_| anyhow::anyhow!("Blokada bazy danych."))?;
        let artifact = store.artifact(&job.artifact_id)?;
        let path = store.artifact_path(&artifact);
        (artifact, path, store.root.join("temporary"))
    };
    let expected = artifact.sha256.clone();
    let check_path = path.clone();
    tokio::task::spawn_blocking(move || assert_hash(&check_path, &expected)).await??;
    let metadata = artifact.metadata.clone();
    let extraction_path = path.clone();
    let directory = tempfile::tempdir_in(temp_root)?;
    let destination = directory.path().to_owned();
    let app = tokio::task::spawn_blocking(move || {
        ipa::extract_app(&extraction_path, &destination, &metadata)
    })
    .await??;
    let provider = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        device::provider(&job.device_id),
    )
    .await
    .context("Urządzenie nie odpowiada.")??;
    let mut engine_guard = state.engine.lock().await;
    let mut signed_cleanup = None;
    let install_path = if job.mode == "resign" {
        ipa::check_basic_signing(&app)?;
        ensure!(
            artifact.metadata.extensions == 0,
            "Podpisywanie aplikacji z rozszerzeniami nie jest jeszcze dopuszczone w tej wersji. Użyj poprawnie podpisanej IPA."
        );
        let engine = engine_guard
            .as_mut()
            .context("Najpierw zaloguj się na konto Apple.")?;
        ensure!(
            state
                .account
                .lock()
                .map_err(|_| anyhow::anyhow!("Blokada konta."))?
                .selected_team
                .is_some(),
            "Wybierz zespół na ekranie Konto Apple."
        );
        let device_info = IdeviceInfo::from_device(&provider)
            .await
            .map_err(|_| anyhow::anyhow!("Nie można otworzyć zaufanej sesji urządzenia."))?;
        update(
            &state,
            job,
            "provisioning",
            "Przygotowywanie urządzenia i profilu w Apple…",
        )?;
        let team = engine
            .get_team()
            .await
            .map_err(|e| anyhow::anyhow!("{}", crate::error::account_message(&e.to_string())))?;
        engine
            .get_dev_session()
            .ensure_device_registered(&team, &device_info.name, &device_info.udid, None)
            .await
            .map_err(|e| anyhow::anyhow!("{}", crate::error::account_message(&e.to_string())))?;
        update(
            &state,
            job,
            "signing",
            "Podpisywanie aplikacji na tym komputerze…",
        )?;
        let (signed, _) = engine
            .sign_app(
                app,
                Some(team),
                false,
                None::<fn(f32) -> std::future::Ready<()>>,
                None,
                Some(&device_info.udid),
            )
            .await
            .map_err(|e| anyhow::anyhow!("{}", crate::error::account_message(&e.to_string())))?;
        signed_cleanup = Some(signed.clone());
        signed
    } else {
        ensure!(
            artifact.metadata.has_signature_files && artifact.metadata.profile.is_some(),
            "Wybrana IPA nie zawiera plików podpisu i czytelnego profilu. Wybierz Podpisz kontem Apple."
        );
        ensure!(
            !artifact.metadata.profile.as_ref().is_some_and(|p| p.expired
                || p.expires_at
                    .as_ref()
                    .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
                    .is_some_and(|date| date <= chrono::Utc::now())),
            "Profil IPA wygasł. Podpisz aplikację ponownie."
        );
        app
    };
    let info = plist::Value::from_file(install_path.join("Info.plist"))?;
    let info = info
        .as_dictionary()
        .context("Nieprawidłowy wynik podpisywania.")?;
    let bundle = info
        .get("CFBundleIdentifier")
        .and_then(plist::Value::as_string)
        .context("Brak ID po podpisaniu.")?
        .to_owned();
    job.effective_bundle_id = Some(bundle.clone());
    update(
        &state,
        job,
        "installing",
        "Przesyłanie aplikacji i instalacja przez usługi iOS…",
    )?;
    let callback_state = state.clone();
    let job_id = job.id.clone();
    let installation =
        isideload::sideload::install::install_app(&provider, &install_path, move |percent| {
            if let Ok(store) = callback_state.store.lock()
                && let Ok(mut current) = store.job(&job_id)
                && current.progress != Some(percent)
            {
                current.progress = Some(percent.min(100));
                current.updated_at = crate::models::now();
                let _ = store.update_job(&current);
            }
        })
        .await;
    if let Some(path) = signed_cleanup {
        let _ = std::fs::remove_dir_all(path);
    }
    if installation.is_err() {
        update(
            &state,
            job,
            "needs_reconciliation",
            "Nie potwierdzono końca instalacji. Odblokuj iPhone i użyj Sprawdź wynik; nie ponawiaj w ciemno.",
        )?;
        return Ok(());
    }
    update(
        &state,
        job,
        "verifying",
        "Potwierdzanie identyfikatora i numeru buildu na iPhonie…",
    )?;
    let verification = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        device::verify(&job.device_id, &bundle, &job.expected_build),
    )
    .await;
    if matches!(verification, Ok(Ok(()))) {
        job.progress = Some(100);
        update(
            &state,
            job,
            "succeeded",
            "Instalacja potwierdzona: właściwa aplikacja i numer buildu są na urządzeniu. Uruchom ją na iPhonie.",
        )?;
    } else {
        update(
            &state,
            job,
            "needs_reconciliation",
            "iOS przyjął instalację, ale nie potwierdzono wersji. Odblokuj urządzenie i wybierz Sprawdź wynik.",
        )?;
    }
    Ok(())
}
