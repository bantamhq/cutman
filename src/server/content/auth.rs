use std::sync::Arc;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::auth::{TokenValidationError, extract_token_from_header, validate_token};
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
            Self::AdminTokenNotAllowed => (
                StatusCode::FORBIDDEN,
                "Admin token cannot be used for this operation",
            ),
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

        let raw_token = match extract_token_from_header(auth_header) {
            Ok(Some(token)) => token,
            Ok(None) => {
                return Ok(OptionalAuth {
                    user: None,
                    token: None,
                });
            }
            Err(e) => {
                return Err(match e {
                    TokenValidationError::InvalidScheme => OptionalAuthError::InvalidScheme,
                    TokenValidationError::InvalidToken => OptionalAuthError::InvalidToken,
                    _ => OptionalAuthError::InternalError,
                });
            }
        };

        let validated = validate_token(state, &raw_token, false).map_err(|e| match e {
            TokenValidationError::InvalidScheme => OptionalAuthError::InvalidScheme,
            TokenValidationError::InvalidToken => OptionalAuthError::InvalidToken,
            TokenValidationError::TokenExpired => OptionalAuthError::TokenExpired,
            TokenValidationError::AdminTokenNotAllowed => OptionalAuthError::AdminTokenNotAllowed,
            TokenValidationError::InternalError => OptionalAuthError::InternalError,
        })?;

        Ok(OptionalAuth {
            user: validated.user,
            token: Some(validated.token),
        })
    }
}

pub fn check_content_access(
    state: &Arc<AppState>,
    auth: &OptionalAuth,
    repo: &Repo,
) -> Result<(), ApiError> {
    if repo.public {
        return Ok(());
    }

    let user = auth
        .user
        .as_ref()
        .ok_or_else(|| ApiError::unauthorized("Authentication required"))?;

    let has_read = check_repo_permission(state.store.as_ref(), user, repo, Permission::REPO_READ)?;

    if !has_read {
        return Err(ApiError::forbidden("Access denied"));
    }

    Ok(())
}
