use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen::JsCast;
use web_sys::{Request, RequestInit, Response};
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
    let opts = RequestInit::new();
    opts.set_method(method);

    // Bug #14 fix: Set Content-Type for POST requests with body
    if let Some(b) = body {
        opts.set_body(&JsValue::from_str(b));
        let headers = js_sys::Object::new();
        js_sys::Reflect::set(&headers, &JsValue::from_str("Content-Type"), &JsValue::from_str("application/json")).ok();
        opts.set_headers(&headers);
    }

    let request = Request::new_with_str_and_init(url, &opts).map_err(|e| e.as_string().unwrap_or_default())?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| e.as_string().unwrap_or_default())?;
    let resp: Response = resp_value.dyn_into().map_err(|e| e.as_string().unwrap_or_default())?;

    // Bug #9 fix: Check HTTP status code before parsing response
    let status = resp.status();
    if status >= 400 {
        let error_text = JsFuture::from(resp.text().map_err(|e| e.as_string().unwrap_or_default())?)
            .await
            .map_err(|e| e.as_string().unwrap_or_default())?
            .as_string()
            .unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, error_text));
    }

    let json_str = JsFuture::from(resp.text().map_err(|e| e.as_string().unwrap_or_default())?)
        .await
        .map_err(|e| e.as_string().unwrap_or_default())?
        .as_string()
        .ok_or("Failed to get response text")?;

    serde_json::from_str(&json_str).map_err(|e| e.to_string())
}

pub async fn list_namespaces() -> Result<Vec<String>, String> {
    let url = format!("{}/api/namespaces", get_base_url());
    fetch_json(&url, "GET", None).await
}

pub async fn list_apps(namespace: Option<&str>) -> Result<Vec<App>, String> {
    let base_url = get_base_url();
    let url = if let Some(ns) = namespace {
        format!("{}/api/apps?namespace={}", base_url, ns)
    } else {
        format!("{}/api/apps", base_url)
    };
    fetch_json(&url, "GET", None).await
}

pub async fn get_snapshots(app: &str, ns: &str) -> Result<Vec<Snapshot>, String> {
    let url = format!("{}/api/apps/{}/{}/snapshots", get_base_url(), app, ns);
    fetch_json(&url, "GET", None).await
}

pub async fn trigger_backup(app: &str, ns: &str) -> Result<BackupResponse, String> {
    let url = format!("{}/api/apps/{}/{}/backup", get_base_url(), app, ns);
    fetch_json(&url, "POST", None).await
}

pub async fn trigger_backup_all() -> Result<BackupAllResponse, String> {
    let url = format!("{}/api/apps/backup-all", get_base_url());
    fetch_json(&url, "POST", None).await
}

pub async fn trigger_restore(app: &str, ns: &str, trigger: &str, timestamp: Option<String>) -> Result<RestoreResponse, String> {
    let url = format!("{}/api/apps/{}/{}/restore", get_base_url(), app, ns);
    let body = RestoreRequest {
        trigger: trigger.to_string(),
        timestamp,
    };
    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    fetch_json(&url, "POST", Some(&body_str)).await
}
