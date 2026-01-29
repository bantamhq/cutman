use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_BLOB_SIZE: i64 = 1_048_576;
pub const MAX_RAW_BLOB_SIZE: i64 = 100_000_000; // 100MB limit for raw blob downloads
pub const MAX_TREE_DEPTH: i32 = 10;
pub const DEFAULT_TREE_DEPTH: i32 = 1;
pub const DEFAULT_PAGE_SIZE: i32 = 20;
pub const MAX_PAGE_SIZE: i32 = 100;

#[derive(Debug, Serialize)]
pub struct RefResponse {
    pub name: String,
    #[serde(rename = "type")]
    pub ref_type: String,
    pub commit_sha: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
pub struct CommitResponse {
    pub sha: String,
    pub message: String,
    pub author: SignatureResponse,
    pub committer: SignatureResponse,
    pub parent_shas: Vec<String>,
    pub tree_sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<CommitStats>,
}

#[derive(Debug, Serialize)]
pub struct SignatureResponse {
    pub name: String,
    pub email: String,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommitStats {
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Serialize)]
pub struct TreeEntryResponse {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub sha: String,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_children: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeEntryResponse>,
}


#[derive(Debug, Serialize)]
pub struct DiffResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
    pub head_sha: String,
    pub stats: CommitStats,
    pub patch: String,
}

#[derive(Debug, Serialize)]
pub struct BlameLineResponse {
    pub line: usize,
    pub sha: String,
    pub author: SignatureResponse,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct BlameResponse {
    pub path: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub lines: Vec<BlameLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct CompareResponse {
    pub base_ref: String,
    pub head_ref: String,
    pub base_sha: String,
    pub head_sha: String,
    pub merge_base_sha: String,
    pub ahead_by: usize,
    pub behind_by: usize,
    pub commits: Vec<CommitResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub diff: DiffResponse,
}

#[derive(Debug, Serialize)]
pub struct ReadmeResponse {
    pub filename: String,
    pub content: String,
    pub size: i64,
    pub sha: String,
    pub is_binary: bool,
    pub is_truncated: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListCommitsParams {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub path: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct TreeParams {
    pub depth: Option<i32>,
}


#[derive(Debug, Deserialize)]
pub struct ArchiveParams {
    pub format: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadmeParams {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompareParams {
    pub cursor: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRefRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub ref_type: String,
    pub target_sha: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRefRequest {
    pub target_sha: String,
    #[serde(default)]
    pub expected_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetDefaultBranchRequest {
    pub branch: String,
}

// ============================================================================
// Content Mutation DTOs
// ============================================================================

/// Request to create or update a file
#[derive(Debug, Deserialize)]
pub struct PutBlobRequest {
    pub message: String,
    pub content: String,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub sha: Option<String>,
}

/// Request to delete a file
#[derive(Debug, Deserialize)]
pub struct DeleteBlobRequest {
    pub message: String,
    pub sha: String,
}

/// Request for multi-file atomic commit
#[derive(Debug, Deserialize)]
pub struct MultiCommitRequest {
    pub message: String,
    #[serde(default)]
    pub branch: Option<String>,
    pub actions: Vec<CommitAction>,
}

/// Tagged enum for commit actions
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum CommitAction {
    Create {
        path: String,
        content: String,
        #[serde(default)]
        encoding: Option<String>,
    },
    Update {
        path: String,
        content: String,
        #[serde(default)]
        encoding: Option<String>,
        #[serde(default)]
        sha: Option<String>,
    },
    Delete {
        path: String,
        sha: String,
    },
    Move {
        from: String,
        to: String,
        #[serde(default)]
        sha: Option<String>,
    },
}

/// Response for mutation operations
#[derive(Debug, Serialize)]
pub struct MutationResponse {
    pub commit_sha: String,
    pub ref_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<FileInfo>,
}

/// File info included in mutation responses
#[derive(Debug, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub sha: String,
    pub size: i64,
}

/// Query params for enhanced blob retrieval
#[derive(Debug, Deserialize)]
pub struct EnhancedBlobParams {
    #[serde(default)]
    pub raw: Option<bool>,
    #[serde(default)]
    pub history: Option<bool>,
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub parsed: Option<bool>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i32>,
}

/// Enhanced blob response with optional history and frontmatter
#[derive(Debug, Serialize)]
pub struct EnhancedBlobResponse {
    pub sha: String,
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub encoding: String,
    pub is_binary: bool,
    pub is_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<CommitResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_has_more: Option<bool>,
}

/// Query params for path search
#[derive(Debug, Deserialize)]
pub struct PathSearchParams {
    pub q: String,
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    #[serde(default)]
    pub limit: Option<i32>,
}

/// Response for path search
#[derive(Debug, Serialize)]
pub struct PathSearchResponse {
    pub matches: Vec<String>,
}
