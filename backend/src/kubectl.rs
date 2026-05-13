use crate::models::{App, BackupResponse, RestoreResponse, Snapshot, TaskStatus};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
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
        .unwrap_or(900)
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
    api_group: String,
    source_suffix: String,
    dest_suffix: String,
    token: Option<String>,
    pub tasks: Arc<RwLock<HashMap<String, TaskStatus>>>,
}

impl Kubectl {
    pub async fn new() -> Result<Self, KubeError> {
        let host = std::env::var("KUBERNETES_SERVICE_HOST")
            .unwrap_or_default();
        let port = std::env::var("KUBERNETES_SERVICE_PORT")
            .unwrap_or_else(|_| if host.is_empty() { "8080".to_string() } else { "443".to_string() });
        let base_url = if host.is_empty() {
            format!("http://localhost:{}", port)
        } else if host.contains(':') {
            format!("https://[{}]:{}", host, port)
        } else {
            format!("https://{}:{}", host, port)
        };

        let api_group = std::env::var("VOLSYNC_API_GROUP")
            .unwrap_or_else(|_| "volsync.backube".to_string());

        let source_suffix = std::env::var("VOLSYNC_SOURCE_SUFFIX")
            .unwrap_or_else(|_| "-backup".to_string());
        let dest_suffix = std::env::var("VOLSYNC_DEST_SUFFIX")
            .unwrap_or_else(|_| "-bootstrap".to_string());

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
                    client_builder.build().unwrap_or_else(|_| {
                        reqwest::Client::builder()
                            .timeout(Duration::from_secs(30))
                            .connect_timeout(Duration::from_secs(10))
                            .build()
                            .unwrap_or_else(|_| reqwest::Client::new())
                    })
                }
            }
        } else {
            tracing::info!("No CA certificate found at {}, using default client", ca_cert_path.display());
            client_builder.build().unwrap_or_else(|_| {
                reqwest::Client::builder()
                    .timeout(Duration::from_secs(30))
                    .connect_timeout(Duration::from_secs(10))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new())
            })
        };

        Ok(Self { client, base_url, api_group, source_suffix, dest_suffix, token, tasks: Arc::new(RwLock::new(HashMap::new())) })
    }

    fn dest_crd_name(&self, app: &str) -> String {
        let base = app.strip_suffix(&self.source_suffix).unwrap_or(app);
        format!("{}{}", base, self.dest_suffix)
    }

    fn app_base_name(&self, app: &str) -> String {
        app.strip_suffix(&self.source_suffix)
            .filter(|s| !s.is_empty())
            .unwrap_or(app)
            .to_string()
    }

    async fn find_app_deployments(&self, base: &str, ns: &str) -> Vec<String> {
        let list_url = format!(
            "/apis/apps/v1/namespaces/{}/deployments?labelSelector=app.kubernetes.io/instance={}",
            ns, base
        );
        match self.request("GET", &list_url, None).await {
            Ok(resp) => {
                let names: Vec<String> = resp.get("items")
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.get("metadata")?.get("name")?.as_str().map(String::from))
                    .collect();
                tracing::debug!("find_app_deployments found {} deployments for app={} in ns={}", names.len(), base, ns);
                names
            }
            Err(e) => {
                tracing::warn!("find_app_deployments failed for app={} in ns={} (non-Flux apps can ignore this): {}", base, ns, e);
                Vec::new()
            }
        }
    }

    async fn read_deployments_replicas(&self, names: &[String], ns: &str) -> HashMap<String, i64> {
        let mut map = HashMap::new();
        for name in names {
            let url = format!("/apis/apps/v1/namespaces/{}/deployments/{}", ns, name);
            match self.request("GET", &url, None).await {
                Ok(deploy) => {
                    let replicas = deploy.get("spec")
                        .and_then(|s| s.get("replicas"))
                        .and_then(|r| r.as_i64())
                        .unwrap_or(1);
                    tracing::debug!("read_deployments_replicas: deployment {} has {} replicas", name, replicas);
                    map.insert(name.clone(), replicas);
                }
                Err(e) => {
                    tracing::warn!("read_deployments_replicas: failed to read deployment {}: {}", name, e);
                    map.insert(name.clone(), 1);
                }
            }
        }
        map
    }

    async fn scale_deployments(&self, names: &[String], ns: &str, replicas: i64) {
        for name in names {
            let url = format!("/apis/apps/v1/namespaces/{}/deployments/{}/scale", ns, name);
            if let Err(e) = self.request("PATCH", &url, Some(serde_json::json!({ "spec": { "replicas": replicas } }))).await {
                tracing::warn!("scale_deployments: failed to scale deployment {} to {}: {}", name, replicas, e);
            } else {
                tracing::info!("scale_deployments: deployment {} scaled to {}", name, replicas);
            }
        }
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
                "/apis/helm.toolkit.fluxcd.io/v2/helmreleases".to_string(),
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
                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        if attempt < max_retries - 1 {
                            let delay = Duration::from_secs(1 << attempt);
                            tracing::warn!("Rate limited (429) on {}, retrying in {}s", path, delay.as_secs());
                            sleep(delay).await;
                            last_err = Some(KubeError::Api(format!("Rate limited on {}", path)));
                            continue;
                        }
                        return Err(KubeError::Api(format!("Rate limited on {} after {} retries", path, max_retries)));
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
            let content_type = if method == "PATCH" {
                "application/merge-patch+json"
            } else {
                "application/json"
            };
            req = req.header("Content-Type", content_type).json(&b);
        }

        req.send().await.map_err(|e| KubeError::Api(e.to_string()))
    }

    pub async fn list_apps(&self) -> Result<Vec<App>, KubeError> {
        let api_path = format!("/apis/{}/v1alpha1/replicationsources", self.api_group);
        let resp: Value = self.request("GET", &api_path, None).await?;

        let items = resp.get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| KubeError::Api("No items in response".to_string()))?;

        tracing::debug!("list_apps received {} ReplicationSources from API", items.len());

        let mut apps: Vec<App> = items.iter().filter_map(|item| {
            let name = item.get("metadata")?.get("name")?.as_str()?.to_string();
            let namespace_item = item.get("metadata")?.get("namespace")?.as_str()?.to_string();
            let status = item.get("status");
            let spec = item.get("spec");

            let last_sync_time = status
                .and_then(|s| s.get("lastSyncTime"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let last_sync_duration = match status
                .and_then(|s| s.get("lastSyncDuration"))
                .and_then(|v| v.as_str())
            {
                Some(s) => match s.trim_end_matches('s').parse::<f64>() {
                    Ok(d) => Some(format!("{:.1}s", d)),
                    Err(e) => {
                        tracing::warn!("Failed to parse lastSyncDuration '{}' for app '{}': {}", s, name, e);
                        None
                    }
                },
                None => None,
            };
            let last_result = status
                .and_then(|s| s.get("latestMoverStatus"))
                .and_then(|m| m.get("result"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let next_sync_time = status
                .and_then(|s| s.get("nextSyncTime"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let in_progress = status
                .and_then(|s| s.get("conditions"))
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter().any(|c| {
                        c.get("type").and_then(|v| v.as_str()) == Some("Synchronizing")
                            && c.get("status").and_then(|v| v.as_str()) == Some("True")
                    })
                })
                .unwrap_or(false);
            let paused = spec
                .and_then(|s| s.get("paused"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let repository = spec
                .and_then(|s| s.get("restic"))
                .and_then(|r| r.get("repository"))
                .and_then(|v| v.as_str())
                .map(String::from);

            Some(App {
                name,
                namespace: namespace_item,
                last_sync_time,
                last_sync_duration,
                last_result,
                next_sync_time,
                in_progress,
                paused,
                repository,
                backup_pending: false,
                restore_pending: false,
            })
        }).collect();

        // Cross-reference with active background tasks
        let tasks = self.tasks.read().await;
        for app in &mut apps {
            let bk = format!("{}/{}/backup", app.namespace, app.name);
            if let Some(t) = tasks.get(&bk) {
                if t.status == "pending" || t.status == "running" {
                    app.backup_pending = true;
                }
            }
            let rk = format!("{}/{}/restore", app.namespace, app.name);
            if let Some(t) = tasks.get(&rk) {
                if t.status == "pending" || t.status == "running" {
                    app.restore_pending = true;
                }
            }
        }
        drop(tasks);

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
                self.clear_manual_trigger(&url).await;
                return Err(KubeError::Timeout("Backup polling timed out".to_string()));
            }
            sleep(Duration::from_secs(poll_interval)).await;

            let rs: Value = self.request("GET", &url, None).await?;

            if let Some(last_sync) = rs.get("status")
                .and_then(|s| s.get("lastManualSync"))
                .and_then(|v| v.as_str()) {
                if last_sync == trigger {
                    if let Some(result) = rs.get("status")
                        .and_then(|s| s.get("latestMoverStatus"))
                        .and_then(|m| m.get("result"))
                        .and_then(|v| v.as_str())
                        .map(String::from) {
                        tracing::info!("trigger_backup completed on poll #{} for app={}", poll_count, app);
                        self.clear_manual_trigger(&url).await;
                        return Ok(BackupResponse {
                            trigger: trigger.to_string(),
                            status: "completed".to_string(),
                            result: Some(result),
                        });
                    }
                    // mover hasn't reported result yet, keep polling
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
                                self.clear_manual_trigger(&url).await;
                                return Err(KubeError::SnapshotFailed(format!("Backup failed: {}", reason)));
                            }
                        }
                    }
                }
            }
        }
    }

    async fn clear_manual_trigger(&self, url: &str) {
        if let Err(e) = self.request("PATCH", url, Some(serde_json::json!({
            "spec": { "trigger": { "manual": serde_json::Value::Null } }
        }))).await {
            tracing::warn!("clear_manual_trigger failed: {}", e);
        }
    }

    pub async fn spawn_backup(self: Arc<Self>, app: String, ns: String, trigger: String) -> Result<TaskStatus, KubeError> {
        let task_key = format!("{}/{}/backup", ns, app);
        let restore_key = format!("{}/{}/restore", ns, app);

        {
            let mut tasks = self.tasks.write().await;
            if let Some(existing) = tasks.get(&task_key) {
                if existing.status == "pending" || existing.status == "running" {
                    return Err(KubeError::Api(format!("Backup already in progress for {}", app)));
                }
            }
            if let Some(restore) = tasks.get(&restore_key) {
                if restore.status == "pending" || restore.status == "running" {
                    return Err(KubeError::Api(format!("Backup cannot start while restore is in progress for {}", app)));
                }
            }
            tasks.remove(&task_key);
        }

        let task = TaskStatus {
            task_type: "backup".to_string(),
            app: app.clone(),
            namespace: ns.clone(),
            status: "pending".to_string(),
            result: None,
            error: None,
            started_at: chrono::Utc::now().to_rfc3339(),
        };
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_key.clone(), task.clone());
        }

        let me = self.clone();
        tokio::spawn(async move {
            {
                let mut tasks = me.tasks.write().await;
                if let Some(t) = tasks.get_mut(&task_key) {
                    t.status = "running".to_string();
                }
            }

            match me.trigger_backup(&app, &ns, &trigger).await {
                Ok(resp) => {
                    let mut tasks = me.tasks.write().await;
                    if let Some(t) = tasks.get_mut(&task_key) {
                        t.status = "completed".to_string();
                        t.result = resp.result;
                    }
                    let cleanup = me.clone();
                    let ck = task_key.clone();
                    tokio::spawn(async move {
                        sleep(Duration::from_secs(30)).await;
                        cleanup.tasks.write().await.remove(&ck);
                    });
                }
                Err(e) => {
                    let mut tasks = me.tasks.write().await;
                    if let Some(t) = tasks.get_mut(&task_key) {
                        t.status = "failed".to_string();
                        t.error = Some(e.to_string());
                    }
                    let cleanup = me.clone();
                    let ck = task_key.clone();
                    tokio::spawn(async move {
                        sleep(Duration::from_secs(30)).await;
                        cleanup.tasks.write().await.remove(&ck);
                    });
                }
            }
        });

        Ok(task)
    }

    pub async fn spawn_restore(self: Arc<Self>, app: String, ns: String, timestamp: Option<String>) -> Result<TaskStatus, KubeError> {
        let task_key = format!("{}/{}/restore", ns, app);
        let backup_key = format!("{}/{}/backup", ns, app);

        {
            let mut tasks = self.tasks.write().await;
            if let Some(existing) = tasks.get(&task_key) {
                if existing.status == "pending" || existing.status == "running" {
                    return Err(KubeError::Api(format!("Restore already in progress for {}", app)));
                }
            }
            if let Some(backup) = tasks.get(&backup_key) {
                if backup.status == "pending" || backup.status == "running" {
                    return Err(KubeError::Api(format!("Restore cannot start while backup is in progress for {}", app)));
                }
            }
            tasks.remove(&task_key);
        }

        let task = TaskStatus {
            task_type: "restore".to_string(),
            app: app.clone(),
            namespace: ns.clone(),
            status: "pending".to_string(),
            result: None,
            error: None,
            started_at: chrono::Utc::now().to_rfc3339(),
        };
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_key.clone(), task.clone());
        }

        let me = self.clone();
        tokio::spawn(async move {
            {
                let mut tasks = me.tasks.write().await;
                if let Some(t) = tasks.get_mut(&task_key) {
                    t.status = "running".to_string();
                }
            }

            match me.trigger_restore(&app, &ns, timestamp.as_deref()).await {
                Ok(resp) => {
                    let mut tasks = me.tasks.write().await;
                    if let Some(t) = tasks.get_mut(&task_key) {
                        t.status = "completed".to_string();
                        t.result = resp.result;
                    }
                    let cleanup = me.clone();
                    let ck = task_key.clone();
                    tokio::spawn(async move {
                        sleep(Duration::from_secs(30)).await;
                        cleanup.tasks.write().await.remove(&ck);
                    });
                }
                Err(e) => {
                    let mut tasks = me.tasks.write().await;
                    if let Some(t) = tasks.get_mut(&task_key) {
                        t.status = "failed".to_string();
                        t.error = Some(e.to_string());
                    }
                    let cleanup = me.clone();
                    let ck = task_key.clone();
                    tokio::spawn(async move {
                        sleep(Duration::from_secs(30)).await;
                        cleanup.tasks.write().await.remove(&ck);
                    });
                }
            }
        });

        Ok(task)
    }

    pub async fn task_status(&self, app: &str, ns: &str, task_type: &str) -> Option<TaskStatus> {
        let key = format!("{}/{}/{}", ns, app, task_type);
        let tasks = self.tasks.read().await;
        tasks.get(&key).cloned()
    }

    pub async fn get_snapshots(&self, app: &str, ns: &str) -> Result<Vec<Snapshot>, KubeError> {
        tracing::info!("get_snapshots called for app={} namespace={}", app, ns);

        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| format!("{:x}", d.as_nanos()))
            .unwrap_or_else(|_| "0".to_string());
        let pod_name = format!("volsync-snapshots-{}-{}", app, unique_id);
        let pod_url = format!("/api/v1/namespaces/{}/pods/{}", ns, pod_name);

        // Read the actual secret name from the ReplicationSource's spec.restic.repository
        let rs_url = format!(
            "/apis/{}/v1alpha1/namespaces/{}/replicationsources/{}",
            self.api_group, ns, app
        );
        let rs: Value = self.request("GET", &rs_url, None).await?;
        let secret_name = rs
            .get("spec")
            .and_then(|s| s.get("restic"))
            .and_then(|r| r.get("repository")) // spec.restic.repository in VolSync's ReplicationSource CRD IS the secret name, not a URL
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                KubeError::Api(format!(
                    "spec.restic.repository not found in ReplicationSource {}/{}",
                    ns, app
                ))
            })?;
        tracing::debug!("get_snapshots using secret {} for app={}", secret_name, app);

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
                    "image": std::env::var("RESTIC_IMAGE").unwrap_or_else(|_| "restic/restic:latest".to_string()),
                    "args": ["snapshots", "--json"],
                    "envFrom": [{ "secretRef": { "name": secret_name }}]
                }]
            }
        });

        let _create: Value = self.request("POST", &create_url, Some(pod_manifest)).await?;
        tracing::debug!("get_snapshots pod {} created successfully", pod_name);

        let mut guard = PodGuard::new(self.client.clone(), &self.base_url, self.token.clone(), &pod_url);

        let start = std::time::Instant::now();
        let mut poll_count = 0;
        loop {
            poll_count += 1;
            if start.elapsed() > Duration::from_secs(pod_startup_timeout_secs()) {
                tracing::warn!("get_snapshots pod {} timed out after {} polls", pod_name, poll_count);
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

        guard.cleanup().await;
        tracing::debug!("get_snapshots pod {} cleaned up", pod_name);

        let snapshots = parse_snapshots(&logs)?;
        if snapshots.is_empty() {
            tracing::warn!("get_snapshots: 0 snapshots for app={}. Raw log ({} bytes): {}", app, logs.len(), &logs[..logs.len().min(1000)]);
        }
        tracing::info!("get_snapshots returning {} snapshots for app={}", snapshots.len(), app);
        Ok(snapshots)
    }

    pub async fn get_dest_repository(&self, app: &str, ns: &str) -> Result<Option<String>, KubeError> {
        let dst_name = self.dest_crd_name(app);
        let dst_url = format!(
            "/apis/{}/v1alpha1/namespaces/{}/replicationdestinations/{}",
            self.api_group, ns, dst_name
        );
        match self.request("GET", &dst_url, None).await {
            Ok(dst) => Ok(dst
                .get("spec")
                .and_then(|s| s.get("restic"))
                .and_then(|r| r.get("repository"))
                .and_then(|v| v.as_str())
                .map(String::from)),
            Err(KubeError::NotFound(_)) => {
                tracing::debug!("Destination CRD {} not found for app={}", dst_name, app);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn trigger_restore(&self, app: &str, ns: &str, timestamp: Option<&str>) -> Result<RestoreResponse, KubeError> {
        tracing::info!("trigger_restore starting for app={} namespace={} timestamp={:?}", app, ns, timestamp);
        let base_app = self.app_base_name(app);

        let hr_url = format!("/apis/helm.toolkit.fluxcd.io/v2/namespaces/{}/helmreleases/{}", ns, base_app);
        if let Err(e) = self.request("PATCH", &hr_url, Some(serde_json::json!({ "spec": { "suspended": true } }))).await {
            tracing::warn!("trigger_restore HelmRelease suspend failed for app={} (non-Flux apps can ignore this): {}", base_app, e);
        } else {
            tracing::info!("trigger_restore HelmRelease suspended for app={}", base_app);
        }

        // Find all deployments belonging to this app via label selector,
        // read original replica counts, then scale all to 0
        let deploys = self.find_app_deployments(&base_app, ns).await;
        let replica_map = self.read_deployments_replicas(&deploys, ns).await;
        self.scale_deployments(&deploys, ns, 0).await;

        let dst_name = self.dest_crd_name(app);
        let dst_url = format!(
            "/apis/{}/v1alpha1/namespaces/{}/replicationdestinations/{}",
            self.api_group, ns, dst_name
        );

        // Read current spec.trigger.manual — this is what Flux maintains as the desired state.
        // If unset (non-Flux app), generate one so the poll loop has something to match.
        let current_rd: Value = match self.request("GET", &dst_url, None).await {
            Ok(rd) => rd,
            Err(e) => {
                self.resume_restore(app, ns, &replica_map).await;
                return Err(e);
            }
        };
        let mut flux_trigger = current_rd.get("spec")
            .and_then(|s| s.get("trigger"))
            .and_then(|t| t.get("manual"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let generated_trigger = flux_trigger.is_empty();
        if generated_trigger {
            flux_trigger = format!("restore-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
            tracing::info!("trigger_restore generated new trigger for non-Flux app: {}", flux_trigger);
        }
        tracing::debug!("trigger_restore using spec.trigger.manual='{}'", flux_trigger);

        let mut restic_spec = serde_json::json!({});
        if let Some(ts) = timestamp {
            if !ts.is_empty() {
                restic_spec["restoreAsOf"] = serde_json::json!(ts);
            }
        }

        let mut dst_patch = serde_json::json!({
            "spec": { "restic": restic_spec }
        });
        if generated_trigger {
            dst_patch["spec"]["trigger"] = serde_json::json!({ "manual": &flux_trigger });
        }

        tracing::debug!("trigger_restore PATCHing ReplicationDestination");
        if let Err(e) = self.request("PATCH", &dst_url, Some(dst_patch)).await {
            tracing::error!("trigger_restore failed to patch ReplicationDestination, rolling back: {}", e);
            self.resume_restore(app, ns, &replica_map).await;
            return Err(KubeError::Api(format!("Failed to patch ReplicationDestination: {}", e)));
        }

        // Trigger restore by nullifying status.lastManualSync — VolSync will see
        // spec.trigger.manual (Flux's value) != status.lastManualSync (null → "")
        let status_url = format!("{}/status", dst_url);
        tracing::debug!("trigger_restore resetting status.lastManualSync to trigger restore");
        if let Err(e) = self.request("PATCH", &status_url, Some(serde_json::json!({
            "status": { "lastManualSync": serde_json::Value::Null }
        }))).await {
            tracing::error!("trigger_restore failed to patch RD status, rolling back: {}", e);
            self.resume_restore(app, ns, &replica_map).await;
            return Err(KubeError::Api(format!("Failed to patch ReplicationDestination status: {}", e)));
        }
        tracing::info!("trigger_restore status.lastManualSync reset, waiting for VolSync to process");

        let restore_timeout = restore_timeout_secs();
        let poll_interval = polling_interval_secs();
        tracing::info!("trigger_restore polling with timeout={}s interval={}s", restore_timeout, poll_interval);

        let start = std::time::Instant::now();
        let mut poll_count = 0;
        loop {
            poll_count += 1;
            if start.elapsed() > Duration::from_secs(restore_timeout) {
                tracing::warn!("trigger_restore polling timed out after {} polls for app={}", poll_count, app);
                self.resume_restore(app, ns, &replica_map).await;
                return Err(KubeError::Timeout("Restore polling timed out".to_string()));
            }
            sleep(Duration::from_secs(poll_interval)).await;

            let dst: Value = match self.request("GET", &dst_url, None).await {
                Ok(d) => d,
                Err(e) => {
                    self.resume_restore(app, ns, &replica_map).await;
                    return Err(e);
                }
            };

            if let Some(last_sync) = dst.get("status")
                .and_then(|s| s.get("lastManualSync"))
                .and_then(|v| v.as_str()) {
                if last_sync == flux_trigger {
                    // Verify mover status before reporting completed
                    let result = dst.get("status")
                        .and_then(|s| s.get("latestMoverStatus"))
                        .and_then(|m| m.get("result"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let finished = result.as_deref().map_or(false, |r| r.eq_ignore_ascii_case("successful"));
                    if finished {
                        tracing::info!("trigger_restore completed on poll #{} for app={} (trigger={:?})", poll_count, app, flux_trigger);
                        // Resume deployment and HelmRelease — no trigger cleanup needed
                        self.resume_restore(app, ns, &replica_map).await;
                        tracing::info!("trigger_restore fully finished — deployments restored and HelmRelease resumed for app={}", app);
                        return Ok(RestoreResponse {
                            trigger: flux_trigger,
                            status: "completed".to_string(),
                            result,
                        });
                    } else {
                        tracing::warn!("trigger_restore failed on poll #{} for app={}: result={:?}", poll_count, app, result);
                        self.resume_restore(app, ns, &replica_map).await;
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

    /// Resume the deployments and HelmRelease to their original state after restore completes (success or failure)
    async fn resume_restore(&self, app: &str, ns: &str, replica_map: &HashMap<String, i64>) {
        tracing::debug!("resume_restore called for app={} with {} deployments to restore", app, replica_map.len());

        for (name, replicas) in replica_map {
            let scale_url = format!("/apis/apps/v1/namespaces/{}/deployments/{}/scale", ns, name);
            if let Err(e) = self.request("PATCH", &scale_url, Some(serde_json::json!({ "spec": { "replicas": replicas } }))).await {
                tracing::warn!("resume_restore failed to scale deployment {} back to {} replicas: {}", name, replicas, e);
            } else {
                tracing::info!("resume_restore scaled deployment {} back to {} replicas", name, replicas);
            }
        }

        let base_app = self.app_base_name(app);
        let hr_url = format!("/apis/helm.toolkit.fluxcd.io/v2/namespaces/{}/helmreleases/{}", ns, base_app);
        if let Err(e) = self.request("PATCH", &hr_url, Some(serde_json::json!({ "spec": { "suspended": false } }))).await {
            tracing::warn!("resume_restore failed to unsuspend HelmRelease {}: {}", base_app, e);
        } else {
            tracing::info!("resume_restore HelmRelease unsuspended for app={} ns={}", base_app, ns);
        }
    }
}

struct PodGuard {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,
    pod_url: String,
    deleted: bool,
}

impl PodGuard {
    fn new(client: reqwest::Client, base_url: &str, token: Option<String>, pod_url: &str) -> Self {
        Self { client, base_url: base_url.to_string(), token, pod_url: pod_url.to_string(), deleted: false }
    }

    async fn cleanup(&mut self) {
        if self.deleted {
            return;
        }
        self.deleted = true;
        let url = format!("{}{}", self.base_url, self.pod_url);
        let mut req = self.client.request(reqwest::Method::DELETE, &url);
        if let Some(ref t) = self.token {
            req = req.header("Authorization", format!("Bearer {}", t));
        }
        if let Err(e) = req.send().await {
            tracing::warn!("PodGuard cleanup failed: {}", e);
        }
    }
}

impl Drop for PodGuard {
    fn drop(&mut self) {
        if self.deleted {
            return;
        }
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let token = self.token.clone();
        let pod_url = self.pod_url.clone();
        tokio::spawn(async move {
            let url = format!("{}{}", base_url, pod_url);
            let mut req = client.request(reqwest::Method::DELETE, &url);
            if let Some(ref t) = token {
                req = req.header("Authorization", format!("Bearer {}", t));
            }
            if let Err(e) = req.send().await {
                tracing::warn!("PodGuard drop cleanup failed: {}", e);
            }
        });
    }
}

fn parse_snapshots(logs: &str) -> Result<Vec<Snapshot>, KubeError> {
    let parse_object = |snap: &Value| -> Option<Snapshot> {
        let id = snap.get("id")?.as_str()?;
        if id.is_empty() {
            return None;
        }
        let summary = snap.get("summary");
        Some(Snapshot {
            id: id.to_string(),
            short_id: snap.get("short_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            time: snap.get("time").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            tags: snap.get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            paths: snap.get("paths")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            hostname: snap.get("hostname").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            files_new: summary.and_then(|s| s.get("files_new")).and_then(|v| v.as_i64()).unwrap_or(0),
            files_changed: summary.and_then(|s| s.get("files_changed")).and_then(|v| v.as_i64()).unwrap_or(0),
            files_unmodified: summary.and_then(|s| s.get("files_unmodified")).and_then(|v| v.as_i64()).unwrap_or(0),
            data_added: summary.and_then(|s| s.get("data_added")).and_then(|v| v.as_i64()).unwrap_or(0),
            total_files_processed: summary.and_then(|s| s.get("total_files_processed")).and_then(|v| v.as_i64()).unwrap_or(0),
            total_bytes_processed: summary.and_then(|s| s.get("total_bytes_processed")).and_then(|v| v.as_i64()).unwrap_or(0),
        })
    };

    // restic outputs a JSON array [{...},{...}]
    if let Ok(arr) = serde_json::from_str::<Vec<Value>>(logs) {
        let snapshots: Vec<Snapshot> = arr.iter().filter_map(parse_object).collect();
        tracing::debug!("parse_snapshots: parsed {} snapshots from array ({} bytes)", snapshots.len(), logs.len());
        return Ok(snapshots);
    }

    // Fallback: per-line NDJSON (some restic commands output one JSON object per line)
    let mut snapshots = Vec::new();
    for line in logs.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(snap) = serde_json::from_str::<Value>(line) {
            if let Some(s) = parse_object(&snap) {
                snapshots.push(s);
            }
        }
    }

    if snapshots.is_empty() && !logs.trim().is_empty() {
        tracing::warn!(
            "parse_snapshots: parsed 0 snapshots from {} bytes. First 500 chars: {}",
            logs.len(),
            &logs[..logs.len().min(500)]
        );
    } else {
        tracing::debug!("parse_snapshots: parsed {} snapshots from NDJSON ({} bytes)", snapshots.len(), logs.len());
    }
    Ok(snapshots)
}
