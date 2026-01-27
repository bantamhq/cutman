mod grants;
mod namespaces;
mod tokens;
mod users;

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
        // User routes
        .route("/users", post(users::create_user))
        .route("/users", get(users::list_users))
        .route("/users/{id}", get(users::get_user))
        .route("/users/{id}", delete(users::delete_user))
        .route("/users/{id}/tokens", get(users::list_user_tokens))
        .route("/users/{id}/tokens", post(users::create_user_token))
        // Namespace grant routes
        .route(
            "/users/{id}/namespace-grants",
            post(grants::create_namespace_grant),
        )
        .route(
            "/users/{id}/namespace-grants",
            get(grants::list_namespace_grants),
        )
        .route(
            "/users/{id}/namespace-grants/{ns_id}",
            get(grants::get_namespace_grant),
        )
        .route(
            "/users/{id}/namespace-grants/{ns_id}",
            delete(grants::delete_namespace_grant),
        )
        // Repo grant routes
        .route("/users/{id}/repo-grants", post(grants::create_repo_grant))
        .route("/users/{id}/repo-grants", get(grants::list_repo_grants))
        .route(
            "/users/{id}/repo-grants/{repo_id}",
            get(grants::get_repo_grant),
        )
        .route(
            "/users/{id}/repo-grants/{repo_id}",
            delete(grants::delete_repo_grant),
        )
}
