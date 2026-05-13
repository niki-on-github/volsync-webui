mod api;
mod kubectl;
mod models;

use api::{health, list_apps, list_namespaces, get_snapshots, trigger_backup, trigger_backup_all, trigger_restore, get_config, get_dest_repository, get_backup_status, get_restore_status};
use std::sync::Arc;
use axum::{Router, extract::Request, response::Response, body::Body};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Serve static files from the `public/` directory with SPA fallback.
async fn serve_static(req: Request) -> Response {
    // Extract the path from the request URI
    let path = req.uri().path().to_string();

    tracing::debug!("Serving static file: {}", path);

    // Normalize path: strip leading slash and prevent directory traversal
    let clean = path.trim_start_matches('/').replace("..", "");
    if clean.is_empty() || clean.contains("..") {
        tracing::debug!("Fallback to index.html (empty or invalid path)");
        return serve_index().await;
    }

    let file_path = format!("public/{}", clean);
    let mime_type = match file_path.rsplit('.').next().unwrap() {
        "html" => "text/html",
        "js" => "application/javascript",
        "wasm" => "application/wasm",
        "css" => "text/css",
        _ => "application/octet-stream",
    };
    match tokio::fs::read(&file_path).await {
        Ok(contents) => {
            tracing::debug!("Served static file: {}", file_path);
            Response::builder()
                .status(200)
                .header("Content-Type", mime_type)
                .body(Body::from(contents))
                .unwrap()
        }
        Err(_) => {
            tracing::debug!("Static file not found, fallback to index.html: {}", file_path);
            serve_index().await
        }
    }
}

/// Serve index.html for SPA client-side routing fallback.
async fn serve_index() -> Response {
    match tokio::fs::read("public/index.html").await {
        Ok(contents) => {
            tracing::debug!("Served index.html");
            Response::builder()
                .status(200)
                .header("Content-Type", "text/html")
                .body(Body::from(contents))
                .unwrap()
        }
        Err(e) => {
            tracing::error!("index.html not found in public/ directory: {}", e);
            Response::builder()
                .status(404)
                .body(Body::from("Not found"))
                .unwrap()
        }
    }
}

#[tokio::main]
async fn main() {
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(rust_log.clone()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting VolSync WebUI (RUST_LOG={})", rust_log);

    let kubectl = Arc::new(match kubectl::Kubectl::new().await {
        Ok(k) => {
            tracing::info!("Kubernetes client initialized successfully");
            k
        }
        Err(e) => {
            tracing::error!("Failed to create Kubernetes client: {}", e);
            std::process::exit(1);
        }
    });

    kubectl.check_rbac().await;

    // Intentionally allow all CORS origins — this is desired behavior for a self-hosted VolSync WebUI
    // that runs inside a Kubernetes cluster and serves the frontend from the same origin.
    // Restricting to specific origins would complicate deployment when the UI is accessed via
    // different ingress paths or port-forwarding. If external access is needed, add an env var
    // like ALLOWED_ORIGINS to make this configurable in the future.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", axum::routing::get(health))
        .route("/api/config", axum::routing::get(get_config))
        .route("/api/namespaces", axum::routing::get(list_namespaces))
        .route("/api/apps", axum::routing::get(list_apps))
        .route("/api/apps/:app/:ns/snapshots", axum::routing::get(get_snapshots))
        .route("/api/apps/:app/:ns/backup", axum::routing::post(trigger_backup))
        .route("/api/apps/:app/:ns/restore", axum::routing::post(trigger_restore))
        .route("/api/apps/backup-all", axum::routing::post(trigger_backup_all))
        .route("/api/apps/:app/:ns/backup/status", axum::routing::get(get_backup_status))
        .route("/api/apps/:app/:ns/restore/status", axum::routing::get(get_restore_status))
        .route("/api/apps/:app/:ns/destination/repository", axum::routing::get(get_dest_repository))
        .fallback(axum::routing::get(serve_static))
        .with_state(kubectl)
        .layer(cors);

    let listener = match tokio::net::TcpListener::bind("0.0.0.0:8080").await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind to port 8080: {}", e);
            std::process::exit(1);
        }
    };
    let addr = listener.local_addr().unwrap();
    tracing::info!("Server listening on http://{}", addr);
    tracing::info!("Available routes:");
    tracing::info!("  GET  /health");
    tracing::info!("  GET  /api/namespaces");
    tracing::info!("  GET  /api/apps");
    tracing::info!("  GET  /api/apps/:app/:ns/snapshots");
    tracing::info!("  POST /api/apps/:app/:ns/backup");
    tracing::info!("  POST /api/apps/:app/:ns/restore");
    tracing::info!("  GET  /api/apps/:app/:ns/backup/status");
    tracing::info!("  GET  /api/apps/:app/:ns/restore/status");
    tracing::info!("  POST /api/apps/backup-all");
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
