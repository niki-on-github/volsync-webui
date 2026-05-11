use crate::models::{App, AppBackupStatus, BackupResponse, RestoreResponse, Snapshot};
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio::time::sleep;

// Configurable timeouts via environment variables (all in seconds)
fn backup_timeout_secs() -> u64 {
    std::env::var("POLL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300)
}

fn restore_timeout_secs() -> u64 {
    std::env::var("POLL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300)
}

fn pod_startup_timeout_secs() -> u64 {
    std::env::var("POD_STARTUP_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60)
}

fn polling_interval_secs() -> u64 {
    std::env::var("POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
}

#[derive(Debug)]
pub enum KubeError {
    Api(String),
    Timeout(String),
    SnapshotFailed(String),
    NotFound(String),
    InvalidMethod(String),
}

impl std::fmt::Display for KubeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KubeError::Api(s) => write!(f, "Api error: {}", s),
            KubeError::Timeout(s) => write!(f, "Timeout: {}", s),
            KubeError::SnapshotFailed(s) => write!(f, "Snapshot failed: {}", s),
            KubeError::NotFound(s) => write!(f, "Not found: {}", s),
            KubeError::InvalidMethod(s) => write!(f, "Invalid HTTP method: {}", s),
        }
    }
}

impl std::error::Error for KubeError {}

#[derive(Clone)]
pub struct Kubectl {
    client: Client,
    base_url: String,
    secret_suffix: String,
    token: Option<String>,
}

impl Kubectl {
    pub async fn new() -> Result<Self, KubeError> {
        let client = Client::new();

        let base_url = std::env::var("KUBERNETES_SERVICE_HOST")
            .map(|h| format!("https://{}:443", h))
            .unwrap_or_else(|_| "http://localhost:8080".to_string());

        let secret_suffix = std::env::var("VOLSYNC_SECRET_SUFFIX")
            .unwrap_or_else(|_| "-volsync-secret".to_string());

        // Read ServiceAccount token if available (in-cluster deployment)
        let token_path = std::path::Path::new("/var/run/secrets/kubernetes.io/serviceaccount/token");
        let token = if token_path.exists() {
            match tokio::fs::read_to_string(token_path).await {
                Ok(t) => {
                    tracing::info!("Loaded ServiceAccount token from {}", token_path.display());
                    Some(t)
                }
                Err(e) => {
                    tracing::warn!("Failed to read ServiceAccount token: {}", e);
                    None
                }
            }
        } else {
            tracing::info!("No ServiceAccount token found; running outside cluster or without RBAC");
            None
        };

        Ok(Self { client, base_url, secret_suffix, token })
    }

    fn secret_name(&self, app: &str) -> String {
        format!("{}{}", app, self.secret_suffix)
    }

    async fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, KubeError> {
        let url = format!("{}{}", self.base_url, path);

        // Retry logic for transient failures (Bug #10 fix) - 1 retry with 1s delay
        let resp = match self.do_request(&url, method, body.clone()).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Request attempt 1 failed for {}: {}, retrying...", path, e);
                sleep(Duration::from_secs(1)).await;
                self.do_request(&url, method, body)
                    .await
                    .map_err(|e| KubeError::Api(format!("Retry failed for {}: {}", path, e)))?
            }
        };

        // Check all error status codes, not just 404 (Bug #5 fix)
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(KubeError::NotFound(format!("Resource not found at {}", path)));
        }
        if status.is_client_error() {
            let body = resp.text().await.unwrap_or_default();
            return Err(KubeError::Api(format!(
                "Client error {} on {}: {}",
                status, path, body
            )));
        }
        if status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            return Err(KubeError::Api(format!(
                "Server error {} on {}: {}",
                status, path, body
            )));
        }

        resp.json::<Value>().await.map_err(|e| KubeError::Api(e.to_string()))
    }

    async fn do_request(&self, url: &str, method: &str, body: Option<Value>) -> Result<reqwest::Response, KubeError> {
        let method_value = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| KubeError::InvalidMethod(format!("Invalid HTTP method: {}", method)))?;

        let mut req = self.client.request(method_value, url);

        // Attach ServiceAccount token if available (Bug #1 fix)
        if let Some(ref t) = self.token {
            req = req.header("Authorization", format!("Bearer {}", t));
        }

        if let Some(b) = body {
            req = req.header("Content-Type", "application/json").json(&b);
        }

        req.send().await.map_err(|e| KubeError::Api(e.to_string()))
    }

    pub async fn list_apps(&self, namespace: Option<&str>) -> Result<Vec<App>, KubeError> {
        let resp: Value = self.request("GET", "/apis/replication.storage.io/v1alpha1/replicationsources", None).await?;

        let items = resp.get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| KubeError::Api("No items in response".to_string()))?;

        let apps = items.iter().filter_map(|item| {
            let name = item.get("metadata")?.get("name")?.as_str()?.to_string();
            let namespace_item = item.get("metadata")?.get("namespace")?.as_str()?.to_string();
            // Filter by namespace if specified
            if let Some(ns) = namespace {
                if namespace_item != ns {
                    return None;
                }
            }
            Some(App { name, namespace: namespace_item })
        }).collect();

        Ok(apps)
    }

    pub async fn list_namespaces(&self) -> Result<Vec<String>, KubeError> {
        let resp: Value = self.request("GET", "/api/v1/namespaces", None).await?;

        let items = resp.get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| KubeError::Api("No items in response".to_string()))?;

        let mut namespaces: Vec<String> = items.iter()
            .filter_map(|item| item.get("metadata")?.get("name")?.as_str()?.to_string().into())
            .collect();
        namespaces.sort();
        Ok(namespaces)
    }

    pub async fn trigger_backup(&self, app: &str, ns: &str, trigger: &str) -> Result<BackupResponse, KubeError> {
        let url = format!(
            "/apis/replication.storage.io/v1alpha1/namespaces/{}/replicationsources/{}",
            ns, app
        );

        let patch = serde_json::json!({
            "spec": {
                "trigger": { "manual": trigger }
            }
        });

        self.request("PATCH", &url, Some(patch)).await?;

        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > Duration::from_secs(backup_timeout_secs()) {
                return Err(KubeError::Timeout("Backup polling timed out".to_string()));
            }
            sleep(Duration::from_secs(polling_interval_secs())).await;

            let rs: Value = self.request("GET", &url, None).await?;

            if let Some(last_sync) = rs.get("status")
                .and_then(|s| s.get("lastManualSync"))
                .and_then(|v| v.as_str()) {
                if last_sync == trigger {
                    let result = rs.get("status")
                        .and_then(|s| s.get("latestMoverStatus"))
                        .and_then(|m| m.get("result"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    return Ok(BackupResponse {
                        trigger: trigger.to_string(),
                        status: "completed".to_string(),
                        result,
                    });
                }
            }

            // Check for error/failed conditions explicitly (Bug #13 fix)
            let conditions = rs.get("status")
                .and_then(|s| s.get("conditions"))
                .and_then(|c| c.as_array());

            if let Some(arr) = conditions {
                // Check for Error or Failed type conditions first
                for c in arr.iter() {
                    let cond_type = c.get("type").and_then(|v| v.as_str());
                    let cond_status = c.get("status").and_then(|v| v.as_str());
                    let cond_reason = c.get("reason").and_then(|v| v.as_str());
                    let cond_message = c.get("message").and_then(|v| v.as_str());

                    // If any condition has a Failure/Error status, return error
                    if cond_status == Some("False") && (cond_type == Some("Ready") || cond_type == Some("Synchronizing")) {
                        let result = rs.get("status")
                            .and_then(|s| s.get("latestMoverStatus"))
                            .and_then(|m| m.get("result"))
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        return Err(KubeError::SnapshotFailed(format!(
                            "Backup failed: {:?} - {:?} - {:?}",
                            cond_reason, cond_message, result
                        )));
                    }
                }

                // Map raw Synchronizing condition reason to user-friendly status
                let status = arr.iter()
                    .find(|c| c.get("type") == Some(&serde_json::json!("Synchronizing")))
                    .and_then(|c| c.get("reason"))
                    .and_then(|v| v.as_str());

                match status {
                    Some("SyncInProgress") | Some("Pending") => continue,
                    Some(reason) => {
                        let result = rs.get("status")
                            .and_then(|s| s.get("latestMoverStatus"))
                            .and_then(|m| m.get("result"))
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        // Return the human-readable reason as status instead of raw K8s condition reason
                        return Ok(BackupResponse {
                            trigger: trigger.to_string(),
                            status: reason.to_string(),
                            result,
                        });
                    }
                    None => continue,
                }
            } else {
                continue;
            }
        }
    }

    pub async fn trigger_backup_all(&self, trigger: &str) -> Result<Vec<AppBackupStatus>, KubeError> {
        let apps = self.list_apps(None).await?;
        let kubectl = self.clone();

        // Concurrency limit to prevent overwhelming the K8s API server (Bug #7 fix)
        let max_concurrent: usize = std::env::var("BACKUP_ALL_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        tracing::info!("Starting backup-all for {} apps with max {} concurrent tasks", apps.len(), max_concurrent);

        // Run backups concurrently with a semaphore to limit concurrency
        let handles: Vec<JoinHandle<AppBackupStatus>> = apps.into_iter().map(|app| {
            let app_name = app.name.clone();
            let app_ns = app.namespace.clone();
            let kubectl_clone = kubectl.clone();
            let trigger_owned = trigger.to_string();
            let sem = semaphore.clone();
            tokio::spawn(async move {
                // Acquire semaphore permit before starting backup
                let _permit = sem.acquire().await.expect("semaphore closed");
                match kubectl_clone.trigger_backup(&app_name, &app_ns, &trigger_owned).await {
                    Ok(r) => AppBackupStatus {
                        app: app_name,
                        namespace: app_ns,
                        success: r.result.as_deref() == Some("Successful"),
                        error: if r.result.as_deref() == Some("Successful") { None } else { r.result },
                    },
                    Err(e) => AppBackupStatus {
                        app: app_name,
                        namespace: app_ns,
                        success: false,
                        error: Some(e.to_string()),
                    },
                }
            })
        }).collect();

        // Await all concurrent tasks
        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(status) => results.push(status),
                Err(e) => results.push(AppBackupStatus {
                    app: "unknown".to_string(),
                    namespace: "".to_string(),
                    success: false,
                    error: Some(format!("Task panicked: {}", e)),
                }),
            }
        }
        Ok(results)
    }

    pub async fn get_snapshots(&self, app: &str, ns: &str) -> Result<Vec<Snapshot>, KubeError> {
        // Use timestamp-based suffix to prevent race conditions between concurrent requests (Bug #12 fix)
        let random_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| format!("{:08x}", d.subsec_nanos()))
            .unwrap_or_else(|_| "00000000".to_string());
        let pod_name = format!("volsync-snapshots-{}-{}", app, random_suffix);
        let pod_url = format!("/api/v1/namespaces/{}/pods/{}", ns, pod_name);

        let create_url = format!("/api/v1/namespaces/{}/pods", ns);
        let pod_manifest = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": pod_name, "namespace": ns },
            "spec": {
                "restartPolicy": "Never",
                "containers": [{
                    "name": "restic",
                    "image": "restic/restic:latest",
                    "args": ["snapshots", "--json"],
                    "envFrom": [{ "secretRef": { "name": self.secret_name(app) }}]
                }]
            }
        });

        let _create: Value = self.request("POST", &create_url, Some(pod_manifest)).await?;

        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > Duration::from_secs(pod_startup_timeout_secs()) {
                // Clean up failed pod before returning error
                let _ = self.request("DELETE", &pod_url, None).await;
                return Err(KubeError::Timeout("Pod startup timed out".to_string()));
            }

            sleep(Duration::from_secs(polling_interval_secs())).await;

            match self.request("GET", &pod_url, None).await {
                Ok(pod) => {
                    if let Some(phase) = pod.get("status")
                        .and_then(|s| s.get("phase"))
                        .and_then(|v| v.as_str()) {
                        match phase {
                            "Succeeded" => break,
                            "Failed" => {
                                // Clean up failed pod before returning error
                                let _ = self.request("DELETE", &pod_url, None).await;
                                return Err(KubeError::SnapshotFailed("Pod failed".to_string()));
                            }
                            _ => {}
                        }
                    }
                }
                Err(_) => {}
            }
        }

        let log_url = format!("/api/v1/namespaces/{}/pods/{}/log", ns, pod_name);
        // Pod logs are raw text output from restic CLI, NOT JSON lines.
        // Restic outputs one JSON object per line when --json flag is used.
        let logs: String = self.request("GET", &log_url, None).await?.to_string();

        // Clean up pod after reading logs — handle error gracefully but log it
        if let Err(e) = self.request("DELETE", &pod_url, None).await {
            tracing::warn!("Pod cleanup failed: {}", e);
        }

        parse_snapshots(&logs)
    }

    pub async fn trigger_restore(&self, app: &str, ns: &str, trigger: &str, timestamp: Option<&str>) -> Result<RestoreResponse, KubeError> {
        // Suspend HelmRelease — propagate error if it fails (resource may not exist for non-Flux apps)
        let hr_url = format!("/apis/source.toolkit.fluxcd.io/v1beta2/namespaces/{}/helmreleases/{}", ns, app);
        if let Err(e) = self.request("PATCH", &hr_url, Some(serde_json::json!({ "spec": { "suspended": true } }))).await {
            tracing::debug!("HelmRelease suspend skipped (may not exist): {}", e);
        }

        // Scale deployment to 0 — but first read the original replica count so we can restore it
        let deploy_url = format!("/apis/apps/v1/namespaces/{}/deployments/{}", ns, app);
        let original_replicas: Option<i64> = match self.request("GET", &deploy_url, None).await {
            Ok(deploy) => deploy.get("spec")
                .and_then(|s| s.get("replicas"))
                .and_then(|r| r.as_i64()),
            Err(e) => {
                tracing::debug!("Deployment read skipped (may not exist): {}", e);
                None
            }
        };

        let scale_url = format!("/apis/apps/v1/namespaces/{}/deployments/{}/scale", ns, app);
        if let Err(e) = self.request("PATCH", &scale_url, Some(serde_json::json!({ "spec": { "replicas": 0 } }))).await {
            tracing::debug!("Deployment scale-down skipped (may not exist): {}", e);
        }

        let dst_name = format!("{}-dst", app);
        let dst_url = format!(
            "/apis/replication.storage.io/v1alpha1/namespaces/{}/replicationdestinations/{}",
            ns, dst_name
        );

        let mut restic_spec = serde_json::json!({});
        if let Some(ts) = timestamp {
            if !ts.is_empty() {
                restic_spec["restoreAsOf"] = serde_json::json!(ts);
            }
        }

        let dst_patch = serde_json::json!({
            "spec": {
                "trigger": { "manual": trigger },
                "restic": restic_spec
            }
        });

        self.request("PATCH", &dst_url, Some(dst_patch)).await?;

        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > Duration::from_secs(restore_timeout_secs()) {
                // Resume deployment and HelmRelease before returning error
                self.resume_restore(app, ns, original_replicas).await;
                return Err(KubeError::Timeout("Restore polling timed out".to_string()));
            }
            sleep(Duration::from_secs(polling_interval_secs())).await;

            let dst: Value = self.request("GET", &dst_url, None).await?;

            if let Some(last_sync) = dst.get("status")
                .and_then(|s| s.get("lastManualSync"))
                .and_then(|v| v.as_str()) {
                if last_sync == trigger {
                    // Verify mover status before reporting completed
                    let result = dst.get("status")
                        .and_then(|s| s.get("latestMoverStatus"))
                        .and_then(|m| m.get("result"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    // Only report completed if mover status is Successful or None (no mover ran)
                    let finished = result.as_deref() == Some("Successful") || result.is_none();
                    if finished {
                        // Resume deployment and HelmRelease
                        self.resume_restore(app, ns, original_replicas).await;
                        return Ok(RestoreResponse {
                            trigger: trigger.to_string(),
                            status: "completed".to_string(),
                            result,
                        });
                    } else {
                        // Restore didn't succeed — resume and return error
                        self.resume_restore(app, ns, original_replicas).await;
                        return Err(KubeError::SnapshotFailed(format!(
                            "Restore failed with result: {:?}", result
                        )));
                    }
                }
            }
        }
    }

    /// Resume the deployment and HelmRelease to their original state after restore completes (success or failure)
    async fn resume_restore(&self, app: &str, ns: &str, original_replicas: Option<i64>) {
        let scale_url = format!("/apis/apps/v1/namespaces/{}/deployments/{}/scale", ns, app);
        let replicas = original_replicas.unwrap_or(1);
        let _ = self.request("PATCH", &scale_url, Some(serde_json::json!({ "spec": { "replicas": replicas } }))).await;

        let hr_url = format!("/apis/source.toolkit.fluxcd.io/v1beta2/namespaces/{}/helmreleases/{}", ns, app);
        let _ = self.request("PATCH", &hr_url, Some(serde_json::json!({ "spec": { "suspended": false } }))).await;
    }
}

fn parse_snapshots(logs: &str) -> Result<Vec<Snapshot>, KubeError> {
    let mut snapshots = Vec::new();
    for line in logs.lines() {
        // Restic --json outputs one JSON object per line, but may also output non-JSON
        // status messages (e.g., "scanning...", error messages). Skip non-JSON lines.
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(snap) = serde_json::from_str::<Value>(line) {
            let id = snap.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let time_str = snap.get("time").and_then(|v| v.as_str()).unwrap_or("");
            let tags: Vec<String> = snap.get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            if !id.is_empty() {
                snapshots.push(Snapshot { id, time: time_str.to_string(), tags });
            }
        }
    }
    Ok(snapshots)
}
