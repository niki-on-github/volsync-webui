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

pub type AppState = Arc<Kubectl>;

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
    axum::extract::Query(query): axum::extract::Query<AppQuery>,
) -> Result<Json<Vec<App>>, ApiError> {
    let ns_filter = query.namespace.as_deref().unwrap_or("all");
    tracing::debug!("list_apps called with namespace filter: {}", ns_filter);

    let apps = kubectl.list_apps(query.namespace.as_deref()).await.map_err(ApiError::from)?;
    tracing::debug!("list_apps returning {} apps", apps.len());
    Ok(Json(apps))
}

pub async fn list_namespaces(State(kubectl): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    tracing::debug!("list_namespaces called");

    let namespaces = kubectl.list_namespaces().await.map_err(ApiError::from)?;
    tracing::debug!("list_namespaces returning {} namespaces", namespaces.len());
    Ok(Json(namespaces))
}

pub async fn get_snapshots(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<Vec<Snapshot>>, ApiError> {
    tracing::debug!("get_snapshots called for app={} namespace={}", app, ns);

    let snapshots = kubectl.get_snapshots(&app, &ns).await.map_err(ApiError::from)?;
    tracing::debug!("get_snapshots returning {} snapshots for app={}", snapshots.len(), app);
    Ok(Json(snapshots))
}

pub async fn trigger_backup(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<BackupResponse>, ApiError> {
    tracing::info!("trigger_backup called for app={} namespace={}", app, ns);

    let trigger = format!("backup-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    tracing::debug!("trigger_backup using trigger ID: {}", trigger);
    let resp = kubectl.trigger_backup(&app, &ns, &trigger).await.map_err(ApiError::from)?;
    tracing::info!("trigger_backup completed for app={} status={}", app, resp.status);
    Ok(Json(resp))
}

pub async fn trigger_backup_all(State(kubectl): State<AppState>) -> Result<Json<BackupAllResponse>, ApiError> {
    tracing::info!("trigger_backup_all called");

    let trigger = format!("backup-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    tracing::debug!("trigger_backup_all using trigger ID: {}", trigger);
    let apps = kubectl.trigger_backup_all(&trigger).await.map_err(ApiError::from)?;

    let total = apps.len();
    let success = apps.iter().filter(|a| a.success).count();
    let failed = total - success;
    tracing::info!("trigger_backup_all completed: {}/{} succeeded, {} failed", success, total, failed);

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
    let resp = kubectl.trigger_restore(&app, &ns, &req.trigger, req.timestamp.as_deref())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(resp))
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}
