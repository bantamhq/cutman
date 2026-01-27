use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::auth::RequireUser;
use crate::server::AppState;
use crate::server::dto::RepoFoldersRequest;
use crate::server::response::{ApiError, ApiResponse, StoreOptionExt, StoreResultExt};
use crate::store::Store;
use crate::types::{Permission, Repo};

use super::access::require_repo_permission;

fn validate_folders_for_repo(
    store: &dyn Store,
    repo: &Repo,
    folder_ids: &[String],
) -> Result<(), ApiError> {
    for folder_id in folder_ids {
        let folder = store
            .get_folder_by_id(folder_id)
            .api_err("Failed to get folder")?
            .ok_or_else(|| ApiError::not_found(format!("Folder not found: {folder_id}")))?;

        if folder.namespace_id != repo.namespace_id {
            return Err(ApiError::bad_request(
                "Folder must belong to the same namespace as the repository",
            ));
        }
    }
    Ok(())
}

pub async fn list_repo_folders(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_READ)?;

    let folders = store.list_repo_folders(&repo.id).api_err("Failed to list repo folders")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(folders)))
}

pub async fn add_repo_folders(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<RepoFoldersRequest>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_WRITE)?;
    validate_folders_for_repo(store, &repo, &req.folder_ids)?;

    for folder_id in &req.folder_ids {
        store.add_repo_folder(&repo.id, folder_id).api_err("Failed to add repo folder")?;
    }

    let folders = store.list_repo_folders(&repo.id).api_err("Failed to list repo folders")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(folders)))
}

pub async fn set_repo_folders(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<RepoFoldersRequest>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_WRITE)?;
    validate_folders_for_repo(store, &repo, &req.folder_ids)?;

    store.set_repo_folders(&repo.id, &req.folder_ids).api_err("Failed to set repo folders")?;

    let folders = store.list_repo_folders(&repo.id).api_err("Failed to list repo folders")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(folders)))
}

pub async fn remove_repo_folder(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path((id, folder_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_WRITE)?;

    store
        .get_folder_by_id(&folder_id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    store.remove_repo_folder(&repo.id, &folder_id).api_err("Failed to remove repo folder")?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}
