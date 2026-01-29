use std::io::Write;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression;
use flate2::write::GzEncoder;
use git2::{ObjectType, Oid};

use crate::server::AppState;
use crate::server::response::{
    ApiError, ApiResponse, PaginatedResponse, StoreOptionExt, StoreResultExt,
};

use crate::auth::RequireUser;
use crate::server::user::access::require_repo_permission;
use crate::types::Permission;

use super::auth::{OptionalAuth, check_content_access};
use super::dto::{
    ArchiveParams, BlameLineResponse, BlameResponse, CommitAction, CompareParams, CompareResponse,
    CreateRefRequest, DEFAULT_PAGE_SIZE, DEFAULT_TREE_DEPTH, DeleteBlobRequest, DiffResponse,
    EnhancedBlobParams, EnhancedBlobResponse, FileInfo, ListCommitsParams, MAX_BLOB_SIZE,
    MAX_PAGE_SIZE, MAX_RAW_BLOB_SIZE, MAX_TREE_DEPTH, MultiCommitRequest, MutationResponse,
    PathSearchParams, PathSearchResponse, PutBlobRequest, ReadmeParams, ReadmeResponse,
    RefResponse, SetDefaultBranchRequest, TreeEntryResponse, TreeParams, UpdateRefRequest,
};

const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
use super::git_ops::{
    CommitActionOp, GitError, apply_actions, build_diff, commit_to_response, compute_commit_stats,
    count_ahead_behind, create_commit_on_branch, create_ref, delete_ref, entry_type_str,
    file_exists, find_merge_base, get_blob_at_path, get_commit, get_default_branch,
    get_file_history, get_tree, get_tree_at_path, is_binary, open_repo, resolve_ref, search_paths,
    set_default_branch, signature_to_response, tree_with_blob, tree_without_entry, update_ref,
    verify_blob_sha,
};

fn repo_path(state: &AppState, namespace_id: &str, repo_name: &str) -> std::path::PathBuf {
    state
        .data_dir
        .join("repos")
        .join(namespace_id)
        .join(format!("{repo_name}.git"))
}

async fn load_repo_and_check_access(
    state: &Arc<AppState>,
    auth: &OptionalAuth,
    repo_id: &str,
) -> Result<(crate::types::Repo, git2::Repository), ApiError> {
    let repo = state
        .store
        .get_repo_by_id(repo_id)
        .api_err("Failed to get repository")?
        .or_not_found("Repository not found")?;

    check_content_access(state, auth, &repo)?;

    let path = repo_path(state, &repo.namespace_id, &repo.name);
    let git_repo = open_repo(&path)?;

    Ok((repo, git_repo))
}

pub async fn list_refs(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let default_branch = get_default_branch(&git_repo);
    let mut refs = Vec::new();

    if let Ok(branches) = git_repo.branches(None) {
        for (branch, _branch_type) in branches.flatten() {
            if let Some(name) = branch.name().ok().flatten() {
                if let Some(reference) = branch.get().target() {
                    refs.push(RefResponse {
                        name: name.to_string(),
                        ref_type: "branch".to_string(),
                        commit_sha: reference.to_string(),
                        is_default: default_branch.as_deref() == Some(name),
                    });
                }
            }
        }
    }

    if let Ok(tag_names) = git_repo.tag_names(None) {
        for tag_name in tag_names.iter().flatten() {
            let tag_ref = format!("refs/tags/{tag_name}");
            if let Ok(reference) = git_repo.find_reference(&tag_ref) {
                if let Some(oid) = reference.target() {
                    let commit_sha = if let Ok(tag) = git_repo.find_tag(oid) {
                        tag.target_id().to_string()
                    } else {
                        oid.to_string()
                    };

                    refs.push(RefResponse {
                        name: tag_name.to_string(),
                        ref_type: "tag".to_string(),
                        commit_sha,
                        is_default: false,
                    });
                }
            }
        }
    }

    if refs.is_empty() {
        return Err(GitError::EmptyRepo.into());
    }

    refs.sort_by(|a, b| match (a.is_default, b.is_default) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => match (a.ref_type.as_str(), b.ref_type.as_str()) {
            ("branch", "tag") => std::cmp::Ordering::Less,
            ("tag", "branch") => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        },
    });

    Ok(Json(ApiResponse::success(refs)))
}

pub async fn list_commits(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<ListCommitsParams>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let ref_name = params.ref_name.as_deref().unwrap_or("");
    let oid = resolve_ref(&git_repo, ref_name)?;

    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE) as usize;

    let start_oid = if let Some(ref cursor) = params.cursor {
        Oid::from_str(cursor).map_err(|_| ApiError::bad_request("Invalid cursor"))?
    } else {
        oid
    };

    let mut revwalk = git_repo
        .revwalk()
        .map_err(|e| ApiError::internal(format!("Failed to create revwalk: {e}")))?;

    revwalk
        .push(start_oid)
        .map_err(|e| ApiError::internal(format!("Failed to start revwalk: {e}")))?;

    if params.cursor.is_some() {
        revwalk.next();
    }

    let path_filter = params.path.as_deref().map(|p| p.trim_start_matches('/'));

    let mut commits = Vec::new();
    let mut count = 0;

    for oid_result in revwalk {
        if count > limit {
            break;
        }

        let commit_oid =
            oid_result.map_err(|e| ApiError::internal(format!("Revwalk error: {e}")))?;
        let commit = get_commit(&git_repo, commit_oid)?;

        if let Some(filter_path) = path_filter {
            if !commit_touches_path(&commit, filter_path) {
                continue;
            }
        }

        let stats = compute_commit_stats(&git_repo, &commit);
        commits.push(commit_to_response(&commit, stats));
        count += 1;
    }

    let has_more = commits.len() > limit;
    let next_cursor = if has_more {
        commits.pop();
        commits.last().map(|c| c.sha.clone())
    } else {
        None
    };

    Ok(Json(PaginatedResponse::new(commits, next_cursor, has_more)))
}

fn commit_touches_path(commit: &git2::Commit<'_>, path: &str) -> bool {
    let Ok(tree) = commit.tree() else {
        return false;
    };

    let path = std::path::Path::new(path);
    let current_entry = tree.get_path(path).ok();

    if commit.parent_count() == 0 {
        return current_entry.is_some();
    }

    let Ok(parent) = commit.parent(0) else {
        return current_entry.is_some();
    };

    let Ok(parent_tree) = parent.tree() else {
        return current_entry.is_some();
    };

    let parent_entry = parent_tree.get_path(path).ok();

    match (&current_entry, &parent_entry) {
        (Some(curr), Some(par)) => curr.id() != par.id(),
        (Some(_), None) | (None, Some(_)) => true,
        (None, None) => false,
    }
}

pub async fn get_commit_handler(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, sha)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let oid = resolve_ref(&git_repo, &sha)?;
    let commit = get_commit(&git_repo, oid)?;
    let stats = compute_commit_stats(&git_repo, &commit);

    Ok(Json(ApiResponse::success(commit_to_response(
        &commit, stats,
    ))))
}

pub async fn get_commit_diff(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, sha)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let oid = resolve_ref(&git_repo, &sha)?;
    let commit = get_commit(&git_repo, oid)?;
    let head_tree = get_tree(&git_repo, &commit)?;

    let (base_sha, parent_tree) = if commit.parent_count() > 0 {
        let parent = commit
            .parent(0)
            .map_err(|e| ApiError::internal(format!("Failed to get parent: {e}")))?;
        let tree = get_tree(&git_repo, &parent)?;
        (Some(parent.id().to_string()), Some(tree))
    } else {
        (None, None)
    };

    let (patch, stats) = build_diff(&git_repo, parent_tree.as_ref(), &head_tree)?;

    Ok(Json(ApiResponse::success(DiffResponse {
        base_sha,
        head_sha: oid.to_string(),
        stats,
        patch,
    })))
}

pub async fn compare_refs(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, spec)): Path<(String, String)>,
    Query(params): Query<CompareParams>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let (base_ref, head_ref) = spec
        .split_once("...")
        .ok_or_else(|| ApiError::bad_request("Invalid compare spec, expected base...head"))?;

    let base_ref = urlencoding::decode(base_ref)
        .map_err(|_| ApiError::bad_request("Invalid base ref encoding"))?;
    let head_ref = urlencoding::decode(head_ref)
        .map_err(|_| ApiError::bad_request("Invalid head ref encoding"))?;

    let base_oid = resolve_ref(&git_repo, &base_ref)?;
    let head_oid = resolve_ref(&git_repo, &head_ref)?;

    let merge_base_oid = find_merge_base(&git_repo, base_oid, head_oid)?;
    let (ahead_by, behind_by) = count_ahead_behind(&git_repo, base_oid, head_oid)?;

    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE) as usize;

    let start_oid = if let Some(ref cursor) = params.cursor {
        Oid::from_str(cursor).map_err(|_| ApiError::bad_request("Invalid cursor"))?
    } else {
        head_oid
    };

    let mut revwalk = git_repo
        .revwalk()
        .map_err(|e| ApiError::internal(format!("Failed to create revwalk: {e}")))?;

    revwalk
        .push(start_oid)
        .map_err(|e| ApiError::internal(format!("Failed to start revwalk: {e}")))?;
    revwalk
        .hide(base_oid)
        .map_err(|e| ApiError::internal(format!("Failed to hide base: {e}")))?;

    if params.cursor.is_some() {
        revwalk.next();
    }

    let mut commits = Vec::new();
    for oid_result in revwalk.take(limit + 1) {
        let commit_oid =
            oid_result.map_err(|e| ApiError::internal(format!("Revwalk error: {e}")))?;
        let commit = get_commit(&git_repo, commit_oid)?;
        let stats = compute_commit_stats(&git_repo, &commit);
        commits.push(commit_to_response(&commit, stats));
    }

    let has_more = commits.len() > limit;
    let next_cursor = if has_more {
        commits.pop();
        commits.last().map(|c| c.sha.clone())
    } else {
        None
    };

    let base_commit = get_commit(&git_repo, base_oid)?;
    let head_commit = get_commit(&git_repo, head_oid)?;
    let base_tree = get_tree(&git_repo, &base_commit)?;
    let head_tree = get_tree(&git_repo, &head_commit)?;

    let (patch, stats) = build_diff(&git_repo, Some(&base_tree), &head_tree)?;

    Ok(Json(ApiResponse::success(CompareResponse {
        base_ref: base_ref.to_string(),
        head_ref: head_ref.to_string(),
        base_sha: base_oid.to_string(),
        head_sha: head_oid.to_string(),
        merge_base_sha: merge_base_oid.to_string(),
        ahead_by,
        behind_by,
        commits,
        next_cursor,
        has_more,
        diff: DiffResponse {
            base_sha: Some(base_oid.to_string()),
            head_sha: head_oid.to_string(),
            stats,
            patch,
        },
    })))
}

pub async fn get_tree_root(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name)): Path<(String, String)>,
    Query(params): Query<TreeParams>,
) -> Result<impl IntoResponse, ApiError> {
    get_tree_impl(auth, state, id, ref_name, String::new(), params).await
}

pub async fn get_tree_handler(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name, path)): Path<(String, String, String)>,
    Query(params): Query<TreeParams>,
) -> Result<impl IntoResponse, ApiError> {
    get_tree_impl(auth, state, id, ref_name, path, params).await
}

async fn get_tree_impl(
    auth: OptionalAuth,
    state: Arc<AppState>,
    id: String,
    ref_name: String,
    path: String,
    params: TreeParams,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let depth = params
        .depth
        .unwrap_or(DEFAULT_TREE_DEPTH)
        .clamp(1, MAX_TREE_DEPTH);

    let oid = resolve_ref(&git_repo, &ref_name)?;
    let commit = get_commit(&git_repo, oid)?;
    let root_tree = get_tree(&git_repo, &commit)?;

    let path = path.trim_matches('/');
    let tree = get_tree_at_path(&git_repo, &root_tree, path)?;

    let base_path = if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    };

    let mut entries = build_tree_entries(&git_repo, &tree, &base_path, depth);
    sort_tree_entries(&mut entries);

    Ok(Json(ApiResponse::success(entries)))
}

fn build_tree_entries(
    repo: &git2::Repository,
    tree: &git2::Tree<'_>,
    base_path: &str,
    depth: i32,
) -> Vec<TreeEntryResponse> {
    let mut entries = Vec::new();

    for entry in tree.iter() {
        let name = entry.name().unwrap_or("").to_string();
        let entry_path = if base_path.is_empty() {
            name.clone()
        } else {
            format!("{base_path}{name}")
        };

        let entry_type = entry_type_str(entry.kind(), entry.filemode());
        let mode = format!("{:06o}", entry.filemode());

        let mut resp = TreeEntryResponse {
            name,
            path: entry_path.clone(),
            entry_type: entry_type.to_string(),
            sha: entry.id().to_string(),
            mode,
            size: None,
            has_children: None,
            children: Vec::new(),
        };

        match entry.kind() {
            Some(ObjectType::Blob) => {
                if let Ok(blob) = repo.find_blob(entry.id()) {
                    resp.size = Some(blob.size() as i64);
                }
            }
            Some(ObjectType::Tree) if depth > 1 => {
                if let Ok(sub_tree) = repo.find_tree(entry.id()) {
                    resp.has_children = Some(!sub_tree.is_empty());
                    resp.children =
                        build_tree_entries(repo, &sub_tree, &format!("{entry_path}/"), depth - 1);
                }
            }
            Some(ObjectType::Tree) => {
                if let Ok(sub_tree) = repo.find_tree(entry.id()) {
                    resp.has_children = Some(!sub_tree.is_empty());
                }
            }
            _ => {}
        }

        entries.push(resp);
    }

    entries
}

fn sort_tree_entries(entries: &mut [TreeEntryResponse]) {
    entries.sort_by(
        |a, b| match (a.entry_type.as_str(), b.entry_type.as_str()) {
            ("dir", t) if t != "dir" => std::cmp::Ordering::Less,
            (t, "dir") if t != "dir" => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        },
    );

    for entry in entries.iter_mut() {
        if !entry.children.is_empty() {
            sort_tree_entries(&mut entry.children);
        }
    }
}

fn serve_raw_blob(blob: &git2::Blob<'_>, filename: &str) -> Result<Response, ApiError> {
    let size = blob.size() as i64;
    if size > MAX_RAW_BLOB_SIZE {
        return Err(ApiError::payload_too_large(format!(
            "File size ({} bytes) exceeds maximum allowed size ({} bytes)",
            size, MAX_RAW_BLOB_SIZE
        )));
    }

    let content_type = detect_content_type(filename, blob.content());
    let content = blob.content().to_vec();

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&blob.size().to_string()).unwrap(),
    );

    Ok((StatusCode::OK, headers, content).into_response())
}

fn detect_content_type(filename: &str, content: &[u8]) -> String {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "go" | "rs" | "py" | "rb" | "java" | "c" | "cpp" | "h" | "hpp" | "sh" | "sql" => {
            "text/plain; charset=utf-8".to_string()
        }
        "js" => "text/javascript; charset=utf-8".to_string(),
        "ts" => "text/typescript; charset=utf-8".to_string(),
        "md" => "text/markdown; charset=utf-8".to_string(),
        "json" => "application/json".to_string(),
        "yaml" | "yml" => "text/yaml; charset=utf-8".to_string(),
        "xml" => "application/xml".to_string(),
        "html" | "htm" => "text/html; charset=utf-8".to_string(),
        "css" => "text/css; charset=utf-8".to_string(),
        "txt" => "text/plain; charset=utf-8".to_string(),
        _ => {
            if is_binary(content) {
                "application/octet-stream".to_string()
            } else {
                "text/plain; charset=utf-8".to_string()
            }
        }
    }
}

pub async fn get_blame(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name, path)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Err(ApiError::bad_request("Path is required"));
    }

    let oid = resolve_ref(&git_repo, &ref_name)?;

    let blame = git_repo
        .blame_file(
            std::path::Path::new(path),
            Some(git2::BlameOptions::new().newest_commit(oid)),
        )
        .map_err(|_| GitError::PathNotFound(path.to_string()))?;

    let commit = get_commit(&git_repo, oid)?;
    let tree = get_tree(&git_repo, &commit)?;
    let blob = get_blob_at_path(&git_repo, &tree, path)?;

    let content = blob.content();
    let text = String::from_utf8_lossy(content);
    let text_lines: Vec<&str> = text.lines().collect();

    let mut lines = Vec::new();
    for (i, line_text) in text_lines.iter().enumerate() {
        let line_num = i + 1;

        if let Some(hunk) = blame.get_line(line_num) {
            lines.push(BlameLineResponse {
                line: line_num,
                sha: hunk.final_commit_id().to_string(),
                author: signature_to_response(&hunk.final_signature()),
                text: line_text.to_string(),
            });
        }
    }

    Ok(Json(ApiResponse::success(BlameResponse {
        path: path.to_string(),
        ref_name: ref_name.clone(),
        lines,
    })))
}

pub async fn get_archive(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name)): Path<(String, String)>,
    Query(params): Query<ArchiveParams>,
) -> Result<Response, ApiError> {
    let (repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let oid = resolve_ref(&git_repo, &ref_name)?;

    let format = params.format.as_deref().unwrap_or("zip");
    let (content_type, extension, git_format, use_gzip) = match format.to_lowercase().as_str() {
        "zip" => ("application/zip", "zip", "zip", false),
        "tar.gz" | "tgz" => ("application/gzip", "tar.gz", "tar", true),
        _ => return Err(ApiError::bad_request("Invalid archive format")),
    };

    if let Some(ref path) = params.path {
        if path.contains("..") {
            return Err(ApiError::bad_request("Invalid path"));
        }

        let path = path.trim_start_matches('/');
        let commit = get_commit(&git_repo, oid)?;
        let tree = get_tree(&git_repo, &commit)?;
        tree.get_path(std::path::Path::new(path))
            .map_err(|_| GitError::PathNotFound(path.to_string()))?;
    }

    let repo_path = repo_path(&state, &repo.namespace_id, &repo.name);
    let clean_ref = ref_name.replace('/', "-");
    let filename = format!("{}-{}.{}", repo.name, clean_ref, extension);

    let mut args = vec![
        "-C".to_string(),
        repo_path.to_string_lossy().to_string(),
        "archive".to_string(),
        format!("--format={git_format}"),
        oid.to_string(),
    ];

    if let Some(ref path) = params.path {
        args.push(path.trim_start_matches('/').to_string());
    }

    let output = tokio::time::timeout(
        ARCHIVE_TIMEOUT,
        Command::new("git")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| ApiError::internal("git archive timed out"))?
    .map_err(|e| ApiError::internal(format!("Failed to run git archive: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ApiError::internal(format!("git archive failed: {stderr}")));
    }

    let body = if use_gzip {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&output.stdout)
            .map_err(|e| ApiError::internal(format!("Failed to gzip: {e}")))?;
        encoder
            .finish()
            .map_err(|e| ApiError::internal(format!("Failed to finish gzip: {e}")))?
    } else {
        output.stdout
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));

    let safe_filename: String = filename
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    let safe_filename = if safe_filename.is_empty() {
        "archive".to_string()
    } else {
        safe_filename
    };

    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{safe_filename}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    } else {
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"archive\""),
        );
    }

    Ok((StatusCode::OK, headers, body).into_response())
}

const README_FILENAMES: &[&str] = &[
    "README.md",
    "readme.md",
    "README.MD",
    "Readme.md",
    "README",
    "readme",
    "README.txt",
    "readme.txt",
    "README.rst",
    "readme.rst",
];

pub async fn get_readme(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<ReadmeParams>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let ref_name = params.ref_name.as_deref().unwrap_or("");
    let oid = resolve_ref(&git_repo, ref_name)?;
    let commit = get_commit(&git_repo, oid)?;
    let tree = get_tree(&git_repo, &commit)?;

    let mut readme_file = None;
    let mut readme_filename = String::new();

    for name in README_FILENAMES {
        if let Ok(entry) = tree.get_path(std::path::Path::new(name)) {
            if entry.kind() == Some(ObjectType::Blob) {
                readme_filename = name.to_string();
                readme_file = Some(entry);
                break;
            }
        }
    }

    let entry = readme_file.ok_or_else(|| ApiError::not_found("No README found"))?;

    let blob = git_repo
        .find_blob(entry.id())
        .map_err(|e| ApiError::internal(format!("Failed to get blob: {e}")))?;

    let size = blob.size() as i64;
    let is_truncated = size > MAX_BLOB_SIZE;
    let read_size = size.min(MAX_BLOB_SIZE) as usize;

    let content = &blob.content()[..read_size];
    let is_bin = is_binary(content);

    let content_str = if is_bin {
        String::new()
    } else {
        String::from_utf8_lossy(content).to_string()
    };

    Ok(Json(ApiResponse::success(ReadmeResponse {
        filename: readme_filename,
        content: content_str,
        size,
        sha: blob.id().to_string(),
        is_binary: is_bin,
        is_truncated,
    })))
}

/// Helper to load repo and check write access for authenticated user
async fn load_repo_and_check_write_access(
    state: &Arc<AppState>,
    auth: &RequireUser,
    repo_id: &str,
) -> Result<(crate::types::Repo, git2::Repository), ApiError> {
    let repo = state
        .store
        .get_repo_by_id(repo_id)
        .api_err("Failed to get repository")?
        .or_not_found("Repository not found")?;

    require_repo_permission(
        state.store.as_ref(),
        &auth.user,
        &repo,
        Permission::REPO_WRITE,
    )?;

    let path = repo_path(state, &repo.namespace_id, &repo.name);
    let git_repo = open_repo(&path)?;

    Ok((repo, git_repo))
}

/// POST /repos/{id}/refs - Create a new reference
pub async fn create_ref_handler(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<CreateRefRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_write_access(&state, &auth, &id).await?;

    let oid = create_ref(
        &git_repo,
        &req.ref_type,
        &req.name,
        &req.target_sha,
        req.force,
    )?;

    let is_default = get_default_branch(&git_repo).as_deref() == Some(&req.name);

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(RefResponse {
            name: req.name,
            ref_type: req.ref_type,
            commit_sha: oid.to_string(),
            is_default,
        })),
    ))
}

#[derive(serde::Deserialize)]
pub struct RefPath {
    id: String,
    #[serde(rename = "type")]
    ref_type: String,
    name: String,
}

/// PATCH /repos/{id}/refs/{type}/{name} - Update a reference
pub async fn update_ref_handler(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(path): Path<RefPath>,
    Json(req): Json<UpdateRefRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_write_access(&state, &auth, &path.id).await?;

    let oid = update_ref(
        &git_repo,
        &path.ref_type,
        &path.name,
        &req.target_sha,
        req.expected_sha.as_deref(),
    )?;

    let is_default = get_default_branch(&git_repo).as_deref() == Some(&path.name);

    Ok(Json(ApiResponse::success(RefResponse {
        name: path.name,
        ref_type: path.ref_type,
        commit_sha: oid.to_string(),
        is_default,
    })))
}

/// DELETE /repos/{id}/refs/{type}/{name} - Delete a reference
pub async fn delete_ref_handler(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(path): Path<RefPath>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_write_access(&state, &auth, &path.id).await?;

    delete_ref(&git_repo, &path.ref_type, &path.name)?;

    Ok(StatusCode::NO_CONTENT)
}

/// PUT /repos/{id}/default-branch - Set the default branch
pub async fn set_default_branch_handler(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<SetDefaultBranchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_write_access(&state, &auth, &id).await?;

    set_default_branch(&git_repo, &req.branch)?;

    let branch_ref = format!("refs/heads/{}", req.branch);
    let reference = git_repo
        .find_reference(&branch_ref)
        .map_err(|e| ApiError::internal(format!("Failed to find branch: {e}")))?;

    let commit_sha = reference
        .target()
        .map(|oid| oid.to_string())
        .unwrap_or_default();

    Ok(Json(ApiResponse::success(RefResponse {
        name: req.branch,
        ref_type: "branch".to_string(),
        commit_sha,
        is_default: true,
    })))
}

// ============================================================================
// Content Mutation Handlers
// ============================================================================

fn decode_content(content: &str, encoding: Option<&str>) -> Result<Vec<u8>, ApiError> {
    match encoding {
        Some("base64") => STANDARD
            .decode(content)
            .map_err(|e| ApiError::bad_request(format!("Invalid base64 content: {e}"))),
        Some("utf-8") | None => Ok(content.as_bytes().to_vec()),
        Some(enc) => Err(ApiError::bad_request(format!(
            "Unsupported encoding: {enc}"
        ))),
    }
}

fn get_commit_author(state: &AppState, user: &crate::types::User) -> (String, String) {
    let name = state
        .store
        .get_namespace(&user.primary_namespace_id)
        .ok()
        .flatten()
        .map(|ns| ns.name)
        .unwrap_or_else(|| "Unknown".to_string());
    let email = format!("{}@noreply.cutman", name);
    (name, email)
}

fn resolve_branch(git_repo: &git2::Repository, ref_name: &str) -> Result<String, GitError> {
    if ref_name.is_empty() {
        get_default_branch(git_repo).ok_or(GitError::EmptyRepo)
    } else {
        Ok(ref_name.to_string())
    }
}

fn check_create_or_update(
    tree: &git2::Tree<'_>,
    path: &str,
    sha: Option<&str>,
) -> Result<(), ApiError> {
    if let Some(expected_sha) = sha {
        verify_blob_sha(tree, path, expected_sha)?;
    } else if file_exists(tree, path) {
        return Err(ApiError::conflict(format!(
            "File already exists: {path}. Provide 'sha' to update."
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_blob_change(
    git_repo: &git2::Repository,
    tree: &git2::Tree<'_>,
    branch: &str,
    path: &str,
    content: &[u8],
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<(Oid, FileInfo), ApiError> {
    let new_tree_oid = tree_with_blob(git_repo, Some(tree), path, content)?;

    let commit_oid = create_commit_on_branch(
        git_repo,
        branch,
        new_tree_oid,
        message,
        author_name,
        author_email,
    )?;

    let new_tree = git_repo
        .find_tree(new_tree_oid)
        .map_err(|e| ApiError::internal(format!("Failed to find new tree: {e}")))?;
    let new_blob = get_blob_at_path(git_repo, &new_tree, path)?;

    let file_info = FileInfo {
        path: path.to_string(),
        sha: new_blob.id().to_string(),
        size: new_blob.size() as i64,
    };

    Ok((commit_oid, file_info))
}

const MAX_UPLOAD_SIZE: usize = 100 * 1024 * 1024;

async fn parse_multipart_upload(
    multipart: &mut axum::extract::Multipart,
    path: &str,
) -> Result<(Vec<u8>, String, Option<String>), ApiError> {
    let mut content: Option<Vec<u8>> = None;
    let mut message: Option<String> = None;
    let mut sha: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("Failed to read multipart: {e}")))?
    {
        match field.name() {
            Some("file") => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::bad_request(format!("Failed to read file: {e}")))?;
                if data.len() > MAX_UPLOAD_SIZE {
                    return Err(ApiError::payload_too_large(format!(
                        "File size ({} bytes) exceeds maximum allowed size ({MAX_UPLOAD_SIZE} bytes)",
                        data.len()
                    )));
                }
                content = Some(data.to_vec());
            }
            Some("message") => {
                message =
                    Some(field.text().await.map_err(|e| {
                        ApiError::bad_request(format!("Failed to read message: {e}"))
                    })?);
            }
            Some("sha") => {
                sha = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::bad_request(format!("Failed to read sha: {e}")))?,
                );
            }
            _ => {}
        }
    }

    let content = content.ok_or_else(|| ApiError::bad_request("File field is required"))?;
    let message = message.unwrap_or_else(|| format!("Upload {path}"));

    Ok((content, message, sha))
}

/// PUT /repos/{id}/blob/{ref}/{*path} - Create or update a file
pub async fn put_blob(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name, path)): Path<(String, String, String)>,
    Json(req): Json<PutBlobRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_write_access(&state, &auth, &id).await?;

    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Err(ApiError::bad_request("Path is required"));
    }

    let branch = resolve_branch(&git_repo, &ref_name)?;
    let content = decode_content(&req.content, req.encoding.as_deref())?;

    let oid = resolve_ref(&git_repo, &branch)?;
    let commit = get_commit(&git_repo, oid)?;
    let tree = get_tree(&git_repo, &commit)?;

    check_create_or_update(&tree, path, req.sha.as_deref())?;

    let (author_name, author_email) = get_commit_author(&state, &auth.user);
    let (commit_oid, file_info) = commit_blob_change(
        &git_repo,
        &tree,
        &branch,
        path,
        &content,
        &req.message,
        &author_name,
        &author_email,
    )?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(MutationResponse {
            commit_sha: commit_oid.to_string(),
            ref_name: branch,
            file: Some(file_info),
        })),
    ))
}

/// DELETE /repos/{id}/blob/{ref}/{*path} - Delete a file
pub async fn delete_blob(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name, path)): Path<(String, String, String)>,
    Json(req): Json<DeleteBlobRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_write_access(&state, &auth, &id).await?;

    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Err(ApiError::bad_request("Path is required"));
    }

    let branch = resolve_branch(&git_repo, &ref_name)?;

    let oid = resolve_ref(&git_repo, &branch)?;
    let commit = get_commit(&git_repo, oid)?;
    let tree = get_tree(&git_repo, &commit)?;

    verify_blob_sha(&tree, path, &req.sha)?;
    let new_tree_oid = tree_without_entry(&git_repo, &tree, path)?;

    let (author_name, author_email) = get_commit_author(&state, &auth.user);
    let commit_oid = create_commit_on_branch(
        &git_repo,
        &branch,
        new_tree_oid,
        &req.message,
        &author_name,
        &author_email,
    )?;

    Ok(Json(ApiResponse::success(MutationResponse {
        commit_sha: commit_oid.to_string(),
        ref_name: branch,
        file: None,
    })))
}

/// POST /repos/{id}/commits - Multi-file atomic commit
pub async fn create_multi_commit(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<MultiCommitRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_write_access(&state, &auth, &id).await?;

    let branch = resolve_branch(&git_repo, req.branch.as_deref().unwrap_or(""))?;

    if req.actions.is_empty() {
        return Err(ApiError::bad_request("At least one action is required"));
    }

    let actions: Vec<CommitActionOp> = req
        .actions
        .iter()
        .map(action_to_op)
        .collect::<Result<_, _>>()?;

    let (author_name, author_email) = get_commit_author(&state, &auth.user);
    let commit_oid = apply_actions(
        &git_repo,
        &branch,
        &actions,
        &req.message,
        &author_name,
        &author_email,
    )?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(MutationResponse {
            commit_sha: commit_oid.to_string(),
            ref_name: branch,
            file: None,
        })),
    ))
}

fn action_to_op(action: &CommitAction) -> Result<CommitActionOp, ApiError> {
    match action {
        CommitAction::Create {
            path,
            content,
            encoding,
        } => {
            let data = decode_content(content, encoding.as_deref())?;
            Ok(CommitActionOp::Create {
                path: path.clone(),
                content: data,
            })
        }
        CommitAction::Update {
            path,
            content,
            encoding,
            sha,
        } => {
            let data = decode_content(content, encoding.as_deref())?;
            Ok(CommitActionOp::Update {
                path: path.clone(),
                content: data,
                sha: sha.clone(),
            })
        }
        CommitAction::Delete { path, sha } => Ok(CommitActionOp::Delete {
            path: path.clone(),
            sha: sha.clone(),
        }),
        CommitAction::Move { from, to, sha } => Ok(CommitActionOp::Move {
            from: from.clone(),
            to: to.clone(),
            sha: sha.clone(),
        }),
    }
}

/// POST /repos/{id}/upload/{ref}/{*path} - Binary file upload
pub async fn upload_blob(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name, path)): Path<(String, String, String)>,
    mut multipart: axum::extract::Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_write_access(&state, &auth, &id).await?;

    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Err(ApiError::bad_request("Path is required"));
    }

    let branch = resolve_branch(&git_repo, &ref_name)?;

    let (content, message, sha) = parse_multipart_upload(&mut multipart, path).await?;

    let oid = resolve_ref(&git_repo, &branch)?;
    let commit = get_commit(&git_repo, oid)?;
    let tree = get_tree(&git_repo, &commit)?;

    check_create_or_update(&tree, path, sha.as_deref())?;

    let (author_name, author_email) = get_commit_author(&state, &auth.user);
    let (commit_oid, file_info) = commit_blob_change(
        &git_repo,
        &tree,
        &branch,
        path,
        &content,
        &message,
        &author_name,
        &author_email,
    )?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(MutationResponse {
            commit_sha: commit_oid.to_string(),
            ref_name: branch,
            file: Some(file_info),
        })),
    ))
}

/// GET /repos/{id}/blob/{ref}/{*path} - Enhanced blob with history and parsed frontmatter
pub async fn get_blob_enhanced(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name, path)): Path<(String, String, String)>,
    Query(params): Query<EnhancedBlobParams>,
) -> Result<Response, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Err(ApiError::bad_request("Path is required"));
    }

    let ref_to_use = params.at.as_deref().unwrap_or(&ref_name);
    let oid = resolve_ref(&git_repo, ref_to_use)?;
    let commit = get_commit(&git_repo, oid)?;
    let tree = get_tree(&git_repo, &commit)?;
    let blob = get_blob_at_path(&git_repo, &tree, path)?;

    if params.raw.unwrap_or(false) {
        return serve_raw_blob(&blob, path);
    }

    let size = blob.size() as i64;
    let is_truncated = size > MAX_BLOB_SIZE;
    let read_size = size.min(MAX_BLOB_SIZE) as usize;

    let content_bytes = &blob.content()[..read_size];
    let binary_content = is_binary(content_bytes);

    let (encoded_content, encoding) = if binary_content {
        (STANDARD.encode(content_bytes), "base64".to_string())
    } else {
        (
            String::from_utf8_lossy(content_bytes).to_string(),
            "utf-8".to_string(),
        )
    };

    let mut frontmatter = None;
    let mut body = None;

    if params.parsed.unwrap_or(false) && !binary_content {
        if let Some((fm, bd)) = parse_frontmatter(&encoded_content) {
            frontmatter = Some(fm);
            body = Some(bd);
        }
    }

    let (history, history_cursor, history_has_more) = if params.history.unwrap_or(false) {
        let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE) as usize;
        let (commits, cursor, has_more) =
            get_file_history(&git_repo, oid, path, limit, params.cursor.as_deref())?;
        (Some(commits), cursor, Some(has_more))
    } else {
        (None, None, None)
    };

    Ok(Json(ApiResponse::success(EnhancedBlobResponse {
        sha: blob.id().to_string(),
        size,
        content: Some(encoded_content),
        encoding,
        is_binary: binary_content,
        is_truncated,
        frontmatter,
        body,
        history,
        history_cursor,
        history_has_more,
    }))
    .into_response())
}

fn parse_frontmatter(content: &str) -> Option<(serde_json::Value, String)> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }

    let after_first_delim = &content[3..];
    let end_delim_pos = after_first_delim.find("\n---")?;

    let yaml_content = after_first_delim[..end_delim_pos].trim();
    let body_start = 3 + end_delim_pos + 4;
    let body = content
        .get(body_start..)?
        .trim_start_matches('\n')
        .to_string();

    let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml_content).ok()?;
    let json_value = yaml_to_json(yaml_value);

    Some((json_value, body))
}

fn yaml_to_json(yaml: serde_yaml::Value) -> serde_json::Value {
    match yaml {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        serde_yaml::Value::String(s) => s,
                        other => serde_yaml::to_string(&other).ok()?.trim().to_string(),
                    };
                    Some((key, yaml_to_json(v)))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
}

/// GET /repos/{id}/search - Search for paths matching a glob pattern
pub async fn search_paths_handler(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<PathSearchParams>,
) -> Result<impl IntoResponse, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let ref_name = params.ref_name.as_deref().unwrap_or("");
    let oid = resolve_ref(&git_repo, ref_name)?;
    let commit = get_commit(&git_repo, oid)?;
    let tree = get_tree(&git_repo, &commit)?;

    let limit = params.limit.unwrap_or(100).min(1000) as usize;
    let matches = search_paths(&git_repo, &tree, &params.q, limit)?;

    Ok(Json(ApiResponse::success(PathSearchResponse { matches })))
}
