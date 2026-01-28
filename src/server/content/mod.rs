mod auth;
mod dto;
mod git_ops;
mod handlers;

use std::sync::Arc;

use axum::{Router, routing::get};

use crate::server::AppState;

pub fn content_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/repos/{id}/refs", get(handlers::list_refs))
        .route("/repos/{id}/commits", get(handlers::list_commits))
        .route(
            "/repos/{id}/commits/{sha}",
            get(handlers::get_commit_handler),
        )
        .route(
            "/repos/{id}/commits/{sha}/diff",
            get(handlers::get_commit_diff),
        )
        .route("/repos/{id}/compare/{spec}", get(handlers::compare_refs))
        .route("/repos/{id}/tree/{ref}", get(handlers::get_tree_root))
        .route(
            "/repos/{id}/tree/{ref}/{*path}",
            get(handlers::get_tree_handler),
        )
        .route("/repos/{id}/blob/{ref}/{*path}", get(handlers::get_blob))
        .route("/repos/{id}/blame/{ref}/{*path}", get(handlers::get_blame))
        .route("/repos/{id}/archive/{ref}", get(handlers::get_archive))
        .route("/repos/{id}/readme", get(handlers::get_readme))
}
