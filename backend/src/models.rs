use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub name: String,
    pub namespace: String,
    pub last_sync_time: Option<String>,
    pub last_sync_duration: Option<String>,
    pub last_result: Option<String>,
    pub next_sync_time: Option<String>,
    pub in_progress: bool,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub time: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResponse {
    pub trigger: String,
    pub status: String,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreRequest {
    pub trigger: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResponse {
    pub trigger: String,
    pub status: String,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSummary {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupAllResponse {
    pub trigger: String,
    pub apps: Vec<AppBackupStatus>,
    pub summary: Option<BackupSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBackupStatus {
    pub app: String,
    pub namespace: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppConfig {
    pub refresh_interval_secs: u64,
}
