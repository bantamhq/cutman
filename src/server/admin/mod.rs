mod grants;
mod namespaces;
mod principals;
mod tokens;

use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::server::AppState;

pub fn admin_router() -> Router<Arc<AppState>> {
    Router::new()
        // Namespace routes
        .route("/namespaces", post(namespaces::create_namespace))
        .route("/namespaces", get(namespaces::list_namespaces))
        .route("/namespaces/{name}", get(namespaces::get_namespace))
        .route("/namespaces/{name}", delete(namespaces::delete_namespace))
        // Token routes
        .route("/tokens", get(tokens::list_tokens))
        .route("/tokens/{id}", get(tokens::get_token))
        .route("/tokens/{id}", delete(tokens::delete_token))
        // Principal routes
        .route("/principals", post(principals::create_principal))
        .route("/principals", get(principals::list_principals))
        .route("/principals/{id}", get(principals::get_principal))
        .route("/principals/{id}", delete(principals::delete_principal))
        .route(
            "/principals/{id}/tokens",
            get(principals::list_principal_tokens),
        )
        .route(
            "/principals/{id}/tokens",
            post(principals::create_principal_token),
        )
        // Namespace grant routes
        .route(
            "/principals/{id}/namespace-grants",
            post(grants::create_namespace_grant),
        )
        .route(
            "/principals/{id}/namespace-grants",
            get(grants::list_namespace_grants),
        )
        .route(
            "/principals/{id}/namespace-grants/{ns_id}",
            get(grants::get_namespace_grant),
        )
        .route(
            "/principals/{id}/namespace-grants/{ns_id}",
            delete(grants::delete_namespace_grant),
        )
        // Repo grant routes
        .route(
            "/principals/{id}/repo-grants",
            post(grants::create_repo_grant),
        )
        .route(
            "/principals/{id}/repo-grants",
            get(grants::list_repo_grants),
        )
        .route(
            "/principals/{id}/repo-grants/{repo_id}",
            get(grants::get_repo_grant),
        )
        .route(
            "/principals/{id}/repo-grants/{repo_id}",
            delete(grants::delete_repo_grant),
        )
}
