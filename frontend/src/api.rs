use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen::JsCast;
use web_sys::{Headers, Request, RequestInit, Response};
use wasm_bindgen_futures::JsFuture;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct App {
    pub name: String,
    pub namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<BackupSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBackupStatus {
    pub app: String,
    pub namespace: String,
    pub success: bool,
    pub error: Option<String>,
}

fn get_base_url() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| {
            log::warn!("Failed to get window origin, falling back to http://localhost:8080");
            "http://localhost:8080".to_string()
        })
}

async fn fetch_json<T: for<'de> Deserialize<'de>>(url: &str, method: &str, body: Option<&str>) -> Result<T, String> {
    log::debug!("fetch_json: {} {}", method, url);

    let opts = RequestInit::new();
    opts.set_method(method);

    if let Some(b) = body {
        log::debug!("fetch_json: request body={}", b);
        opts.set_body(&JsValue::from_str(b));
        let headers = Headers::new().map_err(|_| {
            let msg = "Failed to create Headers".to_string();
            log::error!("{}", msg);
            msg
        })?;
        headers.set("Content-Type", "application/json").map_err(|_| {
            let msg = "Failed to set Content-Type header".to_string();
            log::error!("{}", msg);
            msg
        })?;
        opts.set_headers(&headers);
    }

    let request = Request::new_with_str_and_init(url, &opts).map_err(|e| {
        let msg = e.as_string().unwrap_or_else(|| format!("{:?}", e));
        log::error!("fetch_json: failed to create request for {}: {}", url, msg);
        msg
    })?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| {
            let msg = e.as_string().unwrap_or_else(|| format!("{:?}", e));
            log::error!("fetch_json: fetch failed for {}: {}", url, msg);
            msg
        })?;
    let resp: Response = resp_value.dyn_into().map_err(|e| {
        let msg = e.as_string().unwrap_or_else(|| format!("{:?}", e));
        log::error!("fetch_json: failed to dyn_into Response for {}: {}", url, msg);
        msg
    })?;

    let body_text = JsFuture::from(resp.text().map_err(|e| {
        let msg = e.as_string().unwrap_or_else(|| format!("{:?}", e));
        log::error!("fetch_json: failed to get response text for {}: {}", url, msg);
        msg
    })?)
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| format!("{:?}", e)))?
        .as_string()
        .ok_or_else(|| {
            let msg = format!("Response body is not a string for {}", url);
            log::error!("{}", msg);
            msg
        })?;

    let status = resp.status();
    if status >= 400 {
        log::error!("fetch_json: HTTP {} for {}: {}", status, url, body_text);
        return Err(format!("HTTP {}: {}", status, body_text));
    }

    serde_json::from_str::<T>(&body_text).map_err(|e| {
        log::error!("fetch_json: failed to parse JSON for {}: {} (body: {})", url, e, body_text);
        e.to_string()
    })
}

pub async fn list_namespaces() -> Result<Vec<String>, String> {
    log::debug!("list_namespaces called");
    let url = format!("{}/api/namespaces", get_base_url());
    match fetch_json::<Vec<String>>(&url, "GET", None).await {
        Ok(ns) => {
            log::debug!("list_namespaces returned {} namespaces", ns.len());
            Ok(ns)
        }
        Err(e) => {
            log::error!("list_namespaces failed: {}", e);
            Err(e)
        }
    }
}

pub async fn list_apps(namespace: Option<&str>) -> Result<Vec<App>, String> {
    let ns_filter = namespace.unwrap_or("all");
    log::debug!("list_apps called with namespace filter: {}", ns_filter);
    let base_url = get_base_url();
    let url = match namespace {
        Some(ns) if ns != "all" => format!("{}/api/apps?namespace={}", base_url, ns),
        _ => format!("{}/api/apps", base_url),
    };
    match fetch_json::<Vec<App>>(&url, "GET", None).await {
        Ok(apps) => {
            log::debug!("list_apps returned {} apps", apps.len());
            Ok(apps)
        }
        Err(e) => {
            log::error!("list_apps failed: {}", e);
            Err(e)
        }
    }
}

pub async fn get_snapshots(app: &str, ns: &str) -> Result<Vec<Snapshot>, String> {
    log::debug!("get_snapshots called for app={} namespace={}", app, ns);
    let url = format!("{}/api/apps/{}/{}/snapshots", get_base_url(), app, ns);
    match fetch_json::<Vec<Snapshot>>(&url, "GET", None).await {
        Ok(snaps) => {
            log::debug!("get_snapshots returned {} snapshots for app={}", snaps.len(), app);
            Ok(snaps)
        }
        Err(e) => {
            log::error!("get_snapshots failed for app={}: {}", app, e);
            Err(e)
        }
    }
}

pub async fn trigger_backup(app: &str, ns: &str) -> Result<BackupResponse, String> {
    log::info!("trigger_backup called for app={} namespace={}", app, ns);
    let url = format!("{}/api/apps/{}/{}/backup", get_base_url(), app, ns);
    match fetch_json::<BackupResponse>(&url, "POST", None).await {
        Ok(resp) => {
            log::info!("trigger_backup completed for app={} status={}", app, resp.status);
            Ok(resp)
        }
        Err(e) => {
            log::error!("trigger_backup failed for app={}: {}", app, e);
            Err(e)
        }
    }
}

pub async fn trigger_backup_all() -> Result<BackupAllResponse, String> {
    log::info!("trigger_backup_all called");
    let url = format!("{}/api/apps/backup-all", get_base_url());
    match fetch_json::<BackupAllResponse>(&url, "POST", None).await {
        Ok(resp) => {
            let success = resp.apps.iter().filter(|a| a.success).count();
            log::info!("trigger_backup_all completed: {}/{} succeeded", success, resp.apps.len());
            Ok(resp)
        }
        Err(e) => {
            log::error!("trigger_backup_all failed: {}", e);
            Err(e)
        }
    }
}

pub async fn trigger_restore(app: &str, ns: &str, trigger: &str, timestamp: Option<String>) -> Result<RestoreResponse, String> {
    log::info!("trigger_restore called for app={} namespace={} timestamp={:?}", app, ns, timestamp);
    let url = format!("{}/api/apps/{}/{}/restore", get_base_url(), app, ns);
    let body = RestoreRequest {
        trigger: trigger.to_string(),
        timestamp,
    };
    let body_str = serde_json::to_string(&body).map_err(|e| {
        log::error!("trigger_restore: failed to serialize request body: {}", e);
        e.to_string()
    })?;
    match fetch_json::<RestoreResponse>(&url, "POST", Some(&body_str)).await {
        Ok(resp) => {
            log::info!("trigger_restore completed for app={} status={}", app, resp.status);
            Ok(resp)
        }
        Err(e) => {
            log::error!("trigger_restore failed for app={}: {}", app, e);
            Err(e)
        }
    }
}
