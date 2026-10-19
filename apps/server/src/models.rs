use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub name: String,
    pub expires_at: Option<String>,
    pub expired: bool,
    pub team_id: Option<String>,
    pub metadata_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpaMetadata {
    pub name: String,
    pub bundle_id: String,
    pub version: String,
    pub build: String,
    pub minimum_ios: Option<String>,
    pub app_directory: String,
    pub extensions: usize,
    pub unpacked_bytes: u64,
    pub has_signature_files: bool,
    pub profile: Option<ProfileSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub sha256: String,
    pub size: u64,
    pub imported_at: String,
    pub metadata: IpaMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub artifact_id: String,
    pub device_id: String,
    pub mode: String,
    pub state: String,
    pub progress: Option<u64>,
    pub message: String,
    pub effective_bundle_id: Option<String>,
    pub expected_build: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub ios: Option<String>,
    pub connection: String,
    pub trusted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledApp {
    pub bundle_id: String,
    pub name: String,
    pub version: String,
    pub build: String,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
