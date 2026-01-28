use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};

use crate::auth::RequireUser;
use crate::server::AppState;
use crate::server::dto::SetRepoFolderRequest;
use crate::server::response::{ApiError, ApiResponse, StoreOptionExt, StoreResultExt};
use crate::types::Permission;

use super::access::require_repo_permission;

pub async fn get_repo_folder(
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

    let folder = match &repo.folder_id {
        Some(folder_id) => store
            .get_folder_by_id(folder_id)
            .api_err("Failed to get folder")?,
        None => None,
    };

    Ok::<_, ApiError>(Json(ApiResponse::success(folder)))
}

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

    if let Some(ref folder_id) = req.folder_id {
        let folder = store
            .get_folder_by_id(folder_id)
            .api_err("Failed to get folder")?
            .or_not_found("Folder not found")?;

        if folder.namespace_id != repo.namespace_id {
            return Err(ApiError::bad_request(
                "Folder must belong to the same namespace as the repository",
            ));
        }
    }

    store
        .set_repo_folder(&repo.id, req.folder_id.as_deref())
        .api_err("Failed to set repo folder")?;

    let folder = match &req.folder_id {
        Some(folder_id) => store
            .get_folder_by_id(folder_id)
            .api_err("Failed to get folder")?,
        None => None,
    };

    Ok::<_, ApiError>(Json(ApiResponse::success(folder)))
}
