use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::RequireUser;
use crate::server::AppState;
use crate::server::dto::{
    CreateFolderRequest, DeleteFolderParams, ListFoldersParams, UpdateFolderRequest,
};
use crate::server::response::{
    ApiError, ApiResponse, DEFAULT_PAGE_SIZE, PaginatedResponse, StoreOptionExt, StoreResultExt,
    paginate,
};
use crate::types::{Folder, Permission};

use super::access::{require_namespace_permission, resolve_namespace_id};

#[derive(Debug, Serialize)]
pub struct FolderWithPath {
    #[serde(flatten)]
    pub folder: Folder,
    pub path: String,
}

pub async fn list_folders(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListFoldersParams>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();
    let ns_id = resolve_namespace_id(store, user, params.namespace.as_deref())?;
    let cursor = params.cursor.as_deref().unwrap_or("");

    require_namespace_permission(store, user, &ns_id, Permission::NAMESPACE_READ)?;

    let folders = store
        .list_folders(
            &ns_id,
            params.parent_id.as_deref(),
            cursor,
            DEFAULT_PAGE_SIZE + 1,
        )
        .api_err("Failed to list folders")?;

    let (folders, next_cursor, has_more) =
        paginate(folders, DEFAULT_PAGE_SIZE as usize, |f| f.name.clone());

    Ok::<_, ApiError>(Json(PaginatedResponse::new(folders, next_cursor, has_more)))
}

pub async fn create_folder(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateFolderRequest>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();
    let ns_id = resolve_namespace_id(store, user, req.namespace.as_deref())?;

    require_namespace_permission(store, user, &ns_id, Permission::NAMESPACE_WRITE)?;

    if let Some(parent_id) = &req.parent_id {
        let parent = store
            .get_folder_by_id(parent_id)
            .api_err("Failed to get parent folder")?
            .or_not_found("Parent folder not found")?;

        if parent.namespace_id != ns_id {
            return Err(ApiError::bad_request(
                "Parent folder must belong to the same namespace",
            ));
        }
    }

    if store
        .get_folder_by_name(&ns_id, req.parent_id.as_deref(), &req.name)
        .api_err("Failed to check folder")?
        .is_some()
    {
        return Err(ApiError::conflict("Folder already exists"));
    }

    let now = Utc::now();
    let folder = Folder {
        id: Uuid::new_v4().to_string(),
        namespace_id: ns_id,
        parent_id: req.parent_id,
        name: req.name,
        created_at: now,
        updated_at: now,
    };

    store
        .create_folder(&folder)
        .api_err("Failed to create folder")?;

    Ok::<_, ApiError>((StatusCode::CREATED, Json(ApiResponse::success(folder))))
}

pub async fn get_folder(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let folder = store
        .get_folder_by_id(&id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        user,
        &folder.namespace_id,
        Permission::NAMESPACE_READ,
    )?;

    let path = store
        .get_folder_path(&folder.id)
        .api_err("Failed to get folder path")?;

    let response = FolderWithPath { folder, path };

    Ok::<_, ApiError>(Json(ApiResponse::success(response)))
}

pub async fn update_folder(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateFolderRequest>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let mut folder = store
        .get_folder_by_id(&id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        user,
        &folder.namespace_id,
        Permission::NAMESPACE_WRITE,
    )?;

    if let Some(ref parent_id) = req.parent_id {
        if parent_id == &folder.id {
            return Err(ApiError::bad_request("Folder cannot be its own parent"));
        }

        let parent = store
            .get_folder_by_id(parent_id)
            .api_err("Failed to get parent folder")?
            .or_not_found("Parent folder not found")?;

        if parent.namespace_id != folder.namespace_id {
            return Err(ApiError::bad_request(
                "Parent folder must belong to the same namespace",
            ));
        }

        let ancestors = store
            .list_folder_ancestors(parent_id)
            .api_err("Failed to check for cycles")?;
        if ancestors.iter().any(|a| a.id == folder.id) {
            return Err(ApiError::bad_request("Moving folder would create a cycle"));
        }
    }

    let new_parent_id = req.parent_id.clone().or(folder.parent_id.clone());

    if let Some(name) = req.name {
        if name != folder.name
            && store
                .get_folder_by_name(&folder.namespace_id, new_parent_id.as_deref(), &name)
                .api_err("Failed to check folder name")?
                .is_some()
        {
            return Err(ApiError::conflict(
                "Folder name already exists in this location",
            ));
        }
        folder.name = name;
    }

    if req.parent_id.is_some() {
        folder.parent_id = req.parent_id;
    }

    store
        .update_folder(&folder)
        .api_err("Failed to update folder")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(folder)))
}

pub async fn delete_folder(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<DeleteFolderParams>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let folder = store
        .get_folder_by_id(&id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        user,
        &folder.namespace_id,
        Permission::NAMESPACE_ADMIN,
    )?;

    let recursive = params.recursive == Some(true);

    let repo_count = store
        .count_folder_repos(&folder.id, recursive)
        .api_err("Failed to count folder repos")?;

    if repo_count > 0 && params.force != Some(true) {
        return Err(ApiError::conflict(
            "Folder has repos associated. Use ?force=true to delete anyway",
        ));
    }

    let children = store
        .list_folder_children(&folder.id)
        .api_err("Failed to list children")?;

    if !children.is_empty() && !recursive {
        return Err(ApiError::conflict(
            "Folder has subfolders. Use ?recursive=true to delete recursively",
        ));
    }

    store
        .delete_folder(&folder.id, recursive)
        .api_err("Failed to delete folder")?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}

pub async fn list_folder_children(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let folder = store
        .get_folder_by_id(&id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        user,
        &folder.namespace_id,
        Permission::NAMESPACE_READ,
    )?;

    let children = store
        .list_folder_children(&folder.id)
        .api_err("Failed to list folder children")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(children)))
}

pub async fn list_folder_repos(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let folder = store
        .get_folder_by_id(&id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        user,
        &folder.namespace_id,
        Permission::NAMESPACE_READ,
    )?;

    let repos = store
        .list_folder_repos(&folder.id, false)
        .api_err("Failed to list folder repos")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(repos)))
}
