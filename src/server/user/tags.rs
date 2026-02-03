use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::RequirePrincipal;
use crate::server::AppState;
use crate::server::dto::{CreateTagRequest, DeleteTagParams, ListTagsParams, UpdateTagRequest};
use crate::server::response::{
    ApiError, ApiResponse, DEFAULT_PAGE_SIZE, PaginatedResponse, StoreOptionExt, StoreResultExt,
    paginate,
};
use crate::types::{Permission, Tag};

use super::access::{require_namespace_permission, resolve_namespace_id};
use crate::server::validation::validate_tag_name;

pub async fn list_tags(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListTagsParams>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();
    let ns_id = resolve_namespace_id(store, principal, params.namespace.as_deref())?;
    let cursor = params.cursor.as_deref().unwrap_or("");

    require_namespace_permission(store, principal, &ns_id, Permission::NAMESPACE_READ)?;

    let tags = store
        .list_tags(&ns_id, cursor, DEFAULT_PAGE_SIZE + 1)
        .api_err("Failed to list tags")?;

    let (tags, next_cursor, has_more) =
        paginate(tags, DEFAULT_PAGE_SIZE as usize, |t| t.name.clone());

    Ok::<_, ApiError>(Json(PaginatedResponse::new(tags, next_cursor, has_more)))
}

pub async fn create_tag(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTagRequest>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();
    let ns_id = resolve_namespace_id(store, principal, req.namespace.as_deref())?;

    require_namespace_permission(store, principal, &ns_id, Permission::NAMESPACE_WRITE)?;

    validate_tag_name(&req.name)?;

    if store
        .get_tag_by_name(&ns_id, &req.name)
        .api_err("Failed to check tag")?
        .is_some()
    {
        return Err(ApiError::conflict("Tag already exists"));
    }

    let tag = Tag {
        id: Uuid::new_v4().to_string(),
        namespace_id: ns_id,
        name: req.name,
        color: req.color,
        created_at: Utc::now(),
    };

    store.create_tag(&tag).api_err("Failed to create tag")?;

    Ok::<_, ApiError>((StatusCode::CREATED, Json(ApiResponse::success(tag))))
}

pub async fn get_tag(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let tag = store
        .get_tag_by_id(&id)
        .api_err("Failed to get tag")?
        .or_not_found("Tag not found")?;

    require_namespace_permission(store, principal, &tag.namespace_id, Permission::NAMESPACE_READ)?;

    Ok::<_, ApiError>(Json(ApiResponse::success(tag)))
}

pub async fn update_tag(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateTagRequest>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let mut tag = store
        .get_tag_by_id(&id)
        .api_err("Failed to get tag")?
        .or_not_found("Tag not found")?;

    require_namespace_permission(store, principal, &tag.namespace_id, Permission::NAMESPACE_WRITE)?;

    if let Some(name) = req.name {
        validate_tag_name(&name)?;

        if name != tag.name
            && store
                .get_tag_by_name(&tag.namespace_id, &name)
                .api_err("Failed to check tag name")?
                .is_some()
        {
            return Err(ApiError::conflict("Tag name already exists"));
        }
        tag.name = name;
    }
    if let Some(color) = req.color {
        tag.color = Some(color);
    }

    store.update_tag(&tag).api_err("Failed to update tag")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(tag)))
}

pub async fn delete_tag(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<DeleteTagParams>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let tag = store
        .get_tag_by_id(&id)
        .api_err("Failed to get tag")?
        .or_not_found("Tag not found")?;

    require_namespace_permission(store, principal, &tag.namespace_id, Permission::NAMESPACE_ADMIN)?;

    let repo_count = store
        .count_tag_repos(&tag.id)
        .api_err("Failed to count tag repos")?;

    if repo_count > 0 && params.force != Some(true) {
        return Err(ApiError::conflict(
            "Tag has repos associated. Use ?force=true to delete anyway",
        ));
    }

    store.delete_tag(&tag.id).api_err("Failed to delete tag")?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}
