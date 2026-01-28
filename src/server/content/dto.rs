use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_BLOB_SIZE: i64 = 1_048_576;
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
pub struct BlobResponse {
    pub sha: String,
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub encoding: String,
    pub is_binary: bool,
    pub is_truncated: bool,
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
pub struct BlobParams {
    pub raw: Option<bool>,
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
