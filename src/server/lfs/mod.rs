mod dto;
mod handlers;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post, put};

use crate::server::AppState;

pub fn lfs_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/objects/batch", post(handlers::batch))
        .route("/objects/{oid}", get(handlers::download))
        .route("/objects/{oid}", put(handlers::upload))
        .route("/verify", post(handlers::verify))
}
