use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::auth::RequireUser;
use crate::server::AppState;
use crate::server::dto::SetRepoFolderRequest;
use crate::server::response::{ApiError, ApiResponse, StoreOptionExt, StoreResultExt};
use crate::store::path::normalize_path;
use crate::types::{Folder, Permission};

use super::access::require_repo_permission;

fn folder_to_vec(folder: Option<Folder>) -> Vec<Folder> {
    folder.into_iter().collect()
}

/// GET /repos/{id}/folders - Returns array of folders (0 or 1)
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

    let folder = match repo.folder_id {
        Some(folder_id) => store
            .get_folder_by_id(folder_id)
            .api_err("Failed to get folder")?,
        None => None,
    };

    Ok::<_, ApiError>(Json(ApiResponse::success(folder_to_vec(folder))))
}

/// POST /repos/{id}/folders - Set the folder for a repo by path
pub async fn set_repo_folder(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<SetRepoFolderRequest>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_WRITE)?;

    let normalized_path = match &req.folder_path {
        Some(path) => Some(normalize_path(path).map_err(|e| ApiError::bad_request(e.to_string()))?),
        None => None,
    };

    let folder_id = store
        .set_repo_folder_by_path(&repo.id, &repo.namespace_id, normalized_path.as_deref())
        .api_err("Failed to set repo folder")?;

    let folder = match folder_id {
        Some(id) => store.get_folder_by_id(id).api_err("Failed to get folder")?,
        None => None,
    };

    Ok::<_, ApiError>(Json(ApiResponse::success(folder_to_vec(folder))))
}

#[derive(serde::Deserialize)]
pub struct RepoFolderPath {
    id: String,
    folder_id: i64,
}

/// DELETE /repos/{id}/folders/{folder_id} - Clear folder if it matches
pub async fn clear_repo_folder(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(path): Path<RepoFolderPath>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&path.id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_WRITE)?;

    match repo.folder_id {
        Some(current_folder_id) if current_folder_id == path.folder_id => {
            store
                .set_repo_folder(&repo.id, None)
                .api_err("Failed to clear repo folder")?;
            Ok::<_, ApiError>(StatusCode::NO_CONTENT.into_response())
        }
        Some(_) => Err(ApiError::bad_request(
            "Folder ID does not match current folder",
        )),
        None => Err(ApiError::not_found("Repository has no folder assigned")),
    }
}
