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
    pub repository: Option<String>,
    pub backup_pending: bool,
    pub restore_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    pub task_type: String,
    pub app: String,
    pub namespace: String,
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub short_id: String,
    pub time: String,
    pub tags: Vec<String>,
    pub paths: Vec<String>,
    pub hostname: String,
    pub files_new: i64,
    pub files_changed: i64,
    pub files_unmodified: i64,
    pub data_added: i64,
    pub total_files_processed: i64,
    pub total_bytes_processed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResponse {
    pub trigger: String,
    pub status: String,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreRequest {
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResponse {
    pub trigger: String,
    pub status: String,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppConfig {
    pub refresh_interval_secs: u64,
}
