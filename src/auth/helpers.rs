use std::sync::Arc;

use chrono::Utc;

use super::{TokenGenerator, parse_token};
use crate::server::AppState;
use crate::types::{Token, User};

#[derive(Debug)]
pub enum TokenValidationError {
    InvalidScheme,
    InvalidToken,
    TokenExpired,
    AdminTokenNotAllowed,
    InternalError,
}

pub struct ValidatedToken {
    pub token: Token,
    pub user: Option<User>,
}

/// Extracts a token string from a Basic auth header.
/// Expects format: Basic base64(x-token:actual_token)
pub fn extract_basic_auth_token(header: &str) -> Option<String> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    let encoded = header.strip_prefix("Basic ")?;
    let decoded = STANDARD.decode(encoded).ok()?;
    let credentials = String::from_utf8(decoded).ok()?;

    let (username, password) = credentials.split_once(':')?;

    if username != "x-token" {
        return None;
    }

    Some(password.to_string())
}

/// Validates a raw token string against the store.
/// Returns the validated token and associated user (if any).
/// Set `allow_admin` to false to reject admin tokens.
pub fn validate_token(
    state: &Arc<AppState>,
    raw_token: &str,
    allow_admin: bool,
) -> Result<ValidatedToken, TokenValidationError> {
    let (lookup, _secret) = parse_token(raw_token).map_err(|_| TokenValidationError::InvalidToken)?;

    let token = state
        .store
        .get_token_by_lookup(&lookup)
        .map_err(|_| TokenValidationError::InternalError)?
        .ok_or(TokenValidationError::InvalidToken)?;

    let generator = TokenGenerator::new();
    if !generator
        .verify(raw_token, &token.token_hash)
        .map_err(|_| TokenValidationError::InternalError)?
    {
        return Err(TokenValidationError::InvalidToken);
    }

    if let Some(expires_at) = &token.expires_at {
        if expires_at < &Utc::now() {
            return Err(TokenValidationError::TokenExpired);
        }
    }

    if !allow_admin && token.is_admin {
        return Err(TokenValidationError::AdminTokenNotAllowed);
    }

    let user = match &token.user_id {
        Some(user_id) => state
            .store
            .get_user(user_id)
            .map_err(|_| TokenValidationError::InternalError)?,
        None => None,
    };

    if let Err(e) = state.store.update_token_last_used(&token.id) {
        tracing::warn!("Failed to update token last_used_at: {e}");
    }

    Ok(ValidatedToken { token, user })
}

/// Extracts token from Authorization header (Bearer or Basic).
/// Returns None if no auth header is present.
/// Returns Some(token_string) if auth header is present and valid format.
/// Returns Err if the auth scheme is unsupported.
pub fn extract_token_from_header(auth_header: Option<&str>) -> Result<Option<String>, TokenValidationError> {
    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            Ok(Some(header.strip_prefix("Bearer ").unwrap().to_string()))
        }
        Some(header) if header.starts_with("Basic ") => {
            extract_basic_auth_token(header)
                .ok_or(TokenValidationError::InvalidToken)
                .map(Some)
        }
        Some(_) => Err(TokenValidationError::InvalidScheme),
        None => Ok(None),
    }
}
