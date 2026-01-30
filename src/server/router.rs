use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::{Router, routing::get};

use super::admin::admin_router;
use super::content::content_router;
use super::git::git_router;
use super::user::user_router;
use crate::store::Store;

pub struct AppState {
    pub store: Arc<dyn Store>,
    pub data_dir: PathBuf,
    /// Public base URL for external access. Used for LFS action URLs.
    pub public_base_url: Option<String>,
}

async fn health() -> &'static str {
    "OK"
}

async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();

    let response = next.run(request).await;

    let latency = start.elapsed();
    let status = response.status();

    tracing::info!(
        "{} {} {} {}ms",
        method,
        uri.path(),
        status.as_u16(),
        latency.as_millis()
    );

    response
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api/v1/admin", admin_router())
        .nest("/api/v1", user_router())
        .nest("/api/v1", content_router())
        .nest("/git", git_router())
        .layer(middleware::from_fn(log_request))
        .with_state(state)
}
