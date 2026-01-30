use std::path::Path;

use chrono::{TimeZone, Utc};
use git2::{Commit, DiffOptions, ObjectType, Oid, Repository, Signature, Tree};

use crate::server::response::ApiError;

use super::dto::{CommitResponse, CommitStats, SignatureResponse};

#[derive(Debug)]
pub enum GitError {
    RepoNotFound,
    RefNotFound(String),
    PathNotFound(String),
    EmptyRepo,
    NotAFile,
    NotADirectory,
    Conflict(String),
    Internal(String),
}

impl From<GitError> for ApiError {
    fn from(err: GitError) -> Self {
        match err {
            GitError::RepoNotFound => ApiError::not_found("Repository not initialized"),
            GitError::RefNotFound(r) => ApiError::not_found(format!("Reference not found: {r}")),
            GitError::PathNotFound(p) => ApiError::not_found(format!("Path not found: {p}")),
            GitError::EmptyRepo => ApiError::not_found("Repository is empty"),
            GitError::NotAFile => ApiError::bad_request("Path is a directory, not a file"),
            GitError::NotADirectory => ApiError::bad_request("Path is a file, not a directory"),
            GitError::Conflict(msg) => ApiError::conflict(msg),
            GitError::Internal(msg) => ApiError::internal(msg),
        }
    }
}

pub fn open_repo(path: &Path) -> Result<Repository, GitError> {
    Repository::open_bare(path).map_err(|_| GitError::RepoNotFound)
}

pub fn open_or_init_repo(path: &Path) -> Result<Repository, GitError> {
    match Repository::open_bare(path) {
        Ok(repo) => Ok(repo),
        Err(_) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| GitError::Internal(format!("Failed to create repo directory: {e}")))?;
            }
            let repo = Repository::init_bare(path)
                .map_err(|e| GitError::Internal(format!("Failed to init repo: {e}")))?;
            repo.set_head("refs/heads/main")
                .map_err(|e| GitError::Internal(format!("Failed to set HEAD: {e}")))?;
            Ok(repo)
        }
    }
}

pub fn resolve_ref(repo: &Repository, ref_spec: &str) -> Result<Oid, GitError> {
    let ref_spec = if ref_spec.is_empty() {
        "HEAD"
    } else {
        ref_spec
    };

    if ref_spec.len() == 40 {
        if let Ok(oid) = Oid::from_str(ref_spec) {
            if repo.find_commit(oid).is_ok() {
                return Ok(oid);
            }
        }
    }

    let branch_ref = format!("refs/heads/{ref_spec}");
    if let Ok(reference) = repo.find_reference(&branch_ref) {
        if let Some(oid) = reference.target() {
            return Ok(oid);
        }
    }

    let tag_ref = format!("refs/tags/{ref_spec}");
    if let Ok(reference) = repo.find_reference(&tag_ref) {
        if let Some(oid) = reference.target() {
            if let Ok(tag) = repo.find_tag(oid) {
                return Ok(tag.target_id());
            }
            return Ok(oid);
        }
    }

    if ref_spec == "HEAD" {
        let head = repo.head().map_err(|_| GitError::EmptyRepo)?;
        return head.target().ok_or(GitError::EmptyRepo);
    }

    Err(GitError::RefNotFound(ref_spec.to_string()))
}

pub fn get_commit<'a>(repo: &'a Repository, oid: Oid) -> Result<Commit<'a>, GitError> {
    repo.find_commit(oid)
        .map_err(|e| GitError::Internal(format!("Failed to get commit: {e}")))
}

pub fn get_tree<'a>(repo: &'a Repository, commit: &Commit<'_>) -> Result<Tree<'a>, GitError> {
    let tree_oid = commit.tree_id();
    repo.find_tree(tree_oid)
        .map_err(|e| GitError::Internal(format!("Failed to get tree: {e}")))
}

pub fn get_tree_at_path<'a>(
    repo: &'a Repository,
    tree: &Tree<'_>,
    path: &str,
) -> Result<Tree<'a>, GitError> {
    if path.is_empty() {
        let oid = tree.id();
        return repo
            .find_tree(oid)
            .map_err(|e| GitError::Internal(format!("Failed to clone tree: {e}")));
    }

    let entry = tree
        .get_path(Path::new(path))
        .map_err(|_| GitError::PathNotFound(path.to_string()))?;

    if entry.kind() != Some(ObjectType::Tree) {
        return Err(GitError::NotADirectory);
    }

    let obj = entry
        .to_object(repo)
        .map_err(|e| GitError::Internal(format!("Failed to get tree object: {e}")))?;

    obj.into_tree().map_err(|_| GitError::NotADirectory)
}

pub fn get_blob_at_path<'a>(
    repo: &'a Repository,
    tree: &Tree<'_>,
    path: &str,
) -> Result<git2::Blob<'a>, GitError> {
    let entry = tree
        .get_path(Path::new(path))
        .map_err(|_| GitError::PathNotFound(path.to_string()))?;

    if entry.kind() == Some(ObjectType::Tree) {
        return Err(GitError::NotAFile);
    }

    let obj = entry
        .to_object(repo)
        .map_err(|e| GitError::Internal(format!("Failed to get blob object: {e}")))?;

    obj.into_blob()
        .map_err(|_| GitError::Internal("Object is not a blob".to_string()))
}

#[must_use]
pub fn is_binary(content: &[u8]) -> bool {
    let sample_size = content.len().min(8192);
    content[..sample_size].contains(&0)
}

#[must_use]
pub fn signature_to_response(sig: &Signature<'_>) -> SignatureResponse {
    let timestamp = sig.when();
    let secs = timestamp.seconds();
    let date = Utc.timestamp_opt(secs, 0).single().unwrap_or_else(Utc::now);

    SignatureResponse {
        name: sig.name().unwrap_or("").to_string(),
        email: sig.email().unwrap_or("").to_string(),
        date,
    }
}

#[must_use]
pub fn commit_to_response(commit: &Commit<'_>, stats: Option<CommitStats>) -> CommitResponse {
    let parent_shas: Vec<String> = commit.parent_ids().map(|id| id.to_string()).collect();

    CommitResponse {
        sha: commit.id().to_string(),
        message: commit.message().unwrap_or("").to_string(),
        author: signature_to_response(&commit.author()),
        committer: signature_to_response(&commit.committer()),
        parent_shas,
        tree_sha: commit.tree_id().to_string(),
        stats,
    }
}

#[must_use]
pub fn compute_commit_stats(repo: &Repository, commit: &Commit<'_>) -> Option<CommitStats> {
    let current_tree = commit.tree().ok()?;

    let parent_tree = if commit.parent_count() > 0 {
        commit.parent(0).ok()?.tree().ok()
    } else {
        None
    };

    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&current_tree), None)
        .ok()?;

    let diff_stats = diff.stats().ok()?;

    Some(CommitStats {
        files_changed: diff_stats.files_changed(),
        additions: diff_stats.insertions(),
        deletions: diff_stats.deletions(),
    })
}

pub fn build_diff(
    repo: &Repository,
    base_tree: Option<&Tree<'_>>,
    head_tree: &Tree<'_>,
) -> Result<(String, CommitStats), GitError> {
    let mut opts = DiffOptions::new();
    opts.context_lines(3);

    let diff = repo
        .diff_tree_to_tree(base_tree, Some(head_tree), Some(&mut opts))
        .map_err(|e| GitError::Internal(format!("Failed to compute diff: {e}")))?;

    let stats = diff
        .stats()
        .map_err(|e| GitError::Internal(format!("Failed to get diff stats: {e}")))?;

    let mut patch = Vec::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        if origin == '+' || origin == '-' || origin == ' ' {
            patch.push(origin as u8);
        }
        patch.extend_from_slice(line.content());
        true
    })
    .map_err(|e| GitError::Internal(format!("Failed to format diff: {e}")))?;

    let patch_str = String::from_utf8_lossy(&patch).to_string();

    Ok((
        patch_str,
        CommitStats {
            files_changed: stats.files_changed(),
            additions: stats.insertions(),
            deletions: stats.deletions(),
        },
    ))
}

pub fn find_merge_base(repo: &Repository, base: Oid, head: Oid) -> Result<Oid, GitError> {
    repo.merge_base(base, head)
        .map_err(|e| GitError::Internal(format!("Failed to find merge base: {e}")))
}

pub fn count_ahead_behind(
    repo: &Repository,
    base: Oid,
    head: Oid,
) -> Result<(usize, usize), GitError> {
    repo.graph_ahead_behind(head, base)
        .map_err(|e| GitError::Internal(format!("Failed to count ahead/behind: {e}")))
}

#[must_use]
pub fn get_default_branch(repo: &Repository) -> Option<String> {
    repo.head().ok()?.shorthand().map(String::from)
}

#[must_use]
pub fn entry_type_str(kind: Option<ObjectType>, filemode: i32) -> &'static str {
    match kind {
        Some(ObjectType::Tree) => "dir",
        Some(ObjectType::Blob) => {
            if filemode == 0o120000 {
                "symlink"
            } else {
                "file"
            }
        }
        Some(ObjectType::Commit) => "submodule",
        _ => "file",
    }
}

/// Create a new reference (branch or tag)
pub fn create_ref(
    repo: &Repository,
    ref_type: &str,
    name: &str,
    target_sha: &str,
    force: bool,
) -> Result<Oid, GitError> {
    let oid = Oid::from_str(target_sha)
        .map_err(|_| GitError::Internal(format!("Invalid SHA: {target_sha}")))?;

    repo.find_commit(oid)
        .map_err(|_| GitError::RefNotFound(format!("Target commit not found: {target_sha}")))?;

    let full_ref = match ref_type {
        "branch" => format!("refs/heads/{name}"),
        "tag" => format!("refs/tags/{name}"),
        _ => {
            return Err(GitError::Internal(format!(
                "Invalid ref type: {ref_type}. Must be 'branch' or 'tag'"
            )));
        }
    };

    if !force && repo.find_reference(&full_ref).is_ok() {
        return Err(GitError::Internal(format!(
            "Reference already exists: {name}"
        )));
    }

    repo.reference(
        &full_ref,
        oid,
        force,
        &format!("Creating {ref_type} {name}"),
    )
    .map_err(|e| GitError::Internal(format!("Failed to create reference: {e}")))?;

    Ok(oid)
}

/// Update an existing reference
pub fn update_ref(
    repo: &Repository,
    ref_type: &str,
    name: &str,
    target_sha: &str,
    expected_sha: Option<&str>,
) -> Result<Oid, GitError> {
    let full_ref = match ref_type {
        "branch" => format!("refs/heads/{name}"),
        "tag" => format!("refs/tags/{name}"),
        _ => {
            return Err(GitError::Internal(format!(
                "Invalid ref type: {ref_type}. Must be 'branch' or 'tag'"
            )));
        }
    };

    let current_ref = repo
        .find_reference(&full_ref)
        .map_err(|_| GitError::RefNotFound(name.to_string()))?;

    if let Some(expected) = expected_sha {
        let expected_oid = Oid::from_str(expected)
            .map_err(|_| GitError::Internal(format!("Invalid expected SHA: {expected}")))?;

        let current_oid = current_ref.target().ok_or_else(|| {
            GitError::Internal("Reference has no target (symbolic reference)".to_string())
        })?;

        if current_oid != expected_oid {
            return Err(GitError::Internal(format!(
                "Reference has been updated. Expected {expected}, found {current_oid}"
            )));
        }
    }

    let new_oid = Oid::from_str(target_sha)
        .map_err(|_| GitError::Internal(format!("Invalid SHA: {target_sha}")))?;

    repo.find_commit(new_oid)
        .map_err(|_| GitError::RefNotFound(format!("Target commit not found: {target_sha}")))?;

    repo.reference(
        &full_ref,
        new_oid,
        true,
        &format!("Updating {ref_type} {name}"),
    )
    .map_err(|e| GitError::Internal(format!("Failed to update reference: {e}")))?;

    Ok(new_oid)
}

/// Delete a reference
pub fn delete_ref(repo: &Repository, ref_type: &str, name: &str) -> Result<(), GitError> {
    let full_ref = match ref_type {
        "branch" => format!("refs/heads/{name}"),
        "tag" => format!("refs/tags/{name}"),
        _ => {
            return Err(GitError::Internal(format!(
                "Invalid ref type: {ref_type}. Must be 'branch' or 'tag'"
            )));
        }
    };

    let mut reference = repo
        .find_reference(&full_ref)
        .map_err(|_| GitError::RefNotFound(name.to_string()))?;

    reference
        .delete()
        .map_err(|e| GitError::Internal(format!("Failed to delete reference: {e}")))?;

    Ok(())
}

/// Set the default branch (HEAD)
pub fn set_default_branch(repo: &Repository, branch: &str) -> Result<(), GitError> {
    let full_ref = format!("refs/heads/{branch}");

    repo.find_reference(&full_ref)
        .map_err(|_| GitError::RefNotFound(branch.to_string()))?;

    repo.set_head(&full_ref)
        .map_err(|e| GitError::Internal(format!("Failed to set HEAD: {e}")))?;

    Ok(())
}

/// Verify that the blob at the given path has the expected SHA.
/// Returns Ok(()) if they match, Conflict error if not.
pub fn verify_blob_sha(tree: &Tree<'_>, path: &str, expected_sha: &str) -> Result<(), GitError> {
    let entry = tree
        .get_path(Path::new(path))
        .map_err(|_| GitError::PathNotFound(path.to_string()))?;

    let expected_oid = Oid::from_str(expected_sha)
        .map_err(|_| GitError::Internal(format!("Invalid SHA: {expected_sha}")))?;

    if entry.id() != expected_oid {
        return Err(GitError::Conflict(format!(
            "File has been modified. Expected {expected_sha}, found {}",
            entry.id()
        )));
    }

    Ok(())
}

/// Check if a file exists at the given path in the tree.
#[must_use]
pub fn file_exists(tree: &Tree<'_>, path: &str) -> bool {
    tree.get_path(Path::new(path)).is_ok()
}

/// Build a new tree with a blob added or updated at the given path.
/// Handles nested paths by creating intermediate tree entries as needed.
pub fn tree_with_blob(
    repo: &Repository,
    base_tree: Option<&Tree<'_>>,
    path: &str,
    content: &[u8],
) -> Result<Oid, GitError> {
    let blob_oid = repo
        .blob(content)
        .map_err(|e| GitError::Internal(format!("Failed to create blob: {e}")))?;

    let parts: Vec<&str> = path.split('/').collect();
    build_tree_recursive(repo, base_tree, &parts, blob_oid, 0o100644)
}

fn build_tree_recursive(
    repo: &Repository,
    base_tree: Option<&Tree<'_>>,
    parts: &[&str],
    blob_oid: Oid,
    filemode: i32,
) -> Result<Oid, GitError> {
    let mut builder = repo
        .treebuilder(base_tree)
        .map_err(|e| GitError::Internal(format!("Failed to create tree builder: {e}")))?;

    if parts.len() == 1 {
        builder
            .insert(parts[0], blob_oid, filemode)
            .map_err(|e| GitError::Internal(format!("Failed to insert blob: {e}")))?;
    } else {
        let subdir = parts[0];
        let rest = &parts[1..];

        let existing_subtree = base_tree.and_then(|t| {
            t.get_name(subdir).and_then(|entry| {
                if entry.kind() == Some(ObjectType::Tree) {
                    repo.find_tree(entry.id()).ok()
                } else {
                    None
                }
            })
        });

        let new_subtree_oid =
            build_tree_recursive(repo, existing_subtree.as_ref(), rest, blob_oid, filemode)?;

        builder
            .insert(subdir, new_subtree_oid, 0o040000)
            .map_err(|e| GitError::Internal(format!("Failed to insert subtree: {e}")))?;
    }

    builder
        .write()
        .map_err(|e| GitError::Internal(format!("Failed to write tree: {e}")))
}

/// Build a new tree with a file removed at the given path.
/// Cleans up empty parent directories.
pub fn tree_without_entry(
    repo: &Repository,
    base_tree: &Tree<'_>,
    path: &str,
) -> Result<Oid, GitError> {
    let parts: Vec<&str> = path.split('/').collect();
    remove_from_tree_recursive(repo, base_tree, &parts)
}

fn remove_from_tree_recursive(
    repo: &Repository,
    tree: &Tree<'_>,
    parts: &[&str],
) -> Result<Oid, GitError> {
    let mut builder = repo
        .treebuilder(Some(tree))
        .map_err(|e| GitError::Internal(format!("Failed to create tree builder: {e}")))?;

    if parts.len() == 1 {
        builder
            .remove(parts[0])
            .map_err(|e| GitError::Internal(format!("Failed to remove entry: {e}")))?;
    } else {
        let subdir = parts[0];
        let rest = &parts[1..];

        let entry = tree
            .get_name(subdir)
            .ok_or_else(|| GitError::PathNotFound(parts.join("/")))?;

        if entry.kind() != Some(ObjectType::Tree) {
            return Err(GitError::NotADirectory);
        }

        let subtree = repo
            .find_tree(entry.id())
            .map_err(|e| GitError::Internal(format!("Failed to find subtree: {e}")))?;

        let new_subtree_oid = remove_from_tree_recursive(repo, &subtree, rest)?;

        let new_subtree = repo
            .find_tree(new_subtree_oid)
            .map_err(|e| GitError::Internal(format!("Failed to find new subtree: {e}")))?;

        if new_subtree.is_empty() {
            builder
                .remove(subdir)
                .map_err(|e| GitError::Internal(format!("Failed to remove empty dir: {e}")))?;
        } else {
            builder
                .insert(subdir, new_subtree_oid, 0o040000)
                .map_err(|e| {
                    GitError::Internal(format!("Failed to insert updated subtree: {e}"))
                })?;
        }
    }

    builder
        .write()
        .map_err(|e| GitError::Internal(format!("Failed to write tree: {e}")))
}

/// Create a commit on a branch with the given tree and message.
/// Returns (commit_sha, branch_name).
pub fn create_commit_on_branch(
    repo: &Repository,
    branch: &str,
    tree_oid: Oid,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<Oid, GitError> {
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| GitError::Internal(format!("Failed to find tree: {e}")))?;

    let sig = Signature::now(author_name, author_email)
        .map_err(|e| GitError::Internal(format!("Failed to create signature: {e}")))?;

    let branch_ref = format!("refs/heads/{branch}");

    let parent_commit = repo
        .find_reference(&branch_ref)
        .ok()
        .and_then(|r| r.target())
        .and_then(|oid| repo.find_commit(oid).ok());

    let parents: Vec<&Commit<'_>> = parent_commit.iter().collect();

    let commit_oid = repo
        .commit(Some(&branch_ref), &sig, &sig, message, &tree, &parents)
        .map_err(|e| GitError::Internal(format!("Failed to create commit: {e}")))?;

    Ok(commit_oid)
}

/// Action to apply in a multi-file commit
#[derive(Debug)]
pub enum CommitActionOp {
    Create {
        path: String,
        content: Vec<u8>,
    },
    Update {
        path: String,
        content: Vec<u8>,
        sha: Option<String>,
    },
    Delete {
        path: String,
        sha: String,
    },
    Move {
        from: String,
        to: String,
        sha: Option<String>,
    },
}

/// Apply multiple actions to create a new tree, then commit.
pub fn apply_actions(
    repo: &Repository,
    branch: &str,
    actions: &[CommitActionOp],
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<Oid, GitError> {
    let branch_ref = format!("refs/heads/{branch}");

    let base_tree = repo
        .find_reference(&branch_ref)
        .ok()
        .and_then(|r| r.target())
        .and_then(|oid| repo.find_commit(oid).ok())
        .and_then(|c| c.tree().ok());

    let mut current_tree_oid = base_tree.as_ref().map(|t| t.id());

    for action in actions {
        current_tree_oid = Some(apply_single_action(
            repo,
            current_tree_oid
                .and_then(|oid| repo.find_tree(oid).ok())
                .as_ref(),
            action,
        )?);
    }

    let final_tree_oid = current_tree_oid
        .ok_or_else(|| GitError::Internal("No tree after applying actions".to_string()))?;

    create_commit_on_branch(
        repo,
        branch,
        final_tree_oid,
        message,
        author_name,
        author_email,
    )
}

fn apply_single_action(
    repo: &Repository,
    base_tree: Option<&Tree<'_>>,
    action: &CommitActionOp,
) -> Result<Oid, GitError> {
    match action {
        CommitActionOp::Create { path, content } => {
            if let Some(tree) = base_tree {
                if file_exists(tree, path) {
                    return Err(GitError::Conflict(format!(
                        "File already exists: {path}. Provide 'sha' to update."
                    )));
                }
            }
            tree_with_blob(repo, base_tree, path, content)
        }
        CommitActionOp::Update { path, content, sha } => {
            if let Some(expected_sha) = sha {
                if let Some(tree) = base_tree {
                    verify_blob_sha(tree, path, expected_sha)?;
                }
            }
            tree_with_blob(repo, base_tree, path, content)
        }
        CommitActionOp::Delete { path, sha } => {
            let tree = base_tree.ok_or_else(|| GitError::PathNotFound(path.clone()))?;
            verify_blob_sha(tree, path, sha)?;
            tree_without_entry(repo, tree, path)
        }
        CommitActionOp::Move { from, to, sha } => {
            let tree = base_tree.ok_or_else(|| GitError::PathNotFound(from.clone()))?;

            if let Some(expected_sha) = sha {
                verify_blob_sha(tree, from, expected_sha)?;
            }

            let blob = get_blob_at_path(repo, tree, from)?;
            let content = blob.content().to_vec();

            let intermediate_tree_oid = tree_without_entry(repo, tree, from)?;
            let intermediate_tree = repo.find_tree(intermediate_tree_oid).map_err(|e| {
                GitError::Internal(format!("Failed to find intermediate tree: {e}"))
            })?;

            tree_with_blob(repo, Some(&intermediate_tree), to, &content)
        }
    }
}

/// Search for paths matching a glob pattern in the tree.
pub fn search_paths(
    repo: &Repository,
    tree: &Tree<'_>,
    pattern: &str,
    limit: usize,
) -> Result<Vec<String>, GitError> {
    let glob_pattern = glob::Pattern::new(pattern)
        .map_err(|e| GitError::Internal(format!("Invalid glob pattern: {e}")))?;

    let mut matches = Vec::new();
    collect_matching_paths(repo, tree, "", &glob_pattern, &mut matches, limit);
    Ok(matches)
}

fn collect_matching_paths(
    repo: &Repository,
    tree: &Tree<'_>,
    prefix: &str,
    pattern: &glob::Pattern,
    matches: &mut Vec<String>,
    limit: usize,
) {
    if matches.len() >= limit {
        return;
    }

    for entry in tree.iter() {
        if matches.len() >= limit {
            break;
        }

        let name = entry.name().unwrap_or("");
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };

        if pattern.matches(&path) {
            matches.push(path.clone());
        }

        if let Some(ObjectType::Tree) = entry.kind() {
            if let Ok(subtree) = repo.find_tree(entry.id()) {
                collect_matching_paths(repo, &subtree, &path, pattern, matches, limit);
            }
        }
    }
}

/// Get file history by walking commits that touch the given path.
pub fn get_file_history(
    repo: &Repository,
    start_oid: Oid,
    path: &str,
    limit: usize,
    cursor: Option<&str>,
) -> Result<(Vec<CommitResponse>, Option<String>, bool), GitError> {
    let mut revwalk = repo
        .revwalk()
        .map_err(|e| GitError::Internal(format!("Failed to create revwalk: {e}")))?;

    let start_commit = if let Some(cursor_sha) = cursor {
        Oid::from_str(cursor_sha)
            .map_err(|_| GitError::Internal(format!("Invalid cursor: {cursor_sha}")))?
    } else {
        start_oid
    };

    revwalk
        .push(start_commit)
        .map_err(|e| GitError::Internal(format!("Failed to start revwalk: {e}")))?;

    if cursor.is_some() {
        revwalk.next();
    }

    let path_obj = Path::new(path);
    let mut commits = Vec::new();

    for oid_result in revwalk {
        if commits.len() > limit {
            break;
        }

        let commit_oid =
            oid_result.map_err(|e| GitError::Internal(format!("Revwalk error: {e}")))?;

        let commit = get_commit(repo, commit_oid)?;

        let tree = commit.tree().ok();
        let current_entry = tree.as_ref().and_then(|t| t.get_path(path_obj).ok());

        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let parent_entry = parent_tree.as_ref().and_then(|t| t.get_path(path_obj).ok());

        let touches_path = match (&current_entry, &parent_entry) {
            (Some(curr), Some(par)) => curr.id() != par.id(),
            (Some(_), None) | (None, Some(_)) => true,
            (None, None) => false,
        };

        if touches_path {
            let stats = compute_commit_stats(repo, &commit);
            commits.push(commit_to_response(&commit, stats));
        }
    }

    let has_more = commits.len() > limit;
    let next_cursor = if has_more {
        commits.pop();
        commits.last().map(|c| c.sha.clone())
    } else {
        None
    };

    Ok((commits, next_cursor, has_more))
}
