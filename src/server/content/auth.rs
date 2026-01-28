use std::sync::Arc;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;

use crate::auth::{TokenGenerator, parse_token};
use crate::server::AppState;
use crate::server::response::ApiError;
use crate::server::user::access::check_repo_permission;
use crate::types::{Permission, Repo, Token, User};

pub struct OptionalAuth {
    pub user: Option<User>,
    #[allow(dead_code)]
    pub token: Option<Token>,
}

#[derive(Debug)]
pub enum OptionalAuthError {
    InvalidScheme,
    InvalidToken,
    TokenExpired,
    AdminTokenNotAllowed,
    InternalError,
}

impl IntoResponse for OptionalAuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::InvalidScheme => (StatusCode::UNAUTHORIZED, "Invalid authorization scheme"),
            Self::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            Self::TokenExpired => (StatusCode::UNAUTHORIZED, "Token expired"),
            Self::AdminTokenNotAllowed => {
                (StatusCode::FORBIDDEN, "Admin token cannot be used for this operation")
            }
            Self::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
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

impl FromRequestParts<Arc<AppState>> for OptionalAuth {
    type Rejection = OptionalAuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok());

        let raw_token = match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                header.strip_prefix("Bearer ").unwrap().to_string()
            }
            Some(header) if header.starts_with("Basic ") => {
                extract_basic_auth_token(header)?
            }
            Some(_) => return Err(OptionalAuthError::InvalidScheme),
            None => {
                return Ok(OptionalAuth {
                    user: None,
                    token: None,
                });
            }
        };

        let (lookup, _secret) =
            parse_token(&raw_token).map_err(|_| OptionalAuthError::InvalidToken)?;

        let token = state
            .store
            .get_token_by_lookup(&lookup)
            .map_err(|_| OptionalAuthError::InternalError)?
            .ok_or(OptionalAuthError::InvalidToken)?;

        let generator = TokenGenerator::new();
        if !generator
            .verify(&raw_token, &token.token_hash)
            .map_err(|_| OptionalAuthError::InternalError)?
        {
            return Err(OptionalAuthError::InvalidToken);
        }

        if let Some(expires_at) = &token.expires_at {
            if expires_at < &Utc::now() {
                return Err(OptionalAuthError::TokenExpired);
            }
        }

        if token.is_admin {
            return Err(OptionalAuthError::AdminTokenNotAllowed);
        }

        let user = match &token.user_id {
            Some(user_id) => state
                .store
                .get_user(user_id)
                .map_err(|_| OptionalAuthError::InternalError)?,
            None => None,
        };

        let _ = state.store.update_token_last_used(&token.id);

        Ok(OptionalAuth {
            user,
            token: Some(token),
        })
    }
}

fn extract_basic_auth_token(header: &str) -> Result<String, OptionalAuthError> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    let encoded = header
        .strip_prefix("Basic ")
        .ok_or(OptionalAuthError::InvalidScheme)?;
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| OptionalAuthError::InvalidToken)?;
    let credentials = String::from_utf8(decoded).map_err(|_| OptionalAuthError::InvalidToken)?;

    let (username, password) = credentials
        .split_once(':')
        .ok_or(OptionalAuthError::InvalidToken)?;

    if username != "x-token" {
        return Err(OptionalAuthError::InvalidToken);
    }

    Ok(password.to_string())
}

pub fn check_content_access(
    state: &Arc<AppState>,
    auth: &OptionalAuth,
    repo: &Repo,
) -> Result<(), ApiError> {
    if repo.public {
        return Ok(());
    }

    let user = auth.user.as_ref().ok_or_else(|| {
        ApiError::unauthorized("Authentication required")
    })?;

    let has_read = check_repo_permission(state.store.as_ref(), user, repo, Permission::REPO_READ)?;

    if !has_read {
        return Err(ApiError::forbidden("Access denied"));
    }

    Ok(())
}
