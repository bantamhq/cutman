use std::sync::Arc;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;

use super::token::parse_token;
use crate::auth::TokenGenerator;
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

    let raw_token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            header.strip_prefix("Bearer ").unwrap().to_string()
        }
        Some(header) if header.starts_with("Basic ") => extract_basic_auth_token(header)?,
        Some(_) => return Err(AuthError::InvalidScheme),
        None => return Err(AuthError::MissingAuth),
    };

    let (lookup, _secret) = parse_token(&raw_token).map_err(|_| AuthError::InvalidToken)?;

    let token = state
        .store
        .get_token_by_lookup(&lookup)
        .map_err(|_| AuthError::InternalError)?
        .ok_or(AuthError::InvalidToken)?;

    let generator = TokenGenerator::new();
    if !generator
        .verify(&raw_token, &token.token_hash)
        .map_err(|_| AuthError::InternalError)?
    {
        return Err(AuthError::InvalidToken);
    }

    if let Some(expires_at) = &token.expires_at {
        if expires_at < &Utc::now() {
            return Err(AuthError::TokenExpired);
        }
    }

    let _ = state.store.update_token_last_used(&token.id);

    Ok(token)
}

fn extract_basic_auth_token(header: &str) -> Result<String, AuthError> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    let encoded = header
        .strip_prefix("Basic ")
        .ok_or(AuthError::InvalidScheme)?;
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| AuthError::InvalidToken)?;
    let credentials = String::from_utf8(decoded).map_err(|_| AuthError::InvalidToken)?;

    let (username, password) = credentials.split_once(':').ok_or(AuthError::InvalidToken)?;

    if username != "x-token" {
        return Err(AuthError::InvalidToken);
    }

    Ok(password.to_string())
}
