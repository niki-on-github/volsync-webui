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
) -> Result<Json<Vec<App>>, ApiError> {
    tracing::debug!("list_apps called");
    let apps = kubectl.list_apps().await.map_err(|e| {
        tracing::error!("list_apps failed: {}", e);
        ApiError::from(e)
    })?;
    tracing::debug!("list_apps returning {} apps", apps.len());
    Ok(Json(apps))
}

pub async fn list_namespaces(State(kubectl): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    tracing::debug!("list_namespaces called");
    let namespaces = kubectl.list_namespaces().await.map_err(|e| {
        tracing::error!("list_namespaces failed: {}", e);
        ApiError::from(e)
    })?;
    tracing::debug!("list_namespaces returning {} namespaces", namespaces.len());
    Ok(Json(namespaces))
}

pub async fn get_snapshots(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<Vec<Snapshot>>, ApiError> {
    tracing::debug!("get_snapshots called for app={} namespace={}", app, ns);
    let snapshots = kubectl.get_snapshots(&app, &ns).await.map_err(|e| {
        tracing::error!("get_snapshots failed for app={} ns={}: {}", app, ns, e);
        ApiError::from(e)
    })?;
    tracing::debug!("get_snapshots returning {} snapshots for app={}", snapshots.len(), app);
    Ok(Json(snapshots))
}

pub async fn trigger_backup(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<TaskStatus>, ApiError> {
    tracing::info!("trigger_backup called for app={} namespace={}", app, ns);
    let trigger = format!("backup-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    tracing::debug!("trigger_backup using trigger ID: {}", trigger);
    let app_clone = app.clone();
    let ns_clone = ns.clone();
    let task = kubectl.spawn_backup(app, ns, trigger).await.map_err(|e| {
        tracing::error!("trigger_backup failed for app={} ns={}: {}", app_clone, ns_clone, e);
        ApiError::from(e)
    })?;
    tracing::info!("trigger_backup spawned for app={}", task.app);
    Ok(Json(task))
}

pub async fn trigger_restore(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
    Json(req): Json<RestoreRequest>,
) -> Result<Json<TaskStatus>, ApiError> {
    let app_clone = app.clone();
    let ns_clone = ns.clone();
    let task = kubectl.spawn_restore(app, ns, req.trigger, req.timestamp)
        .await
        .map_err(|e| {
            tracing::error!("trigger_restore failed for app={} ns={}: {}", app_clone, ns_clone, e);
            ApiError::from(e)
        })?;
    Ok(Json(task))
}

pub async fn get_dest_repository(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<Option<String>>, ApiError> {
    let repo = kubectl.get_dest_repository(&app, &ns).await.map_err(|e| {
        tracing::error!("get_dest_repository failed for app={} ns={}: {}", app, ns, e);
        ApiError::from(e)
    })?;
    Ok(Json(repo))
}

pub async fn get_backup_status(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<Option<TaskStatus>>, ApiError> {
    Ok(Json(kubectl.task_status(&app, &ns, "backup").await))
}

pub async fn get_restore_status(
    Path((app, ns)): Path<(String, String)>,
    State(kubectl): State<AppState>,
) -> Result<Json<Option<TaskStatus>>, ApiError> {
    Ok(Json(kubectl.task_status(&app, &ns, "restore").await))
}

pub async fn get_config() -> Json<AppConfig> {
    let interval = std::env::var("REFRESH_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600);
    Json(AppConfig { refresh_interval_secs: interval })
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}
