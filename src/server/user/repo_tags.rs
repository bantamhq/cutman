use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::auth::RequireUser;
use crate::server::AppState;
use crate::server::dto::RepoTagsRequest;
use crate::server::response::{ApiError, ApiResponse, StoreOptionExt, StoreResultExt};
use crate::store::Store;
use crate::types::{Permission, Repo};

use super::access::require_repo_permission;

fn validate_tags_for_repo(
    store: &dyn Store,
    repo: &Repo,
    tag_ids: &[String],
) -> Result<(), ApiError> {
    for tag_id in tag_ids {
        let tag = store
            .get_tag_by_id(tag_id)
            .api_err("Failed to get tag")?
            .ok_or_else(|| ApiError::not_found(format!("Tag not found: {tag_id}")))?;

        if tag.namespace_id != repo.namespace_id {
            return Err(ApiError::bad_request(
                "Tag must belong to the same namespace as the repository",
            ));
        }
    }
    Ok(())
}

pub async fn list_repo_tags(
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

    let tags = store
        .list_repo_tags(&repo.id)
        .api_err("Failed to list repo tags")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(tags)))
}

pub async fn add_repo_tags(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<RepoTagsRequest>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_WRITE)?;
    validate_tags_for_repo(store, &repo, &req.tag_ids)?;

    for tag_id in &req.tag_ids {
        store
            .add_repo_tag(&repo.id, tag_id)
            .api_err("Failed to add repo tag")?;
    }

    let tags = store
        .list_repo_tags(&repo.id)
        .api_err("Failed to list repo tags")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(tags)))
}

pub async fn set_repo_tags(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<RepoTagsRequest>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_WRITE)?;
    validate_tags_for_repo(store, &repo, &req.tag_ids)?;

    store
        .set_repo_tags(&repo.id, &req.tag_ids)
        .api_err("Failed to set repo tags")?;

    let tags = store
        .list_repo_tags(&repo.id)
        .api_err("Failed to list repo tags")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(tags)))
}

pub async fn remove_repo_tag(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path((id, tag_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let user = &auth.user;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, user, &repo, Permission::REPO_WRITE)?;

    store
        .get_tag_by_id(&tag_id)
        .api_err("Failed to get tag")?
        .or_not_found("Tag not found")?;

    store
        .remove_repo_tag(&repo.id, &tag_id)
        .api_err("Failed to remove repo tag")?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}
