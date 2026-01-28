use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::write::GzEncoder;
use flate2::Compression;
use git2::{ObjectType, Oid};

use crate::server::AppState;
use crate::server::response::{ApiError, ApiResponse, PaginatedResponse, StoreOptionExt, StoreResultExt};

use super::auth::{OptionalAuth, check_content_access};
use super::dto::{
    ArchiveParams, BlameLineResponse, BlameResponse, BlobParams, BlobResponse, CompareParams,
    CompareResponse, DiffResponse, ListCommitsParams, ReadmeParams, ReadmeResponse, RefResponse,
    TreeEntryResponse, TreeParams, DEFAULT_PAGE_SIZE, DEFAULT_TREE_DEPTH, MAX_BLOB_SIZE,
    MAX_PAGE_SIZE, MAX_TREE_DEPTH,
};
use super::git_ops::{
    build_diff, commit_to_response, compute_commit_stats, count_ahead_behind, entry_type_str,
    find_merge_base, get_blob_at_path, get_commit, get_default_branch, get_tree, get_tree_at_path,
    is_binary, open_repo, resolve_ref, signature_to_response, GitError,
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

    refs.sort_by(|a, b| {
        match (a.is_default, b.is_default) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => match (a.ref_type.as_str(), b.ref_type.as_str()) {
                ("branch", "tag") => std::cmp::Ordering::Less,
                ("tag", "branch") => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            },
        }
    });

    Ok(Json(refs))
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

    let limit = params
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .min(MAX_PAGE_SIZE) as usize;

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

        let commit_oid = oid_result.map_err(|e| ApiError::internal(format!("Revwalk error: {e}")))?;
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

    Ok(Json(ApiResponse::success(commit_to_response(&commit, stats))))
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

    let limit = params
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .min(MAX_PAGE_SIZE) as usize;

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
        let commit_oid = oid_result.map_err(|e| ApiError::internal(format!("Revwalk error: {e}")))?;
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

    Ok(Json(entries))
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
    entries.sort_by(|a, b| {
        match (a.entry_type.as_str(), b.entry_type.as_str()) {
            ("dir", t) if t != "dir" => std::cmp::Ordering::Less,
            (t, "dir") if t != "dir" => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    for entry in entries.iter_mut() {
        if !entry.children.is_empty() {
            sort_tree_entries(&mut entry.children);
        }
    }
}

pub async fn get_blob(
    auth: OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path((id, ref_name, path)): Path<(String, String, String)>,
    Query(params): Query<BlobParams>,
) -> Result<Response, ApiError> {
    let (_repo, git_repo) = load_repo_and_check_access(&state, &auth, &id).await?;

    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Err(ApiError::bad_request("Path is required"));
    }

    let oid = resolve_ref(&git_repo, &ref_name)?;
    let commit = get_commit(&git_repo, oid)?;
    let tree = get_tree(&git_repo, &commit)?;
    let blob = get_blob_at_path(&git_repo, &tree, path)?;

    if params.raw.unwrap_or(false) {
        return serve_raw_blob(&blob, path);
    }

    let size = blob.size() as i64;
    let is_truncated = size > MAX_BLOB_SIZE;
    let read_size = size.min(MAX_BLOB_SIZE) as usize;

    let content = &blob.content()[..read_size];
    let is_bin = is_binary(content);

    let (encoded_content, encoding) = if is_bin {
        (STANDARD.encode(content), "base64".to_string())
    } else {
        (
            String::from_utf8_lossy(content).to_string(),
            "utf-8".to_string(),
        )
    };

    Ok(Json(ApiResponse::success(BlobResponse {
        sha: blob.id().to_string(),
        size,
        content: Some(encoded_content),
        encoding,
        is_binary: is_bin,
        is_truncated,
    }))
    .into_response())
}

fn serve_raw_blob(blob: &git2::Blob<'_>, filename: &str) -> Result<Response, ApiError> {
    let content_type = detect_content_type(filename, blob.content());
    let content = blob.content().to_vec();

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")),
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
        .blame_file(std::path::Path::new(path), Some(git2::BlameOptions::new().newest_commit(oid)))
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

    let output = Command::new("git")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
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
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).unwrap(),
    );

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
