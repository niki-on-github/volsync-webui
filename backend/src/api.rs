use crate::kubectl::{Kubectl, KubeError};
use crate::models::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type AppState = Arc<RwLock<Kubectl>>;

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    Conflict(String),
    Error(String),
}

impl From<KubeError> for ApiError {
    fn from(err: KubeError) -> Self {
        match err {
            KubeError::NotFound(msg) => ApiError::NotFound(msg),
            KubeError::SnapshotFailed(msg) => ApiError::Conflict(msg),
            KubeError::Timeout(msg) => ApiError::Error(msg),
            KubeError::Api(msg) => ApiError::Error(msg),
            KubeError::InvalidMethod(msg) => ApiError::Error(msg),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Intentionally return distinct HTTP status codes so the frontend can differentiate
        // between "resource not found" (404), "conflict/operation failed" (409), and
        // genuine server errors (500). This is desired behavior — a blanket 500 would hide
        // actionable errors from the UI (e.g., showing "app not found" vs a generic failure).
        let (status, error_msg) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::Error(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let payload = Json(serde_json::json!({ "error": error_msg }));
        (status, payload).into_response()
    }
}

pub async fn list_apps(
    State(kubectl): State<AppState>,
    axum::extract::Query(namespace): axum::extract::Query<Option<String>>,
) -> Result<Json<Vec<App>>, ApiError> {
    let kubectl = {
        let guard = kubectl.read().await;
        guard.clone()
    };
    let apps = kubectl.list_apps(namespace.as_deref()).await.map_err(ApiError::from)?;
    Ok(Json(apps))
}

pub async fn list_namespaces(State(kubectl): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    let kubectl = {
        let guard = kubectl.read().await;
        guard.clone()
    };
    let namespaces = kubectl.list_namespaces().await.map_err(ApiError::from)?;
    Ok(Json(namespaces))
}

pub async fn get_snapshots(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<Vec<Snapshot>>, ApiError> {
    let kubectl = {
        let guard = kubectl.read().await;
        guard.clone()
    };
    let snapshots = kubectl.get_snapshots(&app, &ns).await.map_err(ApiError::from)?;
    Ok(Json(snapshots))
}

pub async fn trigger_backup(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<BackupResponse>, ApiError> {
    // Clone Kubectl and drop lock before long polling loop (Bug #2 fix)
    let kubectl = {
        let guard = kubectl.read().await;
        guard.clone()
    };
    let trigger = format!("backup-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    let resp = kubectl.trigger_backup(&app, &ns, &trigger).await.map_err(ApiError::from)?;
    Ok(Json(resp))
}

pub async fn trigger_backup_all(State(kubectl): State<AppState>) -> Result<Json<BackupAllResponse>, ApiError> {
    // Clone Kubectl and drop lock before spawning concurrent tasks (Bug #3 fix)
    let kubectl = {
        let guard = kubectl.read().await;
        guard.clone()
    };
    let trigger = format!("backup-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    let apps = kubectl.trigger_backup_all(&trigger).await.map_err(ApiError::from)?;

    // Aggregate summary
    let total = apps.len();
    let success = apps.iter().filter(|a| a.success).count();
    let failed = total - success;

    Ok(Json(BackupAllResponse {
        trigger,
        apps,
        summary: Some(BackupSummary {
            total,
            success,
            failed,
        }),
    }))
}

pub async fn trigger_restore(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
    Json(req): Json<RestoreRequest>,
) -> Result<Json<RestoreResponse>, ApiError> {
    let kubectl = {
        let guard = kubectl.read().await;
        guard.clone()
    };
    let resp = kubectl.trigger_restore(&app, &ns, &req.trigger, req.timestamp.as_deref())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(resp))
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}
