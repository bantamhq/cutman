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
use crate::server::dto::{CreateRepoRequest, ListReposParams, UpdateRepoRequest};
use crate::server::response::{
    ApiError, ApiResponse, DEFAULT_PAGE_SIZE, PaginatedResponse, StoreOptionExt, StoreResultExt,
    paginate,
};
use crate::server::validation::validate_repo_name;
use crate::types::{Permission, Repo};

use super::access::{
    check_namespace_permission, require_namespace_permission, require_repo_permission,
    resolve_namespace_id,
};

pub async fn list_repos(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListReposParams>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();
    let cursor = params.cursor.as_deref().unwrap_or("");

    let repos = if let Some(ref ns_name) = params.namespace {
        let ns_id = resolve_namespace_id(store, principal, Some(ns_name))?;

        if check_namespace_permission(store, principal, &ns_id, Permission::NAMESPACE_READ)? {
            store
                .list_repos(&ns_id, cursor, DEFAULT_PAGE_SIZE + 1)
                .api_err("Failed to list repos")?
        } else {
            store
                .list_principal_repos_with_grants(&principal.id, &ns_id)
                .api_err("Failed to list repos")?
        }
    } else {
        let mut all_repos = Vec::new();

        let primary_repos = store
            .list_repos(&principal.primary_namespace_id, cursor, DEFAULT_PAGE_SIZE + 1)
            .api_err("Failed to list repos")?;
        all_repos.extend(primary_repos);

        let ns_grants = store
            .list_principal_namespace_grants(&principal.id)
            .api_err("Failed to list namespace grants")?;

        for grant in ns_grants {
            if grant.namespace_id == principal.primary_namespace_id {
                continue;
            }

            let effective = grant
                .allow_bits
                .expand_implied()
                .difference(grant.deny_bits);
            if effective.has(Permission::NAMESPACE_READ) {
                let repos = store
                    .list_repos(&grant.namespace_id, cursor, DEFAULT_PAGE_SIZE + 1)
                    .api_err("Failed to list repos")?;
                all_repos.extend(repos);
            }
        }

        let repo_grants = store
            .list_principal_repo_grants(&principal.id)
            .api_err("Failed to list repo grants")?;

        for grant in repo_grants {
            if let Some(repo) = store
                .get_repo_by_id(&grant.repo_id)
                .api_err("Failed to get repo")?
            {
                if !all_repos.iter().any(|r| r.id == repo.id) {
                    all_repos.push(repo);
                }
            }
        }

        all_repos.sort_by(|a, b| a.name.cmp(&b.name));
        all_repos
    };

    let (repos, next_cursor, has_more) =
        paginate(repos, DEFAULT_PAGE_SIZE as usize, |r| r.name.clone());

    Ok::<_, ApiError>(Json(PaginatedResponse::new(repos, next_cursor, has_more)))
}

pub async fn create_repo(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRepoRequest>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    validate_repo_name(&req.name)?;

    let ns_id = resolve_namespace_id(store, principal, req.namespace.as_deref())?;

    require_namespace_permission(store, principal, &ns_id, Permission::NAMESPACE_WRITE)?;

    if store
        .get_repo(&ns_id, &req.name)
        .api_err("Failed to check repo")?
        .is_some()
    {
        return Err(ApiError::conflict("Repository already exists"));
    }

    let now = Utc::now();
    let repo = Repo {
        id: Uuid::new_v4().to_string(),
        namespace_id: ns_id,
        name: req.name,
        description: req.description,
        public: req.public,
        folder_id: None,
        size_bytes: 0,
        last_push_at: None,
        created_at: now,
        updated_at: now,
    };

    store.create_repo(&repo).api_err("Failed to create repo")?;

    Ok::<_, ApiError>((StatusCode::CREATED, Json(ApiResponse::success(repo))))
}

pub async fn get_repo(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, principal, &repo, Permission::REPO_READ)?;

    Ok::<_, ApiError>(Json(ApiResponse::success(repo)))
}

pub async fn update_repo(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRepoRequest>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let mut repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, principal, &repo, Permission::REPO_WRITE)?;

    if let Some(name) = req.name {
        if name != repo.name
            && store
                .get_repo(&repo.namespace_id, &name)
                .api_err("Failed to check repo name")?
                .is_some()
        {
            return Err(ApiError::conflict("Repository name already exists"));
        }
        repo.name = name;
    }
    if let Some(description) = req.description {
        repo.description = Some(description);
    }
    if let Some(public) = req.public {
        repo.public = public;
    }

    store.update_repo(&repo).api_err("Failed to update repo")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(repo)))
}

pub async fn delete_repo(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let repo = store
        .get_repo_by_id(&id)
        .api_err("Failed to get repo")?
        .or_not_found("Repository not found")?;

    require_repo_permission(store, principal, &repo, Permission::REPO_ADMIN)?;

    store
        .delete_repo(&repo.id)
        .api_err("Failed to delete repo")?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}
