use std::path::PathBuf;
use std::sync::Arc;

use axum::{Router, routing::get};
use tower_http::trace::TraceLayer;

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

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api/v1/admin", admin_router())
        .nest("/api/v1", user_router())
        .nest("/api/v1", content_router())
        .nest("/git", git_router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
