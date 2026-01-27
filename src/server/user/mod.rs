mod access;
mod folders;
mod namespaces;
mod repo_folders;
mod repos;

use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};

use crate::server::AppState;

pub fn user_router() -> Router<Arc<AppState>> {
    Router::new()
        // Namespaces
        .route("/namespaces", get(namespaces::list_namespaces))
        .route("/namespaces/{name}", patch(namespaces::update_namespace))
        .route("/namespaces/{name}", delete(namespaces::delete_namespace))
        .route(
            "/namespaces/{name}/grants",
            get(namespaces::list_namespace_grants),
        )
        // Repos
        .route("/repos", get(repos::list_repos))
        .route("/repos", post(repos::create_repo))
        .route("/repos/{id}", get(repos::get_repo))
        .route("/repos/{id}", patch(repos::update_repo))
        .route("/repos/{id}", delete(repos::delete_repo))
        // Repo folders
        .route("/repos/{id}/folders", get(repo_folders::list_repo_folders))
        .route("/repos/{id}/folders", post(repo_folders::add_repo_folders))
        .route("/repos/{id}/folders", put(repo_folders::set_repo_folders))
        .route(
            "/repos/{id}/folders/{folder_id}",
            delete(repo_folders::remove_repo_folder),
        )
        // Folders
        .route("/folders", get(folders::list_folders))
        .route("/folders", post(folders::create_folder))
        .route("/folders/{id}", get(folders::get_folder))
        .route("/folders/{id}", patch(folders::update_folder))
        .route("/folders/{id}", delete(folders::delete_folder))
}
