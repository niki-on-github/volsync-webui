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
    api_group: String,
    token: Option<String>,
}

impl Kubectl {
    pub async fn new() -> Result<Self, KubeError> {
        let base_url = std::env::var("KUBERNETES_SERVICE_HOST")
            .map(|h| format!("https://{}:443", h))
            .unwrap_or_else(|_| "http://localhost:8080".to_string());

        let secret_suffix = std::env::var("VOLSYNC_SECRET_SUFFIX")
            .unwrap_or_else(|_| "-volsync-secret".to_string());

        let api_group = std::env::var("VOLSYNC_API_GROUP")
            .unwrap_or_else(|_| "volsync.backube".to_string());

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

        let client_builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10));

        let ca_cert_path = std::path::Path::new("/var/run/secrets/kubernetes.io/serviceaccount/ca.crt");
        let client = if ca_cert_path.exists() {
            match tokio::fs::read(ca_cert_path).await {
                Ok(ca_cert) => {
                    let cert = reqwest::Certificate::from_pem(&ca_cert)
                        .map_err(|e| KubeError::Api(format!("Failed to parse CA certificate: {}", e)))?;
                    let client = client_builder
                        .add_root_certificate(cert)
                        .build()
                        .map_err(|e| KubeError::Api(format!("Failed to build client with CA: {}", e)))?;
                    tracing::info!("Loaded CA certificate from {}", ca_cert_path.display());
                    client
                }
                Err(e) => {
                    tracing::error!("Failed to read CA certificate: {}, using unauthenticated client", e);
                    tracing::warn!("Kubernetes API calls will not use TLS client certificates");
                    client_builder.build().unwrap_or_else(|_| reqwest::Client::new())
                }
            }
        } else {
            tracing::info!("No CA certificate found at {}, using default client", ca_cert_path.display());
            client_builder.build().unwrap_or_else(|_| reqwest::Client::new())
        };

        Ok(Self { client, base_url, secret_suffix, api_group, token })
    }

    fn secret_name(&self, app: &str) -> String {
        format!("{}{}", app, self.secret_suffix)
    }

    pub async fn check_rbac(&self) {
        let checks: Vec<(&str, String)> = vec![
            (
                "list ReplicationSources",
                format!("/apis/{}/v1alpha1/replicationsources", self.api_group),
            ),
            (
                "list Pods",
                "/api/v1/pods".to_string(),
            ),
            (
                "list Deployments",
                "/apis/apps/v1/deployments".to_string(),
            ),
            (
                "list HelmReleases",
                "/apis/source.toolkit.fluxcd.io/v1beta2/helmreleases".to_string(),
            ),
        ];

        for (name, path) in &checks {
            match self.request_text("GET", path, None).await {
                Ok(_) => tracing::info!("RBAC: {} — OK", name),
                Err(KubeError::Api(msg)) if msg.contains("403") => {
                    tracing::error!(
                        "RBAC: {} — MISSING PERMISSION. Grant access via ClusterRole.",
                        name
                    );
                }
                Err(KubeError::NotFound(_)) => {
                    tracing::warn!("RBAC: {} — SKIPPED (CRD not installed)", name);
                }
                Err(e) => {
                    tracing::warn!("RBAC: {} — SKIPPED ({})", name, e);
                }
            }
        }
    }

    async fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, KubeError> {
        let text = self.request_text(method, path, body).await?;
        serde_json::from_str(&text).map_err(|e| KubeError::Api(e.to_string()))
    }

    async fn request_text(&self, method: &str, path: &str, body: Option<Value>) -> Result<String, KubeError> {
        let url = format!("{}{}", self.base_url, path);

        let max_retries = 3;
        let mut last_err = None;
        for attempt in 0..max_retries {
            match self.do_request(&url, method, body.clone()).await {
                Ok(resp) => {
                    let status = resp.status();

                    if status == reqwest::StatusCode::NOT_FOUND {
                        return Err(KubeError::NotFound(format!("Resource not found at {}", path)));
                    }
                    if status.is_client_error() {
                        let text = resp.text().await.unwrap_or_default();
                        return Err(KubeError::Api(format!(
                            "Client error {} on {}: {}",
                            status, path, text
                        )));
                    }
                    if status.is_server_error() {
                        let text = resp.text().await.unwrap_or_default();
                        if attempt < max_retries - 1 {
                            let delay = Duration::from_secs(1 << attempt);
                            tracing::warn!("Server error {} on {}, retrying in {}s: {}", status, path, delay.as_secs(), text);
                            sleep(delay).await;
                            last_err = Some(KubeError::Api(format!("Server error {} on {}: {}", status, path, text)));
                            continue;
                        }
                        return Err(KubeError::Api(format!(
                            "Server error {} on {}: {}",
                            status, path, text
                        )));
                    }

                    return resp.text().await.map_err(|e| KubeError::Api(e.to_string()));
                }
                Err(e) => {
                    if attempt < max_retries - 1 {
                        let delay = Duration::from_secs(1 << attempt);
                        tracing::warn!("Request attempt {} failed for {}, retrying in {}s: {}", attempt + 1, path, delay.as_secs(), e);
                        sleep(delay).await;
                        last_err = Some(e);
                        continue;
                    }
                    return Err(KubeError::Api(format!(
                        "Request failed after {} retries for {}: {}",
                        max_retries, path, e
                    )));
                }
            }
        }
        Err(last_err.unwrap())
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
        let ns_filter = namespace.unwrap_or("all");
        tracing::debug!("list_apps called with namespace filter: {}", ns_filter);

        let api_path = match namespace {
            Some(ns) => format!("/apis/{}/v1alpha1/namespaces/{}/replicationsources", self.api_group, ns),
            None => format!("/apis/{}/v1alpha1/replicationsources", self.api_group),
        };
        let resp: Value = self.request("GET", &api_path, None).await?;

        let items = resp.get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| KubeError::Api("No items in response".to_string()))?;

        tracing::debug!("list_apps received {} ReplicationSources from API", items.len());

        let apps: Vec<App> = items.iter().filter_map(|item| {
            let name = item.get("metadata")?.get("name")?.as_str()?.to_string();
            let namespace_item = item.get("metadata")?.get("namespace")?.as_str()?.to_string();
            Some(App { name, namespace: namespace_item })
        }).collect();

        tracing::debug!("list_apps returning {} apps", apps.len());
        Ok(apps)
    }

    pub async fn list_namespaces(&self) -> Result<Vec<String>, KubeError> {
        tracing::debug!("list_namespaces called");

        let resp: Value = self.request("GET", "/api/v1/namespaces", None).await?;

        let items = resp.get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| KubeError::Api("No items in response".to_string()))?;

        tracing::debug!("list_namespaces received {} namespaces from API", items.len());

        let mut namespaces: Vec<String> = items.iter()
            .filter_map(|item| item.get("metadata")?.get("name")?.as_str()?.to_string().into())
            .collect();
        namespaces.sort();
        tracing::debug!("list_namespaces returning {} sorted namespaces", namespaces.len());
        Ok(namespaces)
    }

    pub async fn trigger_backup(&self, app: &str, ns: &str, trigger: &str) -> Result<BackupResponse, KubeError> {
        tracing::info!("trigger_backup starting for app={} namespace={}", app, ns);

        let url = format!(
            "/apis/{}/v1alpha1/namespaces/{}/replicationsources/{}",
            self.api_group, ns, app
        );

        let patch = serde_json::json!({
            "spec": {
                "trigger": { "manual": trigger }
            }
        });

        tracing::debug!("trigger_backup PATCHing ReplicationSource with trigger={}", trigger);
        self.request("PATCH", &url, Some(patch)).await?;
        tracing::debug!("trigger_backup ReplicationSource updated, starting poll loop");

        let backup_timeout = backup_timeout_secs();
        let poll_interval = polling_interval_secs();
        tracing::info!("trigger_backup polling with timeout={}s interval={}s", backup_timeout, poll_interval);

        let start = std::time::Instant::now();
        let mut poll_count = 0;
        loop {
            poll_count += 1;
            if start.elapsed() > Duration::from_secs(backup_timeout) {
                tracing::warn!("trigger_backup polling timed out after {} polls for app={}", poll_count, app);
                return Err(KubeError::Timeout("Backup polling timed out".to_string()));
            }
            sleep(Duration::from_secs(poll_interval)).await;

            let rs: Value = self.request("GET", &url, None).await?;

            if let Some(last_sync) = rs.get("status")
                .and_then(|s| s.get("lastManualSync"))
                .and_then(|v| v.as_str()) {
                if last_sync == trigger {
                    tracing::info!("trigger_backup completed on poll #{} for app={}", poll_count, app);
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

            // Check for explicit error conditions in the status
            if let Some(conditions) = rs.get("status")
                .and_then(|s| s.get("conditions"))
                .and_then(|c| c.as_array()) {
                for c in conditions {
                    let cond_type = c.get("type").and_then(|v| v.as_str());
                    let cond_status = c.get("status").and_then(|v| v.as_str());
                    let cond_reason = c.get("reason").and_then(|v| v.as_str());

                    if cond_status == Some("False") && cond_type == Some("Ready") {
                        if let Some(reason) = cond_reason {
                            let is_error = reason.contains("Error") || reason.contains("Failed") || reason == "BackupFailed";
                            if is_error {
                                tracing::warn!("trigger_backup detected failure on poll #{} for app={}: {}", poll_count, app, reason);
                                return Err(KubeError::SnapshotFailed(format!("Backup failed: {}", reason)));
                            }
                        }
                    }
                }
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
                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(e) => return AppBackupStatus {
                        app: app_name.clone(),
                        namespace: app_ns.clone(),
                        success: false,
                        error: Some(format!("Semaphore error: {}", e)),
                    },
                };
                match kubectl_clone.trigger_backup(&app_name, &app_ns, &trigger_owned).await {
                    Ok(r) => {
                        let success = is_successful(&r.result);
                        AppBackupStatus {
                            app: app_name,
                            namespace: app_ns,
                            success,
                            error: if success { None } else { r.result },
                        }
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
        tracing::info!("get_snapshots called for app={} namespace={}", app, ns);

        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| format!("{:x}", d.as_nanos()))
            .unwrap_or_else(|_| "0".to_string());
        let pod_name = format!("volsync-snapshots-{}-{}", app, unique_id);
        let pod_url = format!("/api/v1/namespaces/{}/pods/{}", ns, pod_name);

        tracing::debug!("get_snapshots creating pod {} in namespace {}", pod_name, ns);
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
        tracing::debug!("get_snapshots pod {} created successfully", pod_name);

        let start = std::time::Instant::now();
        let mut poll_count = 0;
        loop {
            poll_count += 1;
            if start.elapsed() > Duration::from_secs(pod_startup_timeout_secs()) {
                tracing::warn!("get_snapshots pod {} timed out after {} polls", pod_name, poll_count);
                if let Err(e) = self.request("DELETE", &pod_url, None).await {
                    tracing::warn!("get_snapshots failed to clean up timed-out pod {}: {}", pod_name, e);
                }
                return Err(KubeError::Timeout("Pod startup timed out".to_string()));
            }

            sleep(Duration::from_secs(polling_interval_secs())).await;

            match self.request("GET", &pod_url, None).await {
                Ok(pod) => {
                    if let Some(phase) = pod.get("status")
                        .and_then(|s| s.get("phase"))
                        .and_then(|v| v.as_str()) {
                        tracing::debug!("get_snapshots poll #{}: pod {} phase={}", poll_count, pod_name, phase);
                        match phase {
                            "Succeeded" => {
                                tracing::info!("get_snapshots pod {} succeeded after {} polls", pod_name, poll_count);
                                break;
                            }
                            "Failed" => {
                                tracing::warn!("get_snapshots pod {} failed after {} polls", pod_name, poll_count);
                                if let Err(e) = self.request("DELETE", &pod_url, None).await {
                                    tracing::warn!("get_snapshots failed to clean up failed pod {}: {}", pod_name, e);
                                }
                                return Err(KubeError::SnapshotFailed("Pod failed".to_string()));
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("get_snapshots poll #{}: pod {} not ready yet: {}", poll_count, pod_name, e);
                }
            }
        }

        let log_url = format!("/api/v1/namespaces/{}/pods/{}/log", ns, pod_name);
        tracing::debug!("get_snapshots fetching logs for pod {}", pod_name);
        let logs = self.request_text("GET", &log_url, None).await?;
        tracing::debug!("get_snapshots fetched {} bytes of logs for pod {}", logs.len(), pod_name);

        // Clean up pod after reading logs — handle error gracefully but log it
        if let Err(e) = self.request("DELETE", &pod_url, None).await {
            tracing::warn!("get_snapshots pod cleanup failed: {}", e);
        } else {
            tracing::debug!("get_snapshots pod {} cleaned up", pod_name);
        }

        let snapshot_count = parse_snapshots(&logs).map(|s| s.len()).unwrap_or(0);
        tracing::info!("get_snapshots returning {} snapshots for app={}", snapshot_count, app);
        parse_snapshots(&logs)
    }

    pub async fn trigger_restore(&self, app: &str, ns: &str, trigger: &str, timestamp: Option<&str>) -> Result<RestoreResponse, KubeError> {
        tracing::info!("trigger_restore starting for app={} namespace={} timestamp={:?}", app, ns, timestamp);

        let hr_url = format!("/apis/source.toolkit.fluxcd.io/v1beta2/namespaces/{}/helmreleases/{}", ns, app);
        if let Err(e) = self.request("PATCH", &hr_url, Some(serde_json::json!({ "spec": { "suspended": true } }))).await {
            tracing::warn!("trigger_restore HelmRelease suspend failed for app={} (non-Flux apps can ignore this): {}", app, e);
        } else {
            tracing::info!("trigger_restore HelmRelease suspended for app={}", app);
        }

        // Scale deployment to 0 — but first read the original replica count so we can restore it
        let deploy_url = format!("/apis/apps/v1/namespaces/{}/deployments/{}", ns, app);
        let original_replicas: Option<i64> = match self.request("GET", &deploy_url, None).await {
            Ok(deploy) => {
                let replicas = deploy.get("spec")
                    .and_then(|s| s.get("replicas"))
                    .and_then(|r| r.as_i64());
                tracing::debug!("trigger_restore read deployment {} with replicas={:?}", app, replicas);
                replicas
            }
            Err(e) => {
                tracing::warn!("trigger_restore Deployment read failed for app={} (non-Flux apps can ignore this): {}", app, e);
                None
            }
        };

        let scale_url = format!("/apis/apps/v1/namespaces/{}/deployments/{}/scale", ns, app);
        if let Err(e) = self.request("PATCH", &scale_url, Some(serde_json::json!({ "spec": { "replicas": 0 } }))).await {
            tracing::warn!("trigger_restore Deployment scale-down failed for app={} (non-Flux apps can ignore this): {}", app, e);
        } else {
            tracing::info!("trigger_restore deployment scaled to 0 for app={}", app);
        }

        let dst_name = format!("{}-dst", app);
        let dst_url = format!(
            "/apis/{}/v1alpha1/namespaces/{}/replicationdestinations/{}",
            self.api_group, ns, dst_name
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

        tracing::debug!("trigger_restore PATCHing ReplicationDestination with trigger={}", trigger);
        if let Err(e) = self.request("PATCH", &dst_url, Some(dst_patch)).await {
            tracing::error!("trigger_restore failed to patch ReplicationDestination, rolling back: {}", e);
            self.resume_restore(app, ns, original_replicas).await;
            return Err(KubeError::Api(format!("Failed to patch ReplicationDestination: {}", e)));
        }
        tracing::info!("trigger_restore ReplicationDestination updated, starting poll loop");

        let restore_timeout = restore_timeout_secs();
        let poll_interval = polling_interval_secs();
        tracing::info!("trigger_restore polling with timeout={}s interval={}s", restore_timeout, poll_interval);

        let start = std::time::Instant::now();
        let mut poll_count = 0;
        loop {
            poll_count += 1;
            if start.elapsed() > Duration::from_secs(restore_timeout) {
                // Resume deployment and HelmRelease before returning error
                tracing::warn!("trigger_restore polling timed out after {} polls for app={}", poll_count, app);
                self.resume_restore(app, ns, original_replicas).await;
                return Err(KubeError::Timeout("Restore polling timed out".to_string()));
            }
            sleep(Duration::from_secs(poll_interval)).await;

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

                    let finished = result.as_deref().map_or(true, |r| r.eq_ignore_ascii_case("successful"));
                    if finished {
                        tracing::info!("trigger_restore completed on poll #{} for app={}", poll_count, app);
                        // Resume deployment and HelmRelease
                        self.resume_restore(app, ns, original_replicas).await;
                        return Ok(RestoreResponse {
                            trigger: trigger.to_string(),
                            status: "completed".to_string(),
                            result,
                        });
                    } else {
                        // Restore didn't succeed — resume and return error
                        tracing::warn!("trigger_restore failed on poll #{} for app={}: result={:?}", poll_count, app, result);
                        self.resume_restore(app, ns, original_replicas).await;
                        return Err(KubeError::SnapshotFailed(format!(
                            "Restore failed with result: {:?}", result
                        )));
                    }
                }
            }

            if poll_count % 15 == 0 {
                tracing::debug!("trigger_restore poll #{} for app={} (still waiting)", poll_count, app);
            }
        }
    }

    /// Resume the deployment and HelmRelease to their original state after restore completes (success or failure)
    async fn resume_restore(&self, app: &str, ns: &str, original_replicas: Option<i64>) {
        tracing::debug!("resume_restore called for app={} replicas={:?}", app, original_replicas);

        let scale_url = format!("/apis/apps/v1/namespaces/{}/deployments/{}/scale", ns, app);
        let replicas = original_replicas.unwrap_or(1);
        if let Err(e) = self.request("PATCH", &scale_url, Some(serde_json::json!({ "spec": { "replicas": replicas } }))).await {
            tracing::warn!("resume_restore failed to scale deployment {} back to {} replicas: {}", app, replicas, e);
        } else {
            tracing::info!("resume_restore scaled deployment {} back to {} replicas", app, replicas);
        }

        let hr_url = format!("/apis/source.toolkit.fluxcd.io/v1beta2/namespaces/{}/helmreleases/{}", ns, app);
        if let Err(e) = self.request("PATCH", &hr_url, Some(serde_json::json!({ "spec": { "suspended": false } }))).await {
            tracing::warn!("resume_restore failed to unsuspend HelmRelease {}: {}", app, e);
        } else {
            tracing::info!("resume_restore HelmRelease resumed for app={}", app);
        }
    }
}

fn is_successful(result: &Option<String>) -> bool {
    result.as_deref().map_or(false, |r| r.eq_ignore_ascii_case("successful"))
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
