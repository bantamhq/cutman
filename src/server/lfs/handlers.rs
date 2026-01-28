use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use tokio_util::io::ReaderStream;
use tracing::warn;

use super::dto::{
    Action, BatchRequest, BatchResponse, LfsError, ObjectResponse, ObjectSpec, VerifyRequest,
};
use crate::lfs::{LfsStorage, LfsStorageError, is_valid_oid};
use crate::server::AppState;
use crate::server::git::auth::{GitAuth, GitAuthError, check_git_access, extract_git_auth};
use crate::types::{LfsObject, Namespace, Repo};

const LFS_MEDIA_TYPE: &str = "application/vnd.git-lfs+json";

#[derive(serde::Deserialize)]
pub struct LfsPathParams {
    pub namespace: String,
    pub repo: String,
}

#[derive(serde::Deserialize)]
pub struct LfsObjectPathParams {
    pub namespace: String,
    pub repo: String,
    pub oid: String,
}

impl From<&LfsObjectPathParams> for LfsPathParams {
    fn from(params: &LfsObjectPathParams) -> Self {
        Self {
            namespace: params.namespace.clone(),
            repo: params.repo.clone(),
        }
    }
}

struct LfsContext {
    git_auth: GitAuth,
    namespace: Namespace,
    repo: Repo,
}

#[must_use]
fn lfs_json_response<T: serde::Serialize>(status: StatusCode, body: &T) -> Response {
    let json = serde_json::to_vec(body).unwrap_or_default();
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, LFS_MEDIA_TYPE)
        .body(Body::from(json))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[must_use]
fn lfs_error_response(status: StatusCode, message: &str) -> Response {
    lfs_json_response(
        status,
        &LfsError {
            message: message.to_string(),
        },
    )
}

#[must_use]
fn lfs_auth_error_response(err: GitAuthError) -> Response {
    let status = err.status_code();
    let mut response = lfs_error_response(status, err.message());

    if err.requires_auth_header() {
        response.headers_mut().insert(
            "WWW-Authenticate",
            "Basic realm=\"Git LFS\"".parse().unwrap(),
        );
    }

    response
}

#[must_use]
fn strip_git_suffix(name: &str) -> &str {
    name.strip_suffix(".git").unwrap_or(name)
}

async fn resolve_lfs_context(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    params: &LfsPathParams,
) -> Result<LfsContext, GitAuthError> {
    let repo_name = strip_git_suffix(&params.repo).to_lowercase();

    let git_auth = extract_git_auth(headers, state).await?;

    let namespace = state
        .store
        .get_namespace_by_name(&params.namespace)
        .map_err(|_| GitAuthError::InternalError)?
        .ok_or(GitAuthError::NamespaceNotFound)?;

    let repo = state
        .store
        .get_repo(&namespace.id, &repo_name)
        .map_err(|_| GitAuthError::InternalError)?
        .ok_or(GitAuthError::RepoNotFound)?;

    Ok(LfsContext {
        git_auth,
        namespace,
        repo,
    })
}

#[must_use]
fn build_object_url(host: &str, namespace: &str, repo: &str, oid: &str) -> String {
    format!("{host}/git/{namespace}/{repo}.git/info/lfs/objects/{oid}")
}

#[must_use]
fn build_verify_url(host: &str, namespace: &str, repo: &str) -> String {
    format!("{host}/git/{namespace}/{repo}.git/info/lfs/verify")
}

#[must_use]
fn get_host_from_headers(headers: &HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");

    format!("{scheme}://{host}")
}

struct BatchContext<'a> {
    state: &'a Arc<AppState>,
    storage: &'a LfsStorage,
    ctx: &'a LfsContext,
    host: &'a str,
    namespace: &'a str,
    repo: &'a str,
}

pub async fn batch(
    State(state): State<Arc<AppState>>,
    Path(params): Path<LfsPathParams>,
    headers: HeaderMap,
    Json(request): Json<BatchRequest>,
) -> Response {
    let ctx = match resolve_lfs_context(&state, &headers, &params).await {
        Ok(ctx) => ctx,
        Err(e) => return lfs_auth_error_response(e),
    };

    let is_upload = match request.operation.as_str() {
        "download" => false,
        "upload" => true,
        _ => return lfs_error_response(StatusCode::BAD_REQUEST, "Invalid operation"),
    };

    if let Err(e) = check_git_access(
        &state,
        &ctx.git_auth,
        &ctx.namespace,
        Some(&ctx.repo),
        is_upload,
    ) {
        return lfs_auth_error_response(e);
    }

    let storage = LfsStorage::new(&state.data_dir);
    let host = get_host_from_headers(&headers);
    let batch_ctx = BatchContext {
        state: &state,
        storage: &storage,
        ctx: &ctx,
        host: &host,
        namespace: &params.namespace,
        repo: &params.repo,
    };

    let mut objects = Vec::with_capacity(request.objects.len());

    for obj in &request.objects {
        let obj_response = build_object_response(&batch_ctx, obj, is_upload).await;
        objects.push(obj_response);
    }

    let response = BatchResponse {
        transfer: Some("basic".to_string()),
        objects,
    };

    lfs_json_response(StatusCode::OK, &response)
}

async fn build_object_response(
    batch_ctx: &BatchContext<'_>,
    obj: &ObjectSpec,
    is_upload: bool,
) -> ObjectResponse {
    if !is_valid_oid(&obj.oid) {
        return ObjectResponse::with_error(obj.oid.clone(), obj.size, 422, "Invalid OID format");
    }

    let exists_in_storage = batch_ctx
        .storage
        .exists(&batch_ctx.ctx.repo.id, &obj.oid)
        .await
        .unwrap_or(false);
    let exists_in_db = batch_ctx
        .state
        .store
        .get_lfs_object(&batch_ctx.ctx.repo.id, &obj.oid)
        .ok()
        .flatten()
        .is_some();
    let exists = exists_in_storage && exists_in_db;

    if is_upload {
        build_upload_response(
            obj,
            exists,
            batch_ctx.host,
            batch_ctx.namespace,
            batch_ctx.repo,
        )
    } else {
        build_download_response(
            obj,
            exists,
            batch_ctx.host,
            batch_ctx.namespace,
            batch_ctx.repo,
        )
    }
}

fn build_upload_response(
    obj: &ObjectSpec,
    exists: bool,
    host: &str,
    namespace: &str,
    repo: &str,
) -> ObjectResponse {
    if exists {
        return ObjectResponse::exists(obj.oid.clone(), obj.size);
    }

    let actions = HashMap::from([
        (
            "upload".to_string(),
            Action {
                href: build_object_url(host, namespace, repo, &obj.oid),
                header: None,
                expires_in: Some(3600),
            },
        ),
        (
            "verify".to_string(),
            Action {
                href: build_verify_url(host, namespace, repo),
                header: None,
                expires_in: Some(3600),
            },
        ),
    ]);

    ObjectResponse::with_actions(obj.oid.clone(), obj.size, actions)
}

fn build_download_response(
    obj: &ObjectSpec,
    exists: bool,
    host: &str,
    namespace: &str,
    repo: &str,
) -> ObjectResponse {
    if !exists {
        return ObjectResponse::with_error(obj.oid.clone(), obj.size, 404, "Object not found");
    }

    let actions = HashMap::from([(
        "download".to_string(),
        Action {
            href: build_object_url(host, namespace, repo, &obj.oid),
            header: None,
            expires_in: Some(3600),
        },
    )]);

    ObjectResponse::with_actions(obj.oid.clone(), obj.size, actions)
}

pub async fn download(
    State(state): State<Arc<AppState>>,
    Path(params): Path<LfsObjectPathParams>,
    headers: HeaderMap,
) -> Response {
    let ctx = match resolve_lfs_context(&state, &headers, &LfsPathParams::from(&params)).await {
        Ok(ctx) => ctx,
        Err(e) => return lfs_auth_error_response(e),
    };

    if let Err(e) = check_git_access(
        &state,
        &ctx.git_auth,
        &ctx.namespace,
        Some(&ctx.repo),
        false,
    ) {
        return lfs_auth_error_response(e);
    }

    if !is_valid_oid(&params.oid) {
        return lfs_error_response(StatusCode::BAD_REQUEST, "Invalid OID format");
    }

    let storage = LfsStorage::new(&state.data_dir);

    let (reader, size) = match storage.get(&ctx.repo.id, &params.oid).await {
        Ok(result) => result,
        Err(LfsStorageError::NotFound) => {
            return lfs_error_response(StatusCode::NOT_FOUND, "Object not found");
        }
        Err(e) => {
            warn!("LFS storage error: {e}");
            return lfs_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Storage error");
        }
    };

    let stream = ReaderStream::new(reader);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, size)
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

pub async fn upload(
    State(state): State<Arc<AppState>>,
    Path(params): Path<LfsObjectPathParams>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let ctx = match resolve_lfs_context(&state, &headers, &LfsPathParams::from(&params)).await {
        Ok(ctx) => ctx,
        Err(e) => return lfs_auth_error_response(e),
    };

    if let Err(e) = check_git_access(&state, &ctx.git_auth, &ctx.namespace, Some(&ctx.repo), true) {
        return lfs_auth_error_response(e);
    }

    if !is_valid_oid(&params.oid) {
        return lfs_error_response(StatusCode::BAD_REQUEST, "Invalid OID format");
    }

    let content_length = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok());

    let expected_size = match content_length {
        Some(size) => size,
        None => return lfs_error_response(StatusCode::BAD_REQUEST, "Content-Length required"),
    };

    let storage = LfsStorage::new(&state.data_dir);

    if let Err(e) = storage
        .put(&ctx.repo.id, &params.oid, &body, expected_size)
        .await
    {
        return match e {
            LfsStorageError::HashMismatch { .. } => {
                lfs_error_response(StatusCode::BAD_REQUEST, "Hash mismatch")
            }
            _ => {
                warn!("LFS storage error during upload: {e}");
                lfs_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Storage error")
            }
        };
    }

    let lfs_object = LfsObject {
        repo_id: ctx.repo.id.clone(),
        oid: params.oid.clone(),
        size: expected_size,
        created_at: Utc::now(),
    };

    if let Err(e) = state.store.create_lfs_object(&lfs_object) {
        warn!("Failed to create LFS object record: {e}");
    }

    StatusCode::OK.into_response()
}

pub async fn verify(
    State(state): State<Arc<AppState>>,
    Path(params): Path<LfsPathParams>,
    headers: HeaderMap,
    Json(request): Json<VerifyRequest>,
) -> Response {
    let ctx = match resolve_lfs_context(&state, &headers, &params).await {
        Ok(ctx) => ctx,
        Err(e) => return lfs_auth_error_response(e),
    };

    if let Err(e) = check_git_access(&state, &ctx.git_auth, &ctx.namespace, Some(&ctx.repo), true) {
        return lfs_auth_error_response(e);
    }

    if !is_valid_oid(&request.oid) {
        return lfs_error_response(StatusCode::BAD_REQUEST, "Invalid OID format");
    }

    let storage = LfsStorage::new(&state.data_dir);

    let actual_size = match storage.size(&ctx.repo.id, &request.oid).await {
        Ok(size) => size,
        Err(LfsStorageError::NotFound) => {
            return lfs_error_response(StatusCode::NOT_FOUND, "Object not found");
        }
        Err(e) => {
            warn!("LFS storage error during verify: {e}");
            return lfs_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Storage error");
        }
    };

    if actual_size != request.size {
        return lfs_error_response(
            StatusCode::BAD_REQUEST,
            &format!(
                "Size mismatch: expected {}, got {actual_size}",
                request.size
            ),
        );
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, LFS_MEDIA_TYPE)
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
