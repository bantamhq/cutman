use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::Permission;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_limit_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub primary_namespace_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub id: String,
    #[serde(skip)]
    pub token_hash: String,
    #[serde(skip)]
    pub token_lookup: String,
    pub is_admin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: String,
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub public: bool,
    pub size_bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_push_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub namespace_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceGrant {
    pub user_id: String,
    pub namespace_id: String,
    pub allow_bits: Permission,
    pub deny_bits: Permission,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoGrant {
    pub user_id: String,
    pub repo_id: String,
    pub allow_bits: Permission,
    pub deny_bits: Permission,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfsObject {
    pub repo_id: String,
    pub oid: String,
    pub size: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoWithFolders {
    #[serde(flatten)]
    pub repo: Repo,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<Folder>,
}
