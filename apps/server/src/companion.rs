use crate::{
    api,
    error::ApiError,
    models::{Artifact, Job},
    state::AppState,
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use isideload::util::storage::SideloadingStorage;
use rand::RngCore;
use rustls::pki_types::{CertificateDer, pem::PemObject};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
};
use subtle::ConstantTimeEq;

#[derive(Clone, Default, Serialize)]
pub struct NetworkView {
    pub enabled: bool,
    pub url: Option<String>,
    pub certificate_sha256: Option<String>,
    pub message: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Client {
    pub id: String,
    pub device_id: String,
    pub name: String,
    pub token: String,
    pub created_at: String,
}
impl Client {
    pub fn view(&self) -> serde_json::Value {
        json!({"id":self.id,"device_id":self.device_id,"name":self.name,"created_at":self.created_at})
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/state", get(mobile_state))
        .route(
            "/v1/artifacts",
            post(mobile_upload).layer(DefaultBodyLimit::max(
                crate::ipa::MAX_IPA_BYTES as usize + 64 * 1024,
            )),
        )
        .route("/v1/jobs", post(mobile_job))
        .route("/v1/jobs/{id}/reconcile", post(mobile_reconcile))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .with_state(state)
}
async fn authenticate(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    // Native clients only; browser access belongs to the separate loopback panel.
    if request.headers().contains_key("origin") || request.headers().contains_key("sec-fetch-site")
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    let client = state.clients.lock().ok().and_then(|clients| {
        clients
            .iter()
            .find(|client| client.token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() == 1)
            .cloned()
    });
    let Some(client) = client else {
        return (StatusCode::UNAUTHORIZED,Json(json!({"code":"untrusted_client","message":"Połącz MostStore ponownie kodem QR z komputera."}))).into_response();
    };
    request.extensions_mut().insert(client);
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
}
async fn mobile_state(
    State(state): State<Arc<AppState>>,
    Extension(client): Extension<Client>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let ready = {
        let account = state.account.lock().map_err(|_| ApiError::internal())?;
        account.status == "logged_in" && account.selected_team.is_some()
    };
    let store = state.store.lock().map_err(|_| ApiError::internal())?;
    let jobs: Vec<_> = store
        .jobs()
        .map_err(|_| ApiError::internal())?
        .into_iter()
        .filter(|j| j.device_id == client.device_id)
        .collect();
    Ok(Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"device_id":client.device_id,"device_name":client.name,"apple_ready":ready,"busy":state.operation.try_lock().is_err(),"artifacts":store.artifacts().map_err(|_|ApiError::internal())?,"jobs":jobs,"remote_relay":false,"local_vpn":false}),
    ))
}
async fn mobile_upload(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<Artifact>), ApiError> {
    api::upload(State(state), multipart).await
}
async fn mobile_job(
    State(state): State<Arc<AppState>>,
    Extension(client): Extension<Client>,
    mut headers: HeaderMap,
    Json(body): Json<api::JobRequest>,
) -> Result<(StatusCode, Json<Job>), ApiError> {
    if body.device_id != client.device_id {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            code: "device_scope",
            message: "Ten klient jest przypisany do innego urządzenia.".into(),
        });
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|k| !k.is_empty() && k.len() <= 80)
        .ok_or_else(|| {
            ApiError::bad("idempotency_key_required", "Brak identyfikatora zlecenia.")
        })?;
    let scoped = format!("{}:{key}", client.id);
    headers.insert(
        "idempotency-key",
        scoped
            .parse()
            .map_err(|_| ApiError::bad("invalid_key", "Nieprawidłowy identyfikator."))?,
    );
    api::create_job(State(state), headers, Json(body)).await
}
async fn mobile_reconcile(
    State(state): State<Arc<AppState>>,
    Extension(client): Extension<Client>,
    Path(id): Path<String>,
) -> Result<Json<Job>, ApiError> {
    {
        let store = state.store.lock().map_err(|_| ApiError::internal())?;
        let job = store
            .job(&id)
            .map_err(|_| ApiError::bad("job_not_found", "Nie znaleziono zadania."))?;
        if job.device_id != client.device_id {
            return Err(ApiError {
                status: StatusCode::FORBIDDEN,
                code: "device_scope",
                message: "To zadanie należy do innego urządzenia.".into(),
            });
        }
    }
    api::reconcile(State(state), Path(id)).await
}

#[derive(Deserialize)]
pub struct StartRequest {
    pub address: Ipv4Addr,
    #[serde(default = "default_port")]
    pub port: u16,
}
fn default_port() -> u16 {
    49241
}
pub async fn start(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StartRequest>,
) -> Result<Json<NetworkView>, ApiError> {
    let _guard = state
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("Trwa inna operacja."))?;
    if !body.address.is_private() && !body.address.is_loopback() {
        return Err(ApiError::bad(
            "lan_address_required",
            "Wybierz prywatny adres IPv4 tego komputera, np. 192.168.1.20. Usługa nie jest przeznaczona do internetu.",
        ));
    }
    if state
        .network
        .lock()
        .map_err(|_| ApiError::internal())?
        .enabled
    {
        return Err(ApiError::conflict(
            "MostStore LAN jest już włączony. Aby zmienić adres, uruchom MostServer ponownie.",
        ));
    }
    let cert = state
        .vault
        .retrieve_data("companion-tls-certificate")
        .map_err(|_| ApiError::internal())?;
    let key = state
        .vault
        .retrieve_data("companion-tls-key")
        .map_err(|_| ApiError::internal())?;
    let previous_address = state
        .vault
        .retrieve("companion-tls-address")
        .map_err(|_| ApiError::internal())?;
    let (cert, key) = if let (Some(cert), Some(key), true) = (
        cert,
        key,
        previous_address.as_deref() == Some(body.address.to_string().as_str()),
    ) {
        (cert, key)
    } else {
        let pair =
            rcgen::generate_simple_self_signed(vec![body.address.to_string(), "localhost".into()])
                .map_err(|_| ApiError::internal())?;
        let cert = pair.cert.pem().into_bytes();
        let key = pair.signing_key.serialize_pem().into_bytes();
        state
            .vault
            .store_data("companion-tls-certificate", &cert)
            .map_err(|_| ApiError::internal())?;
        state
            .vault
            .store_data("companion-tls-key", &key)
            .map_err(|_| ApiError::internal())?;
        (cert, key)
    };
    state
        .vault
        .store("companion-tls-address", &body.address.to_string())
        .map_err(|_| ApiError::internal())?;
    let der = CertificateDer::from_pem_slice(&cert).map_err(|_| ApiError::internal())?;
    let fingerprint = hex::encode(Sha256::digest(der.as_ref()));
    let tls = axum_server::tls_rustls::RustlsConfig::from_pem(cert, key)
        .await
        .map_err(|_| ApiError::internal())?;
    let listener=std::net::TcpListener::bind(SocketAddrV4::new(body.address,body.port)).map_err(|_|ApiError::bad("lan_bind_failed","Nie można nasłuchiwać pod tym adresem. Wpisz adres IPv4 przypisany do tego PC i sprawdź zajętość portu."))?;
    listener
        .set_nonblocking(true)
        .map_err(|_| ApiError::internal())?;
    let address = listener.local_addr().map_err(|_| ApiError::internal())?;
    let network = NetworkView {
        enabled: true,
        url: Some(format!("https://{address}")),
        certificate_sha256: Some(fingerprint),
        message: None,
    };
    *state.network.lock().map_err(|_| ApiError::internal())? = network.clone();
    let server_state = state.clone();
    tokio::spawn(async move {
        let result = match axum_server::from_tcp_rustls(listener, tls) {
            Ok(server) => {
                server
                    .serve(router(server_state.clone()).into_make_service())
                    .await
            }
            Err(error) => Err(error),
        };
        if result.is_err()
            && let Ok(mut view) = server_state.network.lock()
        {
            view.enabled = false;
            view.message =
                Some("Serwer MostStore LAN zatrzymał się. Uruchom MostServer ponownie.".into());
        }
    });
    Ok(Json(network))
}

pub async fn clients(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    Ok(Json(
        state
            .clients
            .lock()
            .map_err(|_| ApiError::internal())?
            .iter()
            .map(Client::view)
            .collect(),
    ))
}
#[derive(Deserialize)]
pub struct PairRequest {
    pub device_id: String,
}
pub async fn pair_client(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PairRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _guard = state
        .operation
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("Trwa inna operacja."))?;
    let network = state
        .network
        .lock()
        .map_err(|_| ApiError::internal())?
        .clone();
    if !network.enabled {
        return Err(ApiError::bad(
            "lan_not_started",
            "Najpierw uruchom MostStore LAN na ekranie Połączenie.",
        ));
    }
    let provider = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        crate::device::provider(&body.device_id),
    )
    .await
    .map_err(|_| ApiError::device())?
    .map_err(|_| ApiError::device())?;
    let info = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        isideload::util::device::IdeviceInfo::from_device(&provider),
    )
    .await
    .map_err(|_| ApiError::device())?
    .map_err(|_| ApiError::device())?;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let client = Client {
        id: uuid::Uuid::new_v4().to_string(),
        device_id: body.device_id.clone(),
        name: info.name,
        token: hex::encode(bytes),
        created_at: crate::models::now(),
    };
    let payload = json!({"schema":1,"url":network.url,"token":client.token,"certificate_sha256":network.certificate_sha256,"device_id":body.device_id});
    let pairing_json = serde_json::to_string(&payload).map_err(|_| ApiError::internal())?;
    let qr = qrcode::QrCode::new(pairing_json.as_bytes())
        .map_err(|_| ApiError::internal())?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(320, 320)
        .build();
    let mut clients = state.clients.lock().map_err(|_| ApiError::internal())?;
    if clients.len() >= 16 {
        return Err(ApiError::bad(
            "client_limit",
            "Usuń nieużywane powiązanie MostStore przed dodaniem kolejnego.",
        ));
    }
    let mut updated = clients.clone();
    updated.push(client);
    state
        .vault
        .store(
            "companion-clients",
            &serde_json::to_string(&updated).map_err(|_| ApiError::internal())?,
        )
        .map_err(|_| ApiError::internal())?;
    *clients = updated;
    Ok(Json(json!({"pairing_json":pairing_json,"qr_svg":qr})))
}
pub async fn revoke_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let mut clients = state.clients.lock().map_err(|_| ApiError::internal())?;
    let updated: Vec<_> = clients.iter().filter(|c| c.id != id).cloned().collect();
    state
        .vault
        .store(
            "companion-clients",
            &serde_json::to_string(&updated).map_err(|_| ApiError::internal())?,
        )
        .map_err(|_| ApiError::internal())?;
    *clients = updated;
    Ok(StatusCode::NO_CONTENT)
}
