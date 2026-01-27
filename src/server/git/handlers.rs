use std::sync::Arc;

use async_compression::tokio::bufread::GzipDecoder;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tracing::warn;
use uuid::Uuid;

use super::auth::{GitAuth, GitAuthError, check_git_access, extract_git_auth};
use super::process::{
    GitService, calculate_repo_size, format_pkt_line_header, init_bare_repo, repo_path,
    run_git_command,
};
use crate::server::AppState;
use crate::types::{Namespace, Repo};

#[derive(Deserialize)]
pub struct InfoRefsQuery {
    service: Option<String>,
}

#[derive(Deserialize)]
pub struct GitPathParams {
    namespace: String,
    repo: String,
}

struct GitContext {
    git_auth: GitAuth,
    namespace: Namespace,
    repo: Option<Repo>,
    repo_name: String,
}

fn git_error_response(err: GitAuthError) -> Response {
    let mut response = (err.status_code(), err.message()).into_response();

    if err.requires_auth_header() {
        response.headers_mut().insert(
            "WWW-Authenticate",
            "Basic realm=\"cutman\"".parse().unwrap(),
        );
    }

    response
}

fn strip_git_suffix(name: &str) -> &str {
    name.strip_suffix(".git").unwrap_or(name)
}

fn validate_repo_name(name: &str) -> Result<(), GitAuthError> {
    if name.is_empty() || name.len() > 100 {
        return Err(GitAuthError::InvalidRepoName);
    }

    let valid = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');

    if !valid {
        return Err(GitAuthError::InvalidRepoName);
    }

    Ok(())
}

async fn resolve_git_context(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    params: &GitPathParams,
) -> Result<GitContext, GitAuthError> {
    let repo_name = strip_git_suffix(&params.repo).to_lowercase();
    validate_repo_name(&repo_name)?;

    let git_auth = extract_git_auth(headers, state).await?;

    let namespace = state
        .store
        .get_namespace_by_name(&params.namespace)
        .map_err(|_| GitAuthError::InternalError)?
        .ok_or(GitAuthError::NamespaceNotFound)?;

    let repo = state
        .store
        .get_repo(&namespace.id, &repo_name)
        .map_err(|_| GitAuthError::InternalError)?;

    Ok(GitContext {
        git_auth,
        namespace,
        repo,
        repo_name,
    })
}

fn build_git_response(body: Vec<u8>, content_type: &str) -> Response {
    let mut response = body.into_response();
    response
        .headers_mut()
        .insert("Content-Type", content_type.parse().unwrap());
    response
        .headers_mut()
        .insert("Cache-Control", "no-cache".parse().unwrap());
    response
}

pub async fn info_refs(
    State(state): State<Arc<AppState>>,
    Path(params): Path<GitPathParams>,
    Query(query): Query<InfoRefsQuery>,
    headers: HeaderMap,
) -> Response {
    let service = match query.service.as_deref().and_then(GitService::from_str) {
        Some(s) => s,
        None => return (StatusCode::BAD_REQUEST, "Invalid service").into_response(),
    };

    let ctx = match resolve_git_context(&state, &headers, &params).await {
        Ok(ctx) => ctx,
        Err(e) => return git_error_response(e),
    };

    let is_write = service.is_write();

    if let Err(e) = check_git_access(&state, &ctx.git_auth, &ctx.namespace, ctx.repo.as_ref(), is_write) {
        return git_error_response(e);
    }

    let repo = if is_write && ctx.repo.is_none() {
        match create_repo_for_push(&state, &ctx.namespace.id, &ctx.repo_name).await {
            Ok(r) => Some(r),
            Err(e) => return e,
        }
    } else if ctx.repo.is_none() {
        return git_error_response(GitAuthError::RepoNotFound);
    } else {
        ctx.repo
    };

    let path = repo_path(&state.data_dir, &ctx.namespace.id, &ctx.repo_name);

    if is_write && !path.exists() {
        if let Err(e) = init_bare_repo(&path).await {
            warn!("Failed to init bare repo: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to initialize repository",
            )
                .into_response();
        }
    }

    if !path.exists() && repo.is_some() {
        return git_error_response(GitAuthError::RepoNotFound);
    }

    let output = match run_git_command(&path, service, true, None).await {
        Ok(o) => o,
        Err(e) => {
            warn!("Git command failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Git command failed: {stderr}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
    }

    let mut body = format_pkt_line_header(service);
    body.extend_from_slice(&output.stdout);

    build_git_response(body, service.advertisement_content_type())
}

pub async fn git_upload_pack(
    State(state): State<Arc<AppState>>,
    Path(params): Path<GitPathParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let ctx = match resolve_git_context(&state, &headers, &params).await {
        Ok(ctx) => ctx,
        Err(e) => return git_error_response(e),
    };

    let repo = match ctx.repo {
        Some(r) => r,
        None => return git_error_response(GitAuthError::RepoNotFound),
    };

    if let Err(e) = check_git_access(&state, &ctx.git_auth, &ctx.namespace, Some(&repo), false) {
        return git_error_response(e);
    }

    let path = repo_path(&state.data_dir, &ctx.namespace.id, &ctx.repo_name);

    if !path.exists() {
        return git_error_response(GitAuthError::RepoNotFound);
    }

    let input = match decompress_if_gzip(&headers, body).await {
        Ok(data) => data,
        Err(e) => return e,
    };

    let output = match run_git_command(&path, GitService::UploadPack, false, Some(&input)).await {
        Ok(o) => o,
        Err(e) => {
            warn!("git-upload-pack failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
        }
    };

    build_git_response(output.stdout, GitService::UploadPack.content_type())
}

pub async fn git_receive_pack(
    State(state): State<Arc<AppState>>,
    Path(params): Path<GitPathParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let ctx = match resolve_git_context(&state, &headers, &params).await {
        Ok(ctx) => ctx,
        Err(e) => return git_error_response(e),
    };

    if let Err(e) = check_git_access(&state, &ctx.git_auth, &ctx.namespace, ctx.repo.as_ref(), true) {
        return git_error_response(e);
    }

    let repo = match ctx.repo {
        Some(r) => r,
        None => match create_repo_for_push(&state, &ctx.namespace.id, &ctx.repo_name).await {
            Ok(r) => r,
            Err(e) => return e,
        },
    };

    let path = repo_path(&state.data_dir, &ctx.namespace.id, &ctx.repo_name);

    if !path.exists() {
        if let Err(e) = init_bare_repo(&path).await {
            warn!("Failed to init bare repo: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to initialize repository",
            )
                .into_response();
        }
    }

    let input = match decompress_if_gzip(&headers, body).await {
        Ok(data) => data,
        Err(e) => return e,
    };

    let output = match run_git_command(&path, GitService::ReceivePack, false, Some(&input)).await {
        Ok(o) => o,
        Err(e) => {
            warn!("git-receive-pack failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
        }
    };

    if let Err(e) = state.store.update_repo_last_push(&repo.id) {
        warn!("Failed to update last_push_at: {e}");
    }

    if let Ok(size) = calculate_repo_size(&path).await {
        if let Err(e) = state.store.update_repo_size(&repo.id, size) {
            warn!("Failed to update repo size: {e}");
        }
    }

    build_git_response(output.stdout, GitService::ReceivePack.content_type())
}

async fn decompress_if_gzip(headers: &HeaderMap, body: Bytes) -> Result<Vec<u8>, Response> {
    let content_encoding = headers
        .get("Content-Encoding")
        .and_then(|v| v.to_str().ok());

    if content_encoding == Some("gzip") {
        let reader = std::io::Cursor::new(body);
        let mut decoder = GzipDecoder::new(tokio::io::BufReader::new(reader));
        let mut decompressed = Vec::new();

        decoder
            .read_to_end(&mut decompressed)
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid gzip body").into_response())?;

        Ok(decompressed)
    } else {
        Ok(body.to_vec())
    }
}

async fn create_repo_for_push(
    state: &Arc<AppState>,
    namespace_id: &str,
    repo_name: &str,
) -> Result<Repo, Response> {
    let now = Utc::now();
    let repo = Repo {
        id: Uuid::new_v4().to_string(),
        namespace_id: namespace_id.to_string(),
        name: repo_name.to_string(),
        description: None,
        public: false,
        size_bytes: 0,
        last_push_at: None,
        created_at: now,
        updated_at: now,
    };

    state.store.create_repo(&repo).map_err(|e| {
        warn!("Failed to create repo: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create repository",
        )
            .into_response()
    })?;

    Ok(repo)
}
