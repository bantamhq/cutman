use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::auth::RequirePrincipal;
use crate::server::AppState;
use crate::server::dto::{
    CreateFolderRequest, ListFolderReposParams, ListFoldersParams, UpdateFolderRequest,
};
use crate::server::response::{ApiError, ApiResponse, StoreOptionExt, StoreResultExt};
use crate::store::path::normalize_path;
use crate::types::Permission;

use super::access::{require_namespace_permission, resolve_namespace_id};

pub async fn list_folders(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListFoldersParams>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();
    let ns_id = resolve_namespace_id(store, principal, params.namespace.as_deref())?;

    require_namespace_permission(store, principal, &ns_id, Permission::NAMESPACE_READ)?;

    let folders = store
        .list_all_folders(&ns_id)
        .api_err("Failed to list folders")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(folders)))
}

pub async fn create_folder(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateFolderRequest>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();
    let ns_id = resolve_namespace_id(store, principal, req.namespace.as_deref())?;

    require_namespace_permission(store, principal, &ns_id, Permission::NAMESPACE_WRITE)?;

    let normalized_path =
        normalize_path(&req.path).map_err(|e| ApiError::bad_request(e.to_string()))?;

    if store
        .get_folder_by_path(&ns_id, &normalized_path)
        .api_err("Failed to check folder")?
        .is_some()
    {
        return Err(ApiError::conflict("Folder already exists at this path"));
    }

    let folder_id = store
        .ensure_folder_path(&ns_id, &normalized_path)
        .api_err("Failed to create folder")?;

    let folder = store
        .get_folder_by_id(folder_id)
        .api_err("Failed to get created folder")?
        .or_not_found("Created folder not found")?;

    Ok::<_, ApiError>((StatusCode::CREATED, Json(ApiResponse::success(folder))))
}

pub async fn get_folder(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let folder = store
        .get_folder_by_id(id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        principal,
        &folder.namespace_id,
        Permission::NAMESPACE_READ,
    )?;

    Ok::<_, ApiError>(Json(ApiResponse::success(folder)))
}

pub async fn update_folder(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateFolderRequest>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let folder = store
        .get_folder_by_id(id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        principal,
        &folder.namespace_id,
        Permission::NAMESPACE_WRITE,
    )?;

    if let Some(new_path) = req.path {
        let normalized =
            normalize_path(&new_path).map_err(|e| ApiError::bad_request(e.to_string()))?;

        store.move_folder(id, &normalized).map_err(|e| match e {
            crate::error::Error::BadRequest(msg) => ApiError::bad_request(&msg),
            crate::error::Error::Conflict(msg) => ApiError::conflict(&msg),
            _ => ApiError::internal(e.to_string()),
        })?;
    }

    let updated_folder = store
        .get_folder_by_id(id)
        .api_err("Failed to get updated folder")?
        .or_not_found("Folder not found after update")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(updated_folder)))
}

pub async fn delete_folder(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let folder = store
        .get_folder_by_id(id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        principal,
        &folder.namespace_id,
        Permission::NAMESPACE_ADMIN,
    )?;

    store.delete_folder(id).api_err("Failed to delete folder")?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}

pub async fn list_folder_repos(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<ListFolderReposParams>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let folder = store
        .get_folder_by_id(id)
        .api_err("Failed to get folder")?
        .or_not_found("Folder not found")?;

    require_namespace_permission(
        store,
        principal,
        &folder.namespace_id,
        Permission::NAMESPACE_READ,
    )?;

    let recursive = params.recursive.unwrap_or(false);
    let repos = store
        .list_folder_repos(&folder.namespace_id, &folder.path, recursive)
        .api_err("Failed to list folder repos")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(repos)))
}
