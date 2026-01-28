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
            GitError::Internal(msg) => ApiError::internal(msg),
        }
    }
}

pub fn open_repo(path: &Path) -> Result<Repository, GitError> {
    Repository::open_bare(path).map_err(|_| GitError::RepoNotFound)
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
