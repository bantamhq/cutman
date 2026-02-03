use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::RequireAdmin;
use crate::server::AppState;
use crate::server::dto::{CreateNamespaceRequest, PaginationParams};
use crate::server::response::{
    ApiError, ApiResponse, DEFAULT_PAGE_SIZE, PaginatedResponse, paginate,
};
use crate::types::Namespace;

pub async fn create_namespace(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateNamespaceRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate_name(&req.name) {
        return Err(ApiError::bad_request(e));
    }

    let existing = state
        .store
        .get_namespace_by_name(&req.name)
        .map_err(|_| ApiError::internal("Failed to check existing namespace"))?;

    if existing.is_some() {
        return Err(ApiError::conflict("Namespace already exists"));
    }

    let ns = Namespace {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        created_at: Utc::now(),
        repo_limit: req.repo_limit,
        storage_limit_bytes: req.storage_limit_bytes,
        external_id: None,
    };

    state
        .store
        .create_namespace(&ns)
        .map_err(|_| ApiError::internal("Failed to create namespace"))?;

    Ok((StatusCode::CREATED, Json(ApiResponse::success(ns))))
}

pub async fn list_namespaces(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let cursor = params.cursor.as_deref().unwrap_or("");

    let namespaces = state
        .store
        .list_namespaces(cursor, DEFAULT_PAGE_SIZE + 1)
        .map_err(|_| ApiError::internal("Failed to list namespaces"))?;

    let (namespaces, next_cursor, has_more) =
        paginate(namespaces, DEFAULT_PAGE_SIZE as usize, |ns| ns.id.clone());

    Ok::<_, ApiError>(Json(PaginatedResponse::new(
        namespaces,
        next_cursor,
        has_more,
    )))
}

pub async fn get_namespace(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let ns = state
        .store
        .get_namespace_by_name(&name)
        .map_err(|_| ApiError::internal("Failed to get namespace"))?
        .ok_or_else(|| ApiError::not_found("Namespace not found"))?;

    Ok::<_, ApiError>(Json(ApiResponse::success(ns)))
}

pub async fn delete_namespace(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let ns = state
        .store
        .get_namespace_by_name(&name)
        .map_err(|_| ApiError::internal("Failed to get namespace"))?
        .ok_or_else(|| ApiError::not_found("Namespace not found"))?;

    let repos = state
        .store
        .list_repos(&ns.id, "", 1)
        .map_err(|_| ApiError::internal("Failed to check repos"))?;

    if !repos.is_empty() {
        return Err(ApiError::conflict(
            "Cannot delete namespace with existing repos",
        ));
    }

    let principal_count = state
        .store
        .count_namespace_principals(&ns.id)
        .map_err(|_| ApiError::internal("Failed to check principals"))?;

    if principal_count > 0 {
        return Err(ApiError::conflict(
            "Cannot delete namespace with principal access",
        ));
    }

    state
        .store
        .delete_namespace(&ns.id)
        .map_err(|_| ApiError::internal("Failed to delete namespace"))?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }

    if name.len() > 64 {
        return Err("Name cannot exceed 64 characters".to_string());
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "Name can only contain alphanumeric characters, hyphens, and underscores".to_string(),
        );
    }

    if name.starts_with('-') || name.starts_with('_') {
        return Err("Name cannot start with a hyphen or underscore".to_string());
    }

    Ok(())
}
