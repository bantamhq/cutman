use std::sync::Arc;

use axum::http::HeaderMap;

use crate::auth::{TokenValidationError, extract_token_from_header, validate_token};
use crate::server::AppState;
use crate::server::user::access::{check_namespace_permission, check_repo_permission};
use crate::types::{Namespace, Permission, Repo, Token, User};

pub struct GitAuth {
    pub user: Option<User>,
    #[allow(dead_code)]
    pub token: Option<Token>,
}

#[derive(Debug)]
pub enum GitAuthError {
    InvalidCredentials,
    TokenExpired,
    AdminTokenNotAllowed,
    AuthRequired,
    NamespaceNotFound,
    RepoNotFound,
    PermissionDenied,
    InternalError,
    InvalidRepoName,
}

impl GitAuthError {
    pub fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::InvalidCredentials | Self::TokenExpired | Self::AuthRequired => {
                StatusCode::UNAUTHORIZED
            }
            Self::AdminTokenNotAllowed | Self::PermissionDenied => StatusCode::FORBIDDEN,
            Self::NamespaceNotFound | Self::RepoNotFound => StatusCode::NOT_FOUND,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidRepoName => StatusCode::BAD_REQUEST,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "Invalid credentials",
            Self::TokenExpired => "Token expired",
            Self::AdminTokenNotAllowed => "Admin token cannot be used for git operations",
            Self::AuthRequired => "Authentication required",
            Self::NamespaceNotFound => "Namespace not found",
            Self::RepoNotFound => "Repository not found",
            Self::PermissionDenied => "Permission denied",
            Self::InternalError => "Internal server error",
            Self::InvalidRepoName => "Invalid repository name",
        }
    }

    pub fn requires_auth_header(&self) -> bool {
        matches!(
            self,
            Self::InvalidCredentials | Self::TokenExpired | Self::AuthRequired
        )
    }
}

pub async fn extract_git_auth(
    headers: &HeaderMap,
    state: &Arc<AppState>,
) -> Result<GitAuth, GitAuthError> {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let raw_token = match extract_token_from_header(auth_header) {
        Ok(Some(token)) => token,
        Ok(None) => {
            return Ok(GitAuth {
                user: None,
                token: None,
            });
        }
        Err(_) => return Err(GitAuthError::InvalidCredentials),
    };

    let validated = validate_token(state, &raw_token, false).map_err(|e| match e {
        TokenValidationError::InvalidScheme | TokenValidationError::InvalidToken => {
            GitAuthError::InvalidCredentials
        }
        TokenValidationError::TokenExpired => GitAuthError::TokenExpired,
        TokenValidationError::AdminTokenNotAllowed => GitAuthError::AdminTokenNotAllowed,
        TokenValidationError::InternalError => GitAuthError::InternalError,
    })?;

    Ok(GitAuth {
        user: validated.user,
        token: Some(validated.token),
    })
}

pub fn check_git_access(
    state: &Arc<AppState>,
    git_auth: &GitAuth,
    namespace: &Namespace,
    repo: Option<&Repo>,
    is_write: bool,
) -> Result<(), GitAuthError> {
    let is_public_read = !is_write && repo.is_some_and(|r| r.public);

    let user = match &git_auth.user {
        Some(u) => u,
        None if is_public_read => return Ok(()),
        None => return Err(GitAuthError::AuthRequired),
    };

    if is_write {
        check_write_access(state, user, namespace, repo)
    } else {
        check_read_access(state, user, repo)
    }
}

fn check_write_access(
    state: &Arc<AppState>,
    user: &User,
    namespace: &Namespace,
    repo: Option<&Repo>,
) -> Result<(), GitAuthError> {
    let has_permission = match repo {
        Some(r) => check_repo_permission(state.store.as_ref(), user, r, Permission::REPO_WRITE),
        None => check_namespace_permission(
            state.store.as_ref(),
            user,
            &namespace.id,
            Permission::NAMESPACE_WRITE,
        ),
    };

    if !has_permission.map_err(|_| GitAuthError::InternalError)? {
        return Err(GitAuthError::PermissionDenied);
    }

    Ok(())
}

fn check_read_access(
    state: &Arc<AppState>,
    user: &User,
    repo: Option<&Repo>,
) -> Result<(), GitAuthError> {
    let r = repo.ok_or(GitAuthError::RepoNotFound)?;

    if r.public {
        return Ok(());
    }

    let has_read = check_repo_permission(state.store.as_ref(), user, r, Permission::REPO_READ)
        .map_err(|_| GitAuthError::InternalError)?;

    if !has_read {
        return Err(GitAuthError::PermissionDenied);
    }

    Ok(())
}
