use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::auth::{RequireAdmin, TokenGenerator};
use crate::server::AppState;
use crate::server::dto::{
    CreatePrincipalRequest, CreatePrincipalTokenRequest, CreateTokenResponse, PaginationParams,
    TokenResponse,
};
use crate::server::response::{
    ApiError, ApiResponse, DEFAULT_PAGE_SIZE, PaginatedResponse, paginate,
};
use crate::server::user::access::check_repo_permission;
use crate::server::validation::validate_namespace_name;
use crate::types::{Namespace, NamespaceGrant, Permission, Principal, Token};

use super::tokens::token_to_response;

pub async fn create_principal(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreatePrincipalRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate_namespace_name(&req.namespace_name) {
        return Err(ApiError::bad_request(e));
    }

    let ns = match state.store.get_namespace_by_name(&req.namespace_name) {
        Ok(Some(ns)) => ns,
        Ok(None) => {
            let ns = Namespace {
                id: Uuid::new_v4().to_string(),
                name: req.namespace_name.clone(),
                created_at: Utc::now(),
                repo_limit: None,
                storage_limit_bytes: None,
                external_id: None,
            };
            state
                .store
                .create_namespace(&ns)
                .map_err(|_| ApiError::internal("Failed to create namespace"))?;
            ns
        }
        Err(_) => return Err(ApiError::internal("Failed to check namespace")),
    };

    let existing_principal = state
        .store
        .get_principal_by_primary_namespace_id(&ns.id)
        .map_err(|_| ApiError::internal("Failed to check existing principal"))?;

    if existing_principal.is_some() {
        return Err(ApiError::conflict(
            "Principal already exists for this namespace",
        ));
    }

    let now = Utc::now();
    let principal = Principal {
        id: Uuid::new_v4().to_string(),
        primary_namespace_id: ns.id.clone(),
        created_at: now,
        updated_at: now,
    };

    state
        .store
        .create_principal(&principal)
        .map_err(|_| ApiError::internal("Failed to create principal"))?;

    let grant = NamespaceGrant {
        principal_id: principal.id.clone(),
        namespace_id: ns.id,
        allow_bits: Permission::default_namespace_grant(),
        deny_bits: Permission::default(),
        created_at: now,
        updated_at: now,
    };

    state
        .store
        .upsert_namespace_grant(&grant)
        .map_err(|_| ApiError::internal("Failed to create grant"))?;

    Ok((StatusCode::CREATED, Json(ApiResponse::success(principal))))
}

pub async fn list_principals(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let cursor = params.cursor.as_deref().unwrap_or("");

    let principals = state
        .store
        .list_principals(cursor, DEFAULT_PAGE_SIZE + 1)
        .map_err(|_| ApiError::internal("Failed to list principals"))?;

    let (principals, next_cursor, has_more) =
        paginate(principals, DEFAULT_PAGE_SIZE as usize, |p| p.id.clone());

    Ok::<_, ApiError>(Json(PaginatedResponse::new(
        principals,
        next_cursor,
        has_more,
    )))
}

pub async fn get_principal(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    Ok::<_, ApiError>(Json(ApiResponse::success(principal)))
}

pub async fn delete_principal(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    state
        .store
        .delete_principal(&principal.id)
        .map_err(|_| ApiError::internal("Failed to delete principal"))?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}

pub async fn list_principal_tokens(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    let tokens = state
        .store
        .list_principal_tokens(&principal.id)
        .map_err(|_| ApiError::internal("Failed to list principal tokens"))?;

    let responses: Vec<TokenResponse> = tokens
        .into_iter()
        .map(|t| token_to_response(&state, t))
        .collect::<Result<Vec<_>, _>>()?;

    Ok::<_, ApiError>(Json(ApiResponse::success(responses)))
}

pub async fn create_principal_token(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<CreatePrincipalTokenRequest>,
) -> impl IntoResponse {
    let principal = state
        .store
        .get_principal(&id)
        .map_err(|_| ApiError::internal("Failed to get principal"))?
        .ok_or_else(|| ApiError::not_found("Principal not found"))?;

    if let Some(seconds) = req.expires_in_seconds {
        if seconds < 0 {
            return Err(ApiError::bad_request(
                "expires_in_seconds cannot be negative",
            ));
        }
    }

    let expires_at = req
        .expires_in_seconds
        .map(|s| Utc::now() + Duration::seconds(s));

    let (scope_repo_id, scope) = resolve_token_scope(&state, &principal, &req)?;

    let generator = TokenGenerator::new();

    const MAX_RETRIES: u32 = 3;
    for _ in 0..MAX_RETRIES {
        let (raw_token, lookup, hash) = generator
            .generate()
            .map_err(|_| ApiError::internal("Failed to generate token"))?;

        let now = Utc::now();
        let token = Token {
            id: Uuid::new_v4().to_string(),
            token_hash: hash,
            token_lookup: lookup,
            is_admin: false,
            principal_id: Some(principal.id.clone()),
            created_at: now,
            expires_at,
            last_used_at: None,
            scope_repo_id: scope_repo_id.clone(),
            scope,
        };

        match state.store.create_token(&token) {
            Ok(()) => {
                let response = token_to_response(&state, token)?;
                return Ok((
                    StatusCode::CREATED,
                    Json(ApiResponse::success(CreateTokenResponse {
                        token: raw_token,
                        metadata: response,
                    })),
                ));
            }
            Err(crate::error::Error::TokenLookupCollision) => continue,
            Err(_) => return Err(ApiError::internal("Failed to create token")),
        }
    }

    Err(ApiError::internal("Failed to create token after retries"))
}

fn resolve_token_scope(
    state: &Arc<AppState>,
    principal: &Principal,
    req: &CreatePrincipalTokenRequest,
) -> Result<(Option<String>, Option<Permission>), ApiError> {
    let (repo_id, allow) = match (&req.repo_id, &req.allow) {
        (None, None) => return Ok((None, None)),
        (Some(repo_id), Some(allow)) => (repo_id, allow),
        _ => {
            return Err(ApiError::bad_request(
                "repo_id and allow must be provided together",
            ));
        }
    };

    if allow.is_empty() {
        return Err(ApiError::bad_request("allow cannot be empty"));
    }

    let allow_strs: Vec<&str> = allow.iter().map(String::as_str).collect();
    let scope = Permission::parse_many(&allow_strs)
        .ok_or_else(|| ApiError::bad_request("Invalid permission in allow"))?;

    let repo_permissions = Permission::REPO_READ
        .union(Permission::REPO_WRITE)
        .union(Permission::REPO_ADMIN);

    if scope.difference(repo_permissions) != Permission::default() {
        return Err(ApiError::bad_request(
            "Scoped tokens only support repo permissions",
        ));
    }

    let repo = state
        .store
        .get_repo_by_id(repo_id)
        .map_err(|_| ApiError::internal("Failed to get repo"))?
        .ok_or_else(|| ApiError::not_found("Repo not found"))?;

    let expanded = scope.expand_implied();
    for required in [
        Permission::REPO_READ,
        Permission::REPO_WRITE,
        Permission::REPO_ADMIN,
    ] {
        if expanded.has(required)
            && !check_repo_permission(state.store.as_ref(), principal, &repo, required)?
        {
            return Err(ApiError::bad_request(
                "Scope cannot exceed the principal's permissions on the repo",
            ));
        }
    }

    Ok((Some(repo.id), Some(scope)))
}
