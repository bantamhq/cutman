pub mod access;
mod folders;
mod namespaces;
mod repo_folder;
mod repo_tags;
mod repos;
mod tags;

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
        // Repo tags (many-to-many)
        .route("/repos/{id}/tags", get(repo_tags::list_repo_tags))
        .route("/repos/{id}/tags", post(repo_tags::add_repo_tags))
        .route("/repos/{id}/tags", put(repo_tags::set_repo_tags))
        .route(
            "/repos/{id}/tags/{tag_id}",
            delete(repo_tags::remove_repo_tag),
        )
        // Repo folders (one-to-one relationship, plural for consistency)
        .route("/repos/{id}/folders", get(repo_folder::list_repo_folders))
        .route("/repos/{id}/folders", post(repo_folder::set_repo_folder))
        .route(
            "/repos/{id}/folders/{folder_id}",
            delete(repo_folder::clear_repo_folder),
        )
        // Tags
        .route("/tags", get(tags::list_tags))
        .route("/tags", post(tags::create_tag))
        .route("/tags/{id}", get(tags::get_tag))
        .route("/tags/{id}", patch(tags::update_tag))
        .route("/tags/{id}", delete(tags::delete_tag))
        // Folders (hierarchical)
        .route("/folders", get(folders::list_folders))
        .route("/folders", post(folders::create_folder))
        .route("/folders/{id}", get(folders::get_folder))
        .route("/folders/{id}", patch(folders::update_folder))
        .route("/folders/{id}", delete(folders::delete_folder))
        .route("/folders/{id}/children", get(folders::list_folder_children))
        .route("/folders/{id}/repos", get(folders::list_folder_repos))
}
