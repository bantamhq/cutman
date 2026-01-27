mod auth;
mod handlers;
mod process;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::server::AppState;

pub fn git_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/{namespace}/{repo}/info/refs", get(handlers::info_refs))
        .route(
            "/{namespace}/{repo}/git-upload-pack",
            post(handlers::git_upload_pack),
        )
        .route(
            "/{namespace}/{repo}/git-receive-pack",
            post(handlers::git_receive_pack),
        )
}
