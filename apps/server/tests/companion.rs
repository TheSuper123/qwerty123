use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use isideload::util::storage::SideloadingStorage;
use most_server::{
    companion::{self, Client, StartRequest},
    models::{Job, now},
    state::AppState,
};
use std::sync::Arc;
use tower::ServiceExt;

fn state(root: &std::path::Path) -> Arc<AppState> {
    let state = AppState::new(root, "127.0.0.1:49240".into(), "a".repeat(64)).unwrap();
    state.clients.lock().unwrap().push(Client {
        id: "client-one".into(),
        device_id: "device-one".into(),
        name: "Test iPhone".into(),
        token: "b".repeat(64),
        created_at: now(),
    });
    state
}
fn request(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:49241")
        .header("authorization", format!("Bearer {}", "b".repeat(64)))
}
fn job(id: &str, device_id: &str) -> Job {
    Job {
        id: id.into(),
        device_id: device_id.into(),
        artifact_id: "ipa".into(),
        mode: "presigned".into(),
        state: "needs_reconciliation".into(),
        progress: None,
        message: "test".into(),
        effective_bundle_id: Some("test.app".into()),
        expected_build: "1".into(),
        created_at: now(),
        updated_at: now(),
    }
}
#[tokio::test]
async fn mobile_snapshot_has_only_assigned_jobs_and_no_apple_identity() {
    let root = tempfile::tempdir().unwrap();
    let state = state(root.path());
    {
        let store = state.store.lock().unwrap();
        store
            .add_job(&job("mine", "device-one"), "mine", "body")
            .unwrap();
        store
            .add_job(&job("other", "device-two"), "other", "body")
            .unwrap();
    }
    state.account.lock().unwrap().email = Some("private@example.test".into());
    let response = companion::router(state)
        .oneshot(request("GET", "/v1/state").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["jobs"].as_array().unwrap().len(), 1);
    assert_eq!(value["jobs"][0]["id"], "mine");
    assert!(!body.contains("private@example.test"));
    assert!(!body.contains(&"b".repeat(64)));
}
#[tokio::test]
async fn cannot_install_to_another_device() {
    let root = tempfile::tempdir().unwrap();
    let state = state(root.path());
    let response = companion::router(state)
        .oneshot(
            request("POST", "/v1/jobs")
                .header("content-type", "application/json")
                .header("idempotency-key", "request")
                .body(Body::from(
                    r#"{"artifact_id":"ipa","device_id":"device-two","mode":"presigned"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn cannot_reconcile_someone_elses_job() {
    let root = tempfile::tempdir().unwrap();
    let state = state(root.path());
    state
        .store
        .lock()
        .unwrap()
        .add_job(&job("other", "device-two"), "other", "body")
        .unwrap();
    let response = companion::router(state)
        .oneshot(
            request("POST", "/v1/jobs/other/reconcile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn revoked_client_is_rejected_immediately() {
    let root = tempfile::tempdir().unwrap();
    let state = state(root.path());
    state.clients.lock().unwrap().clear();
    let response = companion::router(state)
        .oneshot(request("GET", "/v1/state").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn mobile_api_does_not_expose_apple_login() {
    let root = tempfile::tempdir().unwrap();
    let response = companion::router(state(root.path()))
        .oneshot(
            request("POST", "/api/account/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn browser_cannot_use_mobile_api() {
    let root = tempfile::tempdir().unwrap();
    let response = companion::router(state(root.path()))
        .oneshot(
            request("GET", "/v1/state")
                .header("origin", "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn lan_rejects_public_interface() {
    let root = tempfile::tempdir().unwrap();
    let result = companion::start(
        State(state(root.path())),
        Json(StartRequest {
            address: "8.8.8.8".parse().unwrap(),
            port: 0,
        }),
    )
    .await;
    assert!(result.is_err());
}
#[tokio::test]
async fn starts_real_tls_listener_with_persisted_certificate() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let root = tempfile::tempdir().unwrap();
    let state = state(root.path());
    let view = companion::start(
        State(state.clone()),
        Json(StartRequest {
            address: "127.0.0.1".parse().unwrap(),
            port: 0,
        }),
    )
    .await
    .unwrap_or_else(|_| panic!("TLS listener failed"));
    assert!(view.enabled);
    assert_eq!(view.certificate_sha256.as_ref().unwrap().len(), 64);
    let url = url::Url::parse(view.url.as_ref().unwrap()).unwrap();
    let cert = state
        .vault
        .retrieve_data("companion-tls-certificate")
        .unwrap()
        .unwrap();
    assert!(cert.starts_with(b"-----BEGIN CERTIFICATE-----"));
    let client = reqwest::Client::builder()
        .tls_backend_rustls()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .tls_certs_only([reqwest::Certificate::from_pem(&cert).unwrap()])
        .build()
        .unwrap();
    let response = client
        .get(url.join("v1/state").unwrap())
        .bearer_auth("b".repeat(64))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let snapshot: serde_json::Value = response.json().await.unwrap();
    assert_eq!(snapshot["device_id"], "device-one");
    let unknown_ca = reqwest::Client::builder()
        .tls_backend_rustls()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    assert!(
        unknown_ca
            .get(url.join("v1/state").unwrap())
            .send()
            .await
            .is_err(),
        "Unknown server certificate was accepted"
    );
    assert!(
        state
            .vault
            .retrieve_data("companion-tls-key")
            .unwrap()
            .is_some()
    );
}
