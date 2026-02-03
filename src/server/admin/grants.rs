use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;

use crate::auth::RequireAdmin;
use crate::server::AppState;
use crate::server::dto::{
    NamespaceGrantRequest, NamespaceGrantResponse, RepoGrantRequest, RepoGrantResponse,
};
use crate::server::response::{ApiError, ApiResponse};
use crate::types::{NamespaceGrant, Permission, RepoGrant};

// Path parameter names match the route: /principals/{id}/...

fn parse_permissions(perms: &[String]) -> Result<Permission, ApiError> {
    let mut result = Permission::default();
    for p in perms {
        let parsed = Permission::parse(p)
            .ok_or_else(|| ApiError::bad_request(format!("Invalid permission: {p}")))?;
        result = result.union(parsed);
    }
    Ok(result)
}

pub async fn create_namespace_grant(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(principal_id): Path<String>,
    Json(req): Json<NamespaceGrantRequest>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&principal_id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let ns = state
        .store
        .get_namespace(&req.namespace_id)
        .map_err(|_| ApiError::internal("Failed to get namespace"))?
        .ok_or_else(|| ApiError::not_found("Namespace not found"))?;

    let allow_bits = parse_permissions(&req.allow)?;
    let deny_bits = parse_permissions(&req.deny)?;

    let now = Utc::now();
    let grant = NamespaceGrant {
        principal_id: principal.id.clone(),
        namespace_id: ns.id,
        allow_bits,
        deny_bits,
        created_at: now,
        updated_at: now,
    };

    state.store.upsert_namespace_grant(&grant).map_err(|e| {
        if matches!(e, crate::error::Error::PrimaryNamespaceGrant) {
            ApiError::bad_request("Cannot grant permissions to primary namespace owner")
        } else {
            ApiError::internal("Failed to create grant")
        }
    })?;

    let grants = state
        .store
        .list_principal_namespace_grants(&principal.id)
        .map_err(|_| ApiError::internal("Failed to list grants"))?;

    let responses: Vec<NamespaceGrantResponse> = grants
        .into_iter()
        .map(|g| NamespaceGrantResponse {
            namespace_id: g.namespace_id,
            allow: g.allow_bits.to_strings(),
            deny: g.deny_bits.to_strings(),
        })
        .collect();

    Ok::<_, ApiError>(Json(ApiResponse::success(responses)))
}

pub async fn list_namespace_grants(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(principal_id): Path<String>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&principal_id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let grants = state
        .store
        .list_principal_namespace_grants(&principal.id)
        .map_err(|_| ApiError::internal("Failed to list grants"))?;

    let responses: Vec<NamespaceGrantResponse> = grants
        .into_iter()
        .map(|g| NamespaceGrantResponse {
            namespace_id: g.namespace_id,
            allow: g.allow_bits.to_strings(),
            deny: g.deny_bits.to_strings(),
        })
        .collect();

    Ok::<_, ApiError>(Json(ApiResponse::success(responses)))
}

#[derive(serde::Deserialize)]
pub struct NamespaceGrantPath {
    id: String,
    ns_id: String,
}

pub async fn get_namespace_grant(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(path): Path<NamespaceGrantPath>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&path.id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let grant = state
        .store
        .get_namespace_grant(&principal.id, &path.ns_id)
        .map_err(|_| ApiError::internal("Failed to get grant"))?
        .ok_or_else(|| ApiError::not_found("Grant not found"))?;

    let response = NamespaceGrantResponse {
        namespace_id: grant.namespace_id,
        allow: grant.allow_bits.to_strings(),
        deny: grant.deny_bits.to_strings(),
    };

    Ok::<_, ApiError>(Json(ApiResponse::success(response)))
}

pub async fn delete_namespace_grant(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(path): Path<NamespaceGrantPath>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&path.id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let grant = state
        .store
        .get_namespace_grant(&principal.id, &path.ns_id)
        .map_err(|_| ApiError::internal("Failed to check grant"))?
        .ok_or_else(|| ApiError::not_found("Grant not found"))?;

    state
        .store
        .delete_namespace_grant(&principal.id, &grant.namespace_id)
        .map_err(|_| ApiError::internal("Failed to delete grant"))?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}

pub async fn create_repo_grant(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(principal_id): Path<String>,
    Json(req): Json<RepoGrantRequest>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&principal_id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let repo = state
        .store
        .get_repo_by_id(&req.repo_id)
        .map_err(|_| ApiError::internal("Failed to get repo"))?
        .ok_or_else(|| ApiError::not_found("Repository not found"))?;

    let allow_bits = parse_permissions(&req.allow)?;
    let deny_bits = parse_permissions(&req.deny)?;

    let now = Utc::now();
    let grant = RepoGrant {
        principal_id: principal.id.clone(),
        repo_id: repo.id,
        allow_bits,
        deny_bits,
        created_at: now,
        updated_at: now,
    };

    state
        .store
        .upsert_repo_grant(&grant)
        .map_err(|_| ApiError::internal("Failed to create grant"))?;

    let grants = state
        .store
        .list_principal_repo_grants(&principal.id)
        .map_err(|_| ApiError::internal("Failed to list grants"))?;

    let responses: Vec<RepoGrantResponse> = grants
        .into_iter()
        .map(|g| RepoGrantResponse {
            repo_id: g.repo_id,
            allow: g.allow_bits.to_strings(),
            deny: g.deny_bits.to_strings(),
        })
        .collect();

    Ok::<_, ApiError>(Json(ApiResponse::success(responses)))
}

pub async fn list_repo_grants(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(principal_id): Path<String>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&principal_id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let grants = state
        .store
        .list_principal_repo_grants(&principal.id)
        .map_err(|_| ApiError::internal("Failed to list grants"))?;

    let responses: Vec<RepoGrantResponse> = grants
        .into_iter()
        .map(|g| RepoGrantResponse {
            repo_id: g.repo_id,
            allow: g.allow_bits.to_strings(),
            deny: g.deny_bits.to_strings(),
        })
        .collect();

    Ok::<_, ApiError>(Json(ApiResponse::success(responses)))
}

#[derive(serde::Deserialize)]
pub struct RepoGrantPath {
    id: String,
    repo_id: String,
}

pub async fn get_repo_grant(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(path): Path<RepoGrantPath>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&path.id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let grant = state
        .store
        .get_repo_grant(&principal.id, &path.repo_id)
        .map_err(|_| ApiError::internal("Failed to get grant"))?
        .ok_or_else(|| ApiError::not_found("Grant not found"))?;

    let response = RepoGrantResponse {
        repo_id: grant.repo_id,
        allow: grant.allow_bits.to_strings(),
        deny: grant.deny_bits.to_strings(),
    };

    Ok::<_, ApiError>(Json(ApiResponse::success(response)))
}

pub async fn delete_repo_grant(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(path): Path<RepoGrantPath>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&path.id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let grant = state
        .store
        .get_repo_grant(&principal.id, &path.repo_id)
        .map_err(|_| ApiError::internal("Failed to check grant"))?
        .ok_or_else(|| ApiError::not_found("Grant not found"))?;

    state
        .store
        .delete_repo_grant(&principal.id, &grant.repo_id)
        .map_err(|_| ApiError::internal("Failed to delete grant"))?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}
