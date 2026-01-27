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
    CreateTokenResponse, CreateUserRequest, CreateUserTokenRequest, PaginationParams, TokenResponse,
};
use crate::server::response::{
    ApiError, ApiResponse, DEFAULT_PAGE_SIZE, PaginatedResponse, paginate,
};
use crate::types::{Namespace, NamespaceGrant, Permission, Token, User};

use super::tokens::token_to_response;

pub async fn create_user(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
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

    let existing_user = state
        .store
        .get_user_by_primary_namespace_id(&ns.id)
        .map_err(|_| ApiError::internal("Failed to check existing user"))?;

    if existing_user.is_some() {
        return Err(ApiError::conflict("User already exists for this namespace"));
    }

    let now = Utc::now();
    let user = User {
        id: Uuid::new_v4().to_string(),
        primary_namespace_id: ns.id.clone(),
        created_at: now,
        updated_at: now,
    };

    state
        .store
        .create_user(&user)
        .map_err(|_| ApiError::internal("Failed to create user"))?;

    let grant = NamespaceGrant {
        user_id: user.id.clone(),
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

    Ok((StatusCode::CREATED, Json(ApiResponse::success(user))))
}

pub async fn list_users(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let cursor = params.cursor.as_deref().unwrap_or("");

    let users = state
        .store
        .list_users(cursor, DEFAULT_PAGE_SIZE + 1)
        .map_err(|_| ApiError::internal("Failed to list users"))?;

    let (users, next_cursor, has_more) =
        paginate(users, DEFAULT_PAGE_SIZE as usize, |u| u.id.clone());

    Ok::<_, ApiError>(Json(PaginatedResponse::new(users, next_cursor, has_more)))
}

pub async fn get_user(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let user = state
        .store
        .get_user(&id)
        .map_err(|_| ApiError::internal("Failed to get user"))?
        .ok_or_else(|| ApiError::not_found("User not found"))?;

    Ok::<_, ApiError>(Json(ApiResponse::success(user)))
}

pub async fn delete_user(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let user = state
        .store
        .get_user(&id)
        .map_err(|_| ApiError::internal("Failed to get user"))?
        .ok_or_else(|| ApiError::not_found("User not found"))?;

    state
        .store
        .delete_user(&user.id)
        .map_err(|_| ApiError::internal("Failed to delete user"))?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}

pub async fn list_user_tokens(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let user = state
        .store
        .get_user(&id)
        .map_err(|_| ApiError::internal("Failed to get user"))?
        .ok_or_else(|| ApiError::not_found("User not found"))?;

    let tokens = state
        .store
        .list_user_tokens(&user.id)
        .map_err(|_| ApiError::internal("Failed to list user tokens"))?;

    let responses: Vec<TokenResponse> = tokens
        .into_iter()
        .map(|t| token_to_response(&state, t))
        .collect::<Result<Vec<_>, _>>()?;

    Ok::<_, ApiError>(Json(ApiResponse::success(responses)))
}

pub async fn create_user_token(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<CreateUserTokenRequest>,
) -> impl IntoResponse {
    let user = state
        .store
        .get_user(&id)
        .map_err(|_| ApiError::internal("Failed to get user"))?
        .ok_or_else(|| ApiError::not_found("User not found"))?;

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
            user_id: Some(user.id.clone()),
            created_at: now,
            expires_at,
            last_used_at: None,
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
