use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use isideload::util::storage::SideloadingStorage;
use most_server::{
    api, ipa,
    models::{Artifact, Job, now},
    state::AppState,
    store::Store,
    vault::Vault,
};
use std::{
    io::{Cursor, Write},
    path::Path,
    sync::Arc,
};
use tower::ServiceExt;
use zip::write::SimpleFileOptions;

fn macho(encrypted: bool) -> Vec<u8> {
    let mut binary = vec![0u8; 32];
    binary[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
    if encrypted {
        binary[16..20].copy_from_slice(&1u32.to_le_bytes());
        binary[20..24].copy_from_slice(&24u32.to_le_bytes());
        let mut command = vec![0u8; 24];
        command[..4].copy_from_slice(&0x2cu32.to_le_bytes());
        command[4..8].copy_from_slice(&24u32.to_le_bytes());
        command[16..20].copy_from_slice(&1u32.to_le_bytes());
        binary.extend(command);
    }
    binary
}

fn macho_with_entitlement(key: &str) -> Vec<u8> {
    let xml = format!(
        "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>{key}</key><true/></dict></plist>"
    );
    let mut signature = vec![];
    for word in [
        0xfade0cc0u32,
        (28 + xml.len()) as u32,
        1,
        5,
        20,
        0xfade7171,
        (8 + xml.len()) as u32,
    ] {
        signature.extend(word.to_be_bytes());
    }
    signature.extend(xml.as_bytes());
    let mut binary = macho(false);
    binary[16..20].copy_from_slice(&1u32.to_le_bytes());
    binary[20..24].copy_from_slice(&16u32.to_le_bytes());
    for word in [0x1du32, 16, 48, signature.len() as u32] {
        binary.extend(word.to_le_bytes());
    }
    binary.extend(signature);
    binary
}
#[test]
fn signing_does_not_silently_drop_unsupported_entitlements() {
    let root = tempfile::tempdir().unwrap();
    let info=br#"<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Test</string></dict></plist>"#;
    std::fs::write(root.path().join("Info.plist"), info).unwrap();
    std::fs::write(
        root.path().join("Test"),
        macho_with_entitlement("aps-environment"),
    )
    .unwrap();
    assert!(
        ipa::check_basic_signing(root.path())
            .unwrap_err()
            .to_string()
            .contains("aps-environment")
    );
    std::fs::write(
        root.path().join("Test"),
        macho_with_entitlement("application-identifier"),
    )
    .unwrap();
    assert!(ipa::check_basic_signing(root.path()).is_ok());
}
fn ipa_bytes(extras: Vec<(&str, Vec<u8>)>, encrypted: bool) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let info=br#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.most.test</string><key>CFBundleName</key><string>Most Test</string><key>CFBundleExecutable</key><string>Test</string><key>CFBundleShortVersionString</key><string>1.2</string><key>CFBundleVersion</key><string>7</string><key>CFBundleSupportedPlatforms</key><array><string>iPhoneOS</string></array></dict></plist>"#;
    for (name, data) in [
        ("Payload/Test.app/Info.plist", info.to_vec()),
        ("Payload/Test.app/Test", macho(encrypted)),
    ]
    .into_iter()
    .chain(extras)
    {
        archive.start_file(name, options).unwrap();
        archive.write_all(&data).unwrap();
    }
    archive.finish().unwrap().into_inner()
}
fn write_ipa(root: &Path, bytes: &[u8]) -> std::path::PathBuf {
    let path = root.join("test.ipa");
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn reads_real_plist_and_macho_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_ipa(dir.path(), &ipa_bytes(vec![], false));
    let metadata = ipa::inspect(&path).unwrap();
    assert_eq!(metadata.bundle_id, "org.most.test");
    assert_eq!(metadata.build, "7");
    assert!(metadata.profile.is_none());
    assert!(!metadata.has_signature_files);
}
#[test]
fn rejects_drm_binary() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_ipa(dir.path(), &ipa_bytes(vec![], true));
    assert!(ipa::inspect(&path).unwrap_err().to_string().contains("DRM"));
}
#[test]
fn rejects_archive_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "../outside",
        "Payload/../../outside",
        "/outside",
        "Payload\\escape",
        "Payload/Test.app/a:stream",
        "Payload/Test.app/CON.txt",
        "Payload/Test.app/trailing.",
    ] {
        let path = write_ipa(dir.path(), &ipa_bytes(vec![(name, vec![1])], false));
        assert!(ipa::inspect(&path).is_err(), "{name}");
        assert!(!dir.path().parent().unwrap().join("outside").exists());
    }
}
#[test]
fn rejects_case_collisions_on_windows() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_ipa(
        dir.path(),
        &ipa_bytes(vec![("Payload/Test.app/info.plist", vec![1])], false),
    );
    assert!(
        ipa::inspect(&path)
            .unwrap_err()
            .to_string()
            .contains("powtarzające")
    );
}
#[test]
fn rejects_multiple_main_apps() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_ipa(
        dir.path(),
        &ipa_bytes(vec![("Payload/Other.app/Info.plist", vec![1])], false),
    );
    assert!(
        ipa::inspect(&path)
            .unwrap_err()
            .to_string()
            .contains("dokładnie jedną")
    );
}
#[test]
fn rejects_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = ipa_bytes(vec![], false);
    let mut zip = zip::ZipWriter::new_append(Cursor::new(bytes)).unwrap();
    zip.add_symlink(
        "Payload/Test.app/link",
        "../../outside",
        SimpleFileOptions::default(),
    )
    .unwrap();
    let path = write_ipa(dir.path(), &zip.finish().unwrap().into_inner());
    assert!(
        ipa::inspect(&path)
            .unwrap_err()
            .to_string()
            .contains("dowiązanie")
    );
}
#[test]
fn extraction_does_not_write_non_app_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_ipa(
        dir.path(),
        &ipa_bytes(vec![("Other/file", b"not-app".to_vec())], false),
    );
    let metadata = ipa::inspect(&path).unwrap();
    let output = tempfile::tempdir().unwrap();
    let app = ipa::extract_app(&path, output.path(), &metadata).unwrap();
    assert!(app.join("Test").exists());
    assert!(!output.path().join("Other").exists());
}
#[test]
fn profile_is_explicitly_metadata_only() {
    let dir = tempfile::tempdir().unwrap();
    let profile=br#"CMS<?xml version="1.0"?><plist version="1.0"><dict><key>Name</key><string>Unsigned test fixture</string><key>ExpirationDate</key><date>2020-01-01T00:00:00Z</date></dict></plist>CMS"#;
    let path = write_ipa(
        dir.path(),
        &ipa_bytes(
            vec![(
                "Payload/Test.app/embedded.mobileprovision",
                profile.to_vec(),
            )],
            false,
        ),
    );
    let profile = ipa::inspect(&path).unwrap().profile.unwrap();
    assert!(profile.metadata_only);
    assert!(profile.expired);
}

fn fixture_artifact() -> Artifact {
    let dir = tempfile::tempdir().unwrap();
    let path = write_ipa(dir.path(), &ipa_bytes(vec![], false));
    Artifact {
        id: "artifact".into(),
        sha256: "a".repeat(64),
        size: 100,
        imported_at: now(),
        metadata: ipa::inspect(&path).unwrap(),
    }
}
fn fixture_job(state: &str) -> Job {
    Job {
        id: "job".into(),
        artifact_id: "artifact".into(),
        device_id: "test-device".into(),
        mode: "presigned".into(),
        state: state.into(),
        progress: None,
        message: "test".into(),
        effective_bundle_id: Some("org.most.test".into()),
        expected_build: "7".into(),
        created_at: now(),
        updated_at: now(),
    }
}
#[test]
fn persists_library_and_recovers_unknown_installation() {
    let root = tempfile::tempdir().unwrap();
    {
        let store = Store::open(root.path()).unwrap();
        store.add_artifact(&fixture_artifact()).unwrap();
        store
            .add_job(&fixture_job("installing"), "request", "body")
            .unwrap();
    }
    let reopened = Store::open(root.path()).unwrap();
    assert_eq!(reopened.artifacts().unwrap().len(), 1);
    assert_eq!(reopened.job("job").unwrap().state, "needs_reconciliation");
    assert!(reopened.request("request", "changed body").is_err());
    assert!(reopened.request("request", "body").unwrap().is_some());
}
#[test]
fn vault_roundtrip_and_tamper_detection() {
    let root = tempfile::tempdir().unwrap();
    let vault = Vault::open(root.path()).unwrap();
    vault.store("test-secret", "sensitive-value").unwrap();
    assert_eq!(
        vault.retrieve("test-secret").unwrap().as_deref(),
        Some("sensitive-value")
    );
    for item in std::fs::read_dir(root.path()).unwrap() {
        let path = item.unwrap().path();
        if path.file_name().unwrap().to_string_lossy().starts_with('.') {
            continue;
        }
        let mut bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.windows(15).any(|v| v == b"sensitive-value"));
        bytes[15] ^= 1;
        std::fs::write(path, bytes).unwrap();
    }
    assert!(vault.retrieve("test-secret").is_err());
}
#[test]
fn rejects_unsafe_anisette_endpoints() {
    use most_server::account::validate_anisette;
    assert!(validate_anisette("http://127.0.0.1:6969", false).is_ok());
    assert!(validate_anisette("http://attacker.example", true).is_err());
    assert!(validate_anisette("https://trusted.example", false).is_err());
    assert!(validate_anisette("https://trusted.example", true).is_ok());
    assert!(validate_anisette("https://user:password@trusted.example", true).is_err());
}

fn test_state(root: &Path) -> Arc<AppState> {
    AppState::new(root, "127.0.0.1:49240".into(), "a".repeat(64)).unwrap()
}
fn request(method: &str, path: &str, body: Body) -> axum::http::request::Builder {
    let _ = body;
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:49240")
        .header("authorization", format!("Bearer {}", "a".repeat(64)))
}
#[tokio::test]
async fn api_requires_local_authentication() {
    let root = tempfile::tempdir().unwrap();
    let state = test_state(root.path());
    let response = api::router(state)
        .oneshot(
            Request::builder()
                .uri("/api/state")
                .header("host", "127.0.0.1:49240")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn api_rejects_dns_rebinding_and_foreign_origin() {
    for (header, value) in [
        ("host", "evil.example"),
        ("origin", "https://evil.example"),
        ("sec-fetch-site", "cross-site"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let state = test_state(root.path());
        let mut req = request("GET", "/api/state", Body::empty())
            .body(Body::empty())
            .unwrap();
        req.headers_mut().insert(
            axum::http::HeaderName::from_bytes(header.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
        let response = api::router(state).oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{header}");
    }
}
#[tokio::test]
async fn status_does_not_claim_remote_support() {
    let root = tempfile::tempdir().unwrap();
    let response = api::router(test_state(root.path()))
        .oneshot(
            request("GET", "/api/state", Body::empty())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value["capabilities"]["remote"], false);
    assert_eq!(value["capabilities"]["local_vpn"], false);
}
#[tokio::test]
async fn imports_large_multipart_and_deduplicates() {
    let root = tempfile::tempdir().unwrap();
    let state = test_state(root.path());
    let zip = ipa_bytes(vec![("Payload/Test.app/data", vec![42u8; 40000])], false);
    let mut body=b"--test-boundary\r\nContent-Disposition: form-data; name=\"ipa\"; filename=\"test.ipa\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
    body.extend(zip);
    body.extend(b"\r\n--test-boundary--\r\n");
    for expected in [StatusCode::CREATED, StatusCode::OK] {
        let req = request("POST", "/api/artifacts", Body::empty())
            .header(
                "content-type",
                "multipart/form-data; boundary=test-boundary",
            )
            .body(Body::from(body.clone()))
            .unwrap();
        let response = api::router(state.clone()).oneshot(req).await.unwrap();
        assert_eq!(response.status(), expected);
        let artifact: Artifact =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(
            state
                .store
                .lock()
                .unwrap()
                .artifact_path(&artifact)
                .exists()
        );
    }
    assert_eq!(state.store.lock().unwrap().artifacts().unwrap().len(), 1);
}
#[tokio::test]
async fn malformed_upload_is_removed() {
    let root = tempfile::tempdir().unwrap();
    let state = test_state(root.path());
    let body = "--test\r\nContent-Disposition: form-data; name=\"ipa\"; filename=\"bad.ipa\"\r\n\r\nnot-a-zip\r\n--test--\r\n";
    let response = api::router(state.clone())
        .oneshot(
            request("POST", "/api/artifacts", Body::empty())
                .header("content-type", "multipart/form-data; boundary=test")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        std::fs::read_dir(root.path().join("temporary"))
            .unwrap()
            .count(),
        0
    );
    assert!(state.store.lock().unwrap().artifacts().unwrap().is_empty());
}
#[tokio::test]
async fn signed_job_requires_apple_account() {
    let root = tempfile::tempdir().unwrap();
    let state = test_state(root.path());
    state
        .store
        .lock()
        .unwrap()
        .add_artifact(&fixture_artifact())
        .unwrap();
    let body = r#"{"artifact_id":"artifact","device_id":"test-device","mode":"resign"}"#;
    let response = api::router(state.clone())
        .oneshot(
            request("POST", "/api/jobs", Body::empty())
                .header("content-type", "application/json")
                .header("idempotency-key", "test")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(state.store.lock().unwrap().jobs().unwrap().is_empty());
}
#[tokio::test]
async fn idempotent_request_returns_original_job_without_running_install() {
    let root = tempfile::tempdir().unwrap();
    let state = test_state(root.path());
    let body = r#"{"artifact_id":"artifact","device_id":"test-device","mode":"presigned"}"#;
    state
        .store
        .lock()
        .unwrap()
        .add_job(&fixture_job("succeeded"), "test", body)
        .unwrap();
    let response = api::router(state)
        .oneshot(
            request("POST", "/api/jobs", Body::empty())
                .header("content-type", "application/json")
                .header("idempotency-key", "test")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let job: Job =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(job.id, "job");
}
#[tokio::test]
async fn diagnostics_exclude_authentication_and_device_identifiers() {
    let root = tempfile::tempdir().unwrap();
    let state = test_state(root.path());
    state
        .store
        .lock()
        .unwrap()
        .add_job(&fixture_job("succeeded"), "test", "body")
        .unwrap();
    let response = api::router(state)
        .oneshot(
            request("GET", "/api/diagnostics", Body::empty())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
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
    assert!(!body.contains("test-device"));
    assert!(!body.contains(&"a".repeat(64)));
    assert!(body.contains("not_run"));
}
