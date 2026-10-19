use crate::{
    account, device,
    error::ApiError,
    ipa, jobs,
    models::{Artifact, Job, now},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tokio::io::AsyncWriteExt;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/licenses.txt",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    include_str!(concat!(env!("OUT_DIR"), "/licenses.txt")),
                )
            }),
        )
        .route(
            "/",
            get(|| async { Html(include_str!("../ui/index.html")) }),
        )
        .route(
            "/app.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../ui/app.css"),
                )
            }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../ui/app.js"),
                )
            }),
        )
        .route(
            "/favicon.svg",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/svg+xml")],
                    include_str!("../../../site/favicon.svg"),
                )
            }),
        )
        .route("/api/state", get(snapshot))
        .route(
            "/api/devices",
            get(|| async { Json(device::discover().await) }),
        )
        .route("/api/devices/{id}/pair", post(pair))
        .route("/api/devices/{id}/apps", get(installed))
        .route(
            "/api/artifacts",
            post(upload).layer(DefaultBodyLimit::max(
                ipa::MAX_IPA_BYTES as usize + 64 * 1024,
            )),
        )
        .route("/api/jobs", post(create_job))
        .route("/api/jobs/{id}/reconcile", post(reconcile))
        .route("/api/account/login", post(login))
        .route("/api/account/challenge", post(challenge))
        .route("/api/account/team", post(team))
        .route("/api/account/logout", post(logout))
        .route("/api/diagnostics", get(diagnostics))
        .route("/api/companion/start", post(crate::companion::start))
        .route(
            "/api/companion/clients",
            get(crate::companion::clients).post(crate::companion::pair_client),
        )
        .route(
            "/api/companion/clients/{id}",
            axum::routing::delete(crate::companion::revoke_client),
        )
        .route("/api/shutdown", post(shutdown))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), security))
        .with_state(state)
}

async fn security(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok());
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok());
    let expected_origin = format!("http://{}", state.authority);
    let cross_site = request
        .headers()
        .get("sec-fetch-site")
        .and_then(|h| h.to_str().ok())
        == Some("cross-site");
    if host != Some(state.authority.as_str())
        || origin.is_some_and(|value| value != expected_origin)
        || cross_site
    {
        return (StatusCode::FORBIDDEN,Json(json!({"code":"origin_rejected","message":"To połączenie nie pochodzi z lokalnego panelu MostServer."}))).into_response();
    }
    if request.uri().path().starts_with("/api/") {
        let authorization = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or("");
        if authorization
            .as_bytes()
            .ct_eq(state.token.as_bytes())
            .unwrap_u8()
            != 1
        {
            return (StatusCode::UNAUTHORIZED,Json(json!({"code":"unauthorized","message":"Otwórz panel z odnośnika w oknie MostServer."}))).into_response();
        }
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("referrer-policy", "no-referrer".parse().unwrap());
    headers.insert("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'".parse().unwrap());
    response
}

async fn snapshot(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, ApiError> {
    let account = state
        .account
        .lock()
        .map_err(|_| ApiError::internal())?
        .clone();
    let store = state.store.lock().map_err(|_| ApiError::internal())?;
    let network = state
        .network
        .lock()
        .map_err(|_| ApiError::internal())?
        .clone();
    Ok(Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"busy":state.operation.try_lock().is_err(),"capabilities":{"usb":true,"wifi_device":true,"remote":false,"local_vpn":false,"apple_signing":true,"mobile_client":true},"network":network,"account":account,"artifacts":store.artifacts().map_err(|_|ApiError::internal())?,"jobs":store.jobs().map_err(|_|ApiError::internal())?}),
    ))
}

pub(crate) async fn upload(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Artifact>), ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::bad("ipa_invalid", "Nie można odczytać pliku IPA."))?
        .ok_or_else(|| ApiError::bad("ipa_required", "Wybierz plik IPA."))?;
    if field.name() != Some("ipa") {
        return Err(ApiError::bad("ipa_required", "Oczekiwano pola ipa."));
    }
    if !field
        .file_name()
        .is_some_and(|name| name.to_lowercase().ends_with(".ipa"))
    {
        return Err(ApiError::bad(
            "ipa_required",
            "Wybierz plik z rozszerzeniem .ipa.",
        ));
    }
    let root = state
        .store
        .lock()
        .map_err(|_| ApiError::internal())?
        .root
        .clone();
    let path = root
        .join("temporary")
        .join(format!("{}.upload", uuid::Uuid::new_v4()));
    let mut cleanup = UploadCleanup(Some(path.clone()));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
        .map_err(|_| ApiError::internal())?;
    let mut field = field;
    let mut size = 0u64;
    let mut hash = Sha256::new();
    while let Some(chunk) = field.chunk().await.map_err(|_| {
        ApiError::bad(
            "upload_interrupted",
            "Przerwano przesyłanie IPA. Wybierz plik ponownie.",
        )
    })? {
        size += chunk.len() as u64;
        if size > ipa::MAX_IPA_BYTES {
            return Err(ApiError {
                status: StatusCode::PAYLOAD_TOO_LARGE,
                code: "ipa_too_large",
                message: "Limit IPA wynosi 2 GiB.".into(),
            });
        }
        file.write_all(&chunk)
            .await
            .map_err(|_| ApiError::internal())?;
        hash.update(&chunk);
    }
    file.sync_all().await.map_err(|_| ApiError::internal())?;
    drop(file);
    let inspect_path = path.clone();
    let metadata = tokio::task::spawn_blocking(move || ipa::inspect(&inspect_path))
        .await
        .map_err(|_| ApiError::internal())?
        .map_err(|e| ApiError::bad("ipa_invalid", e.to_string()))?;
    let sha256 = hex::encode(hash.finalize());
    let store = state.store.lock().map_err(|_| ApiError::internal())?;
    if let Some(existing) = store.by_hash(&sha256).map_err(|_| ApiError::internal())? {
        return Ok((StatusCode::OK, Json(existing)));
    }
    let artifact = Artifact {
        id: uuid::Uuid::new_v4().to_string(),
        sha256,
        size,
        imported_at: now(),
        metadata,
    };
    std::fs::rename(&path, store.artifact_path(&artifact)).map_err(|_| ApiError::internal())?;
    cleanup.0 = None;
    store
        .add_artifact(&artifact)
        .map_err(|_| ApiError::internal())?;
    Ok((StatusCode::CREATED, Json(artifact)))
}
struct UploadCleanup(Option<std::path::PathBuf>);
impl Drop for UploadCleanup {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) struct JobRequest {
    pub artifact_id: String,
    pub device_id: String,
    pub mode: String,
}
pub(crate) async fn create_job(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<JobRequest>,
) -> Result<(StatusCode, Json<Job>), ApiError> {
    if !["presigned", "resign"].contains(&body.mode.as_str())
        || body.device_id.is_empty()
        || body.device_id.len() > 128
    {
        return Err(ApiError::bad(
            "invalid_job",
            "Wybierz urządzenie i sposób instalacji.",
        ));
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|h| h.to_str().ok())
        .filter(|h| !h.is_empty() && h.len() <= 128)
        .ok_or_else(|| {
            ApiError::bad(
                "idempotency_key_required",
                "Brakuje identyfikatora żądania.",
            )
        })?;
    let request_data = serde_json::to_string(&body).map_err(|_| ApiError::internal())?;
    {
        let store = state.store.lock().map_err(|_| ApiError::internal())?;
        if let Some(existing) = store
            .request(key, &request_data)
            .map_err(|_| ApiError::conflict("Ten identyfikator żądania ma już inne dane."))?
        {
            return Ok((StatusCode::OK, Json(existing)));
        }
    }
    let guard = state
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("Poczekaj na zakończenie bieżącej operacji."))?;
    let artifact = state
        .store
        .lock()
        .map_err(|_| ApiError::internal())?
        .artifact(&body.artifact_id)
        .map_err(|_| ApiError::bad("artifact_not_found", "Nie znaleziono wybranej IPA."))?;
    if body.mode == "resign" {
        let view = state.account.lock().map_err(|_| ApiError::internal())?;
        if view.status != "logged_in" || view.selected_team.is_none() {
            return Err(ApiError::bad(
                "apple_auth_required",
                "Zaloguj się na konto Apple i wybierz zespół.",
            ));
        }
        if artifact.metadata.extensions > 0 {
            return Err(ApiError::bad(
                "extensions_not_supported",
                "W tej wersji podpisywanie obejmuje aplikacje bez rozszerzeń. Rozszerzenia wymagają wcześniej poprawnie podpisanej IPA.",
            ));
        }
    }
    let mut job = Job {
        id: uuid::Uuid::new_v4().to_string(),
        artifact_id: body.artifact_id,
        device_id: body.device_id,
        mode: body.mode,
        state: "queued".into(),
        message: "Zadanie przyjęte.".into(),
        progress: None,
        effective_bundle_id: None,
        expected_build: artifact.metadata.build,
        created_at: now(),
        updated_at: now(),
    };
    state
        .store
        .lock()
        .map_err(|_| ApiError::internal())?
        .add_job(&job, key, &request_data)
        .map_err(|_| ApiError::internal())?;
    let response = job.clone();
    tokio::spawn(async move {
        let _guard = guard;
        let outcome = tokio::time::timeout(
            Duration::from_secs(900),
            std::panic::AssertUnwindSafe(jobs::execute(state.clone(), &mut job)).catch_unwind(),
        )
        .await;
        match outcome {
            Ok(Ok(Ok(()))) => (),
            Ok(Ok(Err(error))) => {
                let message = error.to_string();
                let _ = jobs::update(&state, &mut job, "failed", &message);
            }
            Ok(Err(_)) | Err(_) => {
                let _ = jobs::update(
                    &state,
                    &mut job,
                    "needs_reconciliation",
                    "Operacja została przerwana lub przekroczyła czas. Najpierw sprawdź stan aplikacji na urządzeniu.",
                );
            }
        }
    });
    Ok((StatusCode::ACCEPTED, Json(response)))
}

pub(crate) async fn reconcile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, ApiError> {
    let _guard = state
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("Trwa inna operacja."))?;
    let mut job = state
        .store
        .lock()
        .map_err(|_| ApiError::internal())?
        .job(&id)
        .map_err(|_| ApiError::bad("job_not_found", "Nie znaleziono zadania."))?;
    if job.state != "needs_reconciliation" {
        return Err(ApiError::conflict(
            "To zadanie nie wymaga uzgadniania wyniku.",
        ));
    }
    let Some(bundle) = job.effective_bundle_id.as_deref() else {
        return Err(ApiError::bad(
            "no_installation_started",
            "Brak potwierdzonego rozpoczęcia instalacji. Sprawdź bibliotekę aplikacji na iPhonie przed ponowieniem.",
        ));
    };
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        device::verify(&job.device_id, bundle, &job.expected_build),
    )
    .await;
    if matches!(result, Ok(Ok(()))) {
        job.progress = Some(100);
        jobs::update(
            &state,
            &mut job,
            "succeeded",
            "Właściwy bundle ID i build zostały potwierdzone na urządzeniu.",
        )
        .map_err(|_| ApiError::internal())?;
    } else {
        return Err(ApiError::device());
    }
    Ok(Json(job))
}

async fn pair(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _guard = state
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("Trwa inna operacja."))?;
    tokio::time::timeout(Duration::from_secs(90),device::pair(&id)).await.map_err(|_|ApiError::bad("pairing_timeout","Nie potwierdzono zaufania w ciągu 90 sekund. Odblokuj telefon, zaakceptuj Zaufaj i spróbuj ponownie."))?.map_err(|_|ApiError::device())?;
    Ok(Json(json!({"paired":true})))
}
async fn installed(
    Path(id): Path<String>,
) -> Result<Json<Vec<crate::models::InstalledApp>>, ApiError> {
    Ok(Json(
        tokio::time::timeout(Duration::from_secs(20), device::apps(&id))
            .await
            .map_err(|_| ApiError::device())?
            .map_err(|_| ApiError::device())?,
    ))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<account::LoginRequest>,
) -> Result<StatusCode, ApiError> {
    if body.email.len() > 320
        || !body.email.contains('@')
        || body.password.is_empty()
        || body.password.len() > 1024
        || body.anisette_url.len() > 2048
    {
        return Err(ApiError::bad(
            "invalid_login",
            "Sprawdź adres konta, hasło i adres anisette.",
        ));
    }
    body.anisette_url = account::validate_anisette(&body.anisette_url, body.allow_external)
        .map_err(|e| ApiError::bad("invalid_anisette", e.to_string()))?;
    let guard = state.operation.clone().try_lock_owned().map_err(|_| {
        ApiError::conflict(
            "Trwa inna operacja. Jeśli czekasz na 2FA, użyj formularza kodu albo Anuluj.",
        )
    })?;
    *state.engine.lock().await = None;
    *state.account.lock().map_err(|_| ApiError::internal())? = crate::state::AccountView {
        status: "logging_in".into(),
        email: Some(body.email.clone()),
        ..Default::default()
    };
    tokio::spawn(async move {
        let _guard = guard;
        let result = tokio::time::timeout(
            Duration::from_secs(600),
            std::panic::AssertUnwindSafe(account::login(state.clone(), body)).catch_unwind(),
        )
        .await;
        let message = match result {
            Ok(Ok(Ok(()))) => None,
            Ok(Ok(Err(e))) => Some(e.to_string()),
            Ok(Err(_)) | Err(_) => {
                Some("Przekroczono czas logowania. Sprawdź anisette i potwierdzenia Apple.".into())
            }
        };
        if let Some(message) = message {
            if let Ok(mut view) = state.account.lock() {
                view.status = "error".into();
                view.message = Some(message);
                view.challenge = None;
            }
            if let Ok(mut pending) = state.challenge.lock() {
                *pending = None;
            }
        }
    });
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
struct ChallengeRequest {
    id: String,
    action: String,
    #[serde(default)]
    value: String,
}
async fn challenge(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChallengeRequest>,
) -> Result<StatusCode, ApiError> {
    use isideload::auth::apple_account::TwoFactorCallbackResponse as Reply;
    let response = match body.action.as_str() {
        "code" if body.value.len() == 6 && body.value.bytes().all(|b| b.is_ascii_digit()) => {
            Reply::SubmitCode(body.value)
        }
        "devices" => Reply::SendToDevices,
        "resend" => Reply::ResendCode,
        "abort" => Reply::Abort,
        "sms" => {
            let id = body
                .value
                .parse::<u32>()
                .map_err(|_| ApiError::bad("invalid_phone", "Nieprawidłowy wybór numeru."))?;
            let view = state.account.lock().map_err(|_| ApiError::internal())?;
            if !view
                .challenge
                .as_ref()
                .is_some_and(|c| c.numbers.iter().any(|n| n.id == id))
            {
                return Err(ApiError::bad(
                    "invalid_phone",
                    "Numer nie jest dostępny dla tego wyzwania.",
                ));
            }
            Reply::SendSms(id)
        }
        _ => {
            return Err(ApiError::bad(
                "invalid_2fa",
                "Wpisz sześciocyfrowy kod albo wybierz dostępną metodę.",
            ));
        }
    };
    let mut pending = state.challenge.lock().map_err(|_| ApiError::internal())?;
    if pending.as_ref().is_none_or(|p| p.id != body.id) {
        return Err(ApiError::conflict(
            "To wyzwanie 2FA wygasło. Odśwież stan konta.",
        ));
    }
    pending
        .take()
        .unwrap()
        .sender
        .send(response)
        .map_err(|_| ApiError::conflict("Wyzwanie 2FA już nie jest aktywne."))?;
    Ok(StatusCode::ACCEPTED)
}
#[derive(Deserialize)]
struct TeamRequest {
    id: String,
}
async fn team(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TeamRequest>,
) -> Result<StatusCode, ApiError> {
    let _guard = state
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("Trwa inna operacja."))?;
    account::choose_team(&state, &body.id)
        .map_err(|e| ApiError::bad("team_unavailable", e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
async fn logout(State(state): State<Arc<AppState>>) -> Result<StatusCode, ApiError> {
    let _guard =
        state.operation.clone().try_lock_owned().map_err(|_| {
            ApiError::conflict("Trwa inna operacja. Aby przerwać 2FA, wybierz Anuluj.")
        })?;
    *state.engine.lock().await = None;
    *state.account.lock().map_err(|_| ApiError::internal())? = Default::default();
    Ok(StatusCode::NO_CONTENT)
}
async fn diagnostics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store.lock().map_err(|_| ApiError::internal())?;
    Ok(Json(
        json!({"application":"MostServer","version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"timestamp":now(),"artifact_count":store.artifacts().map_err(|_|ApiError::internal())?.len(),"job_states":store.jobs().map_err(|_|ApiError::internal())?.iter().map(|j|j.state.clone()).collect::<Vec<_>>(),"remote_enabled":false,"hardware_validation":"not_run"}),
    ))
}
async fn shutdown(State(state): State<Arc<AppState>>) -> Result<StatusCode, ApiError> {
    let _guard = state
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("Poczekaj na zakończenie bieżącej operacji."))?;
    state.shutdown.notify_one();
    Ok(StatusCode::NO_CONTENT)
}
