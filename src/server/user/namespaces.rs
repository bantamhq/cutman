use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::auth::RequirePrincipal;
use crate::server::AppState;
use crate::server::dto::{NamespaceResponse, PrincipalGrantResponse, UpdateNamespaceRequest};
use crate::server::response::{ApiError, ApiResponse, StoreOptionExt, StoreResultExt};
use crate::types::Permission;

use super::access::require_namespace_permission;

pub async fn list_namespaces(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();
    let mut namespaces = Vec::new();

    let primary_ns = store
        .get_namespace(&principal.primary_namespace_id)
        .api_err("Failed to get primary namespace")?
        .ok_or_else(|| ApiError::internal("Primary namespace not found"))?;
    namespaces.push(primary_ns);

    let ns_grants = store
        .list_principal_namespace_grants(&principal.id)
        .api_err("Failed to list namespace grants")?;

    for grant in ns_grants {
        if grant.namespace_id == principal.primary_namespace_id {
            continue;
        }
        if let Some(ns) = store
            .get_namespace(&grant.namespace_id)
            .api_err("Failed to get namespace")?
        {
            namespaces.push(ns);
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
            if !namespaces.iter().any(|ns| ns.id == repo.namespace_id) {
                if let Some(ns) = store
                    .get_namespace(&repo.namespace_id)
                    .api_err("Failed to get namespace")?
                {
                    namespaces.push(ns);
                }
            }
        }
    }

    let responses: Vec<NamespaceResponse> = namespaces
        .into_iter()
        .map(|ns| NamespaceResponse {
            is_primary: ns.id == principal.primary_namespace_id,
            namespace: ns,
        })
        .collect();

    Ok::<_, ApiError>(Json(ApiResponse::success(responses)))
}

pub async fn update_namespace(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<UpdateNamespaceRequest>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let mut ns = store
        .get_namespace_by_name(&name)
        .api_err("Failed to get namespace")?
        .or_not_found("Namespace not found")?;

    require_namespace_permission(store, principal, &ns.id, Permission::NAMESPACE_ADMIN)?;

    if let Some(limit) = req.repo_limit {
        ns.repo_limit = Some(limit);
    }
    if let Some(limit) = req.storage_limit_bytes {
        ns.storage_limit_bytes = Some(limit);
    }

    store
        .update_namespace(&ns)
        .api_err("Failed to update namespace")?;

    Ok::<_, ApiError>(Json(ApiResponse::success(ns)))
}

pub async fn delete_namespace(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let ns = store
        .get_namespace_by_name(&name)
        .api_err("Failed to get namespace")?
        .or_not_found("Namespace not found")?;

    if ns.id == principal.primary_namespace_id {
        return Err(ApiError::forbidden("Cannot delete your primary namespace"));
    }

    require_namespace_permission(store, principal, &ns.id, Permission::NAMESPACE_ADMIN)?;

    store
        .delete_namespace(&ns.id)
        .api_err("Failed to delete namespace")?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}

pub async fn list_namespace_grants(
    auth: RequirePrincipal,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let principal = &auth.principal;
    let store = state.store.as_ref();

    let ns = store
        .get_namespace_by_name(&name)
        .api_err("Failed to get namespace")?
        .or_not_found("Namespace not found")?;

    require_namespace_permission(store, principal, &ns.id, Permission::NAMESPACE_ADMIN)?;

    let grants = store
        .list_namespace_grants_for_namespace(&ns.id)
        .api_err("Failed to list grants")?;

    let responses: Vec<PrincipalGrantResponse> = grants
        .into_iter()
        .map(|g| PrincipalGrantResponse {
            principal_id: g.principal_id,
            allow: g.allow_bits.to_strings(),
            deny: g.deny_bits.to_strings(),
        })
        .collect();

    Ok::<_, ApiError>(Json(ApiResponse::success(responses)))
}
