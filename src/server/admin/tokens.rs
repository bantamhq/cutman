use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::auth::RequireAdmin;
use crate::server::AppState;
use crate::server::dto::{
    NamespaceGrantResponse, PaginationParams, RepoGrantResponse, TokenResponse,
};
use crate::server::response::{
    ApiError, ApiResponse, DEFAULT_PAGE_SIZE, PaginatedResponse, paginate,
};
use crate::types::Token;

pub async fn list_tokens(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let cursor = params.cursor.as_deref().unwrap_or("");

    let tokens = state
        .store
        .list_tokens(cursor, DEFAULT_PAGE_SIZE + 1)
        .map_err(|_| ApiError::internal("Failed to list tokens"))?;

    let (tokens, next_cursor, has_more) =
        paginate(tokens, DEFAULT_PAGE_SIZE as usize, |t| t.id.clone());

    let responses: Vec<TokenResponse> = tokens
        .into_iter()
        .map(|t| token_to_response(&state, t))
        .collect::<Result<Vec<_>, _>>()?;

    Ok::<_, ApiError>(Json(PaginatedResponse::new(
        responses,
        next_cursor,
        has_more,
    )))
}

pub async fn get_token(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let token = state
        .store
        .get_token_by_id(&id)
        .map_err(|_| ApiError::internal("Failed to get token"))?
        .ok_or_else(|| ApiError::not_found("Token not found"))?;

    let response = token_to_response(&state, token)?;

    Ok::<_, ApiError>(Json(ApiResponse::success(response)))
}

pub async fn delete_token(
    admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let token = state
        .store
        .get_token_by_id(&id)
        .map_err(|_| ApiError::internal("Failed to get token"))?
        .ok_or_else(|| ApiError::not_found("Token not found"))?;

    if token.id == admin.0.id {
        return Err(ApiError::bad_request("Cannot delete current token"));
    }

    state
        .store
        .delete_token(&token.id)
        .map_err(|_| ApiError::internal("Failed to delete token"))?;

    Ok::<_, ApiError>(StatusCode::NO_CONTENT)
}

pub fn token_to_response(state: &Arc<AppState>, token: Token) -> Result<TokenResponse, ApiError> {
    let mut response = TokenResponse {
        id: token.id,
        is_admin: token.is_admin,
        principal_id: token.principal_id.clone(),
        created_at: token.created_at,
        expires_at: token.expires_at,
        last_used_at: token.last_used_at,
        namespace_grants: Vec::new(),
        repo_grants: Vec::new(),
    };

    if !token.is_admin {
        if let Some(principal_id) = &token.principal_id {
            let ns_grants = state
                .store
                .list_principal_namespace_grants(principal_id)
                .map_err(|_| ApiError::internal("Failed to list namespace grants"))?;

            response.namespace_grants = ns_grants
                .into_iter()
                .map(|g| NamespaceGrantResponse {
                    namespace_id: g.namespace_id,
                    allow: g.allow_bits.to_strings(),
                    deny: g.deny_bits.to_strings(),
                })
                .collect();

            let repo_grants = state
                .store
                .list_principal_repo_grants(principal_id)
                .map_err(|_| ApiError::internal("Failed to list repo grants"))?;

            response.repo_grants = repo_grants
                .into_iter()
                .map(|g| RepoGrantResponse {
                    repo_id: g.repo_id,
                    allow: g.allow_bits.to_strings(),
                    deny: g.deny_bits.to_strings(),
                })
                .collect();
        }
    }

    Ok(response)
}
