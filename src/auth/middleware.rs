use std::sync::Arc;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
};
use serde_json::json;

use super::helpers::{TokenValidationError, extract_token_from_header, validate_token};
use crate::server::AppState;
use crate::types::{Token, User};

/// An authenticated token (admin or user)
pub struct AuthToken(pub Token);

/// An admin token specifically
pub struct AdminToken(pub Token);

/// Extractor that requires any valid authentication
pub struct RequireAuth(pub Token);

/// Extractor that requires admin authentication
pub struct RequireAdmin(pub Token);

/// Extractor that requires user authentication (non-admin token with user_id)
pub struct RequireUser {
    pub token: Token,
    pub user: User,
}

#[derive(Debug)]
pub enum AuthError {
    MissingAuth,
    InvalidScheme,
    InvalidToken,
    TokenExpired,
    NotAdmin,
    NotUser,
    InternalError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingAuth => (StatusCode::UNAUTHORIZED, "Authentication required"),
            AuthError::InvalidScheme => (StatusCode::UNAUTHORIZED, "Invalid authorization scheme"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token expired"),
            AuthError::NotAdmin => (StatusCode::FORBIDDEN, "Admin access required"),
            AuthError::NotUser => (
                StatusCode::FORBIDDEN,
                "User token required for this operation",
            ),
            AuthError::InternalError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        let body = json!({ "data": null, "error": message });

        let mut response = (status, Json(body)).into_response();

        if status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                "WWW-Authenticate",
                "Bearer realm=\"cutman\"".parse().unwrap(),
            );
        }

        response
    }
}

impl FromRequestParts<Arc<AppState>> for RequireAuth {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_and_validate_token(parts, state).await?;
        Ok(RequireAuth(token))
    }
}

impl FromRequestParts<Arc<AppState>> for RequireAdmin {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_and_validate_token(parts, state).await?;

        if !token.is_admin {
            return Err(AuthError::NotAdmin);
        }

        Ok(RequireAdmin(token))
    }
}

impl FromRequestParts<Arc<AppState>> for RequireUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_and_validate_token(parts, state).await?;

        if token.is_admin {
            return Err(AuthError::NotUser);
        }

        let user_id = token.user_id.as_ref().ok_or(AuthError::NotUser)?;

        let user = state
            .store
            .get_user(user_id)
            .map_err(|_| AuthError::InternalError)?
            .ok_or(AuthError::NotUser)?;

        Ok(RequireUser { token, user })
    }
}

async fn extract_and_validate_token(
    parts: &mut Parts,
    state: &Arc<AppState>,
) -> Result<Token, AuthError> {
    let auth_header = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let raw_token = extract_token_from_header(auth_header)
        .map_err(|e| match e {
            TokenValidationError::InvalidScheme => AuthError::InvalidScheme,
            TokenValidationError::InvalidToken => AuthError::InvalidToken,
            _ => AuthError::InternalError,
        })?
        .ok_or(AuthError::MissingAuth)?;

    let validated = validate_token(state, &raw_token, true).map_err(|e| match e {
        TokenValidationError::InvalidScheme => AuthError::InvalidScheme,
        TokenValidationError::InvalidToken => AuthError::InvalidToken,
        TokenValidationError::TokenExpired => AuthError::TokenExpired,
        TokenValidationError::AdminTokenNotAllowed => AuthError::NotAdmin, // unreachable since allow_admin=true
        TokenValidationError::InternalError => AuthError::InternalError,
    })?;

    Ok(validated.token)
}
