mod api;
mod kubectl;
mod models;

use api::{health, list_apps, list_namespaces, get_snapshots, trigger_backup, trigger_backup_all, trigger_restore};
use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{Router, extract::Path, response::Response};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Serve static files from the `public/` directory with SPA fallback.
async fn serve_static(Path(path): Path<String>) -> Response {
    // Normalize path: strip leading slash and prevent directory traversal
    let clean = path.trim_start_matches('/').replace("..", "");
    if clean.is_empty() || clean.contains("..") {
        return serve_index().await;
    }

    let file_path = format!("public/{}", clean);
    match tokio::fs::read(&file_path).await {
        Ok(contents) => Response::builder()
            .status(200)
            .body(axum::body::Body::from(contents))
            .unwrap(),
        Err(_) => serve_index().await,
    }
}

/// Serve index.html for SPA client-side routing fallback.
async fn serve_index() -> Response {
    match tokio::fs::read("public/index.html").await {
        Ok(contents) => Response::builder()
            .status(200)
            .body(axum::body::Body::from(contents))
            .unwrap(),
        Err(_) => Response::builder()
            .status(404)
            .body(axum::body::Body::from("Not found"))
            .unwrap(),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let kubectl = Arc::new(RwLock::new(match kubectl::Kubectl::new().await {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Failed to create kubectl client: {}", e);
            std::process::exit(1);
        }
    }));

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
        .route("/api/namespaces", axum::routing::get(list_namespaces))
        .route("/api/apps", axum::routing::get(list_apps))
        .route("/api/apps/:app/:ns/snapshots", axum::routing::get(get_snapshots))
        .route("/api/apps/:app/:ns/backup", axum::routing::post(trigger_backup))
        .route("/api/apps/:app/:ns/restore", axum::routing::post(trigger_restore))
        .route("/api/apps/backup-all", axum::routing::post(trigger_backup_all))
        .fallback(axum::routing::get(serve_static))
        .with_state(kubectl)
        .layer(cors);

    let listener = match tokio::net::TcpListener::bind("0.0.0.0:8080").await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to port 8080: {}", e);
            std::process::exit(1);
        }
    };
    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
