use serde::{Deserialize, Serialize};

use crate::types::Namespace;

#[derive(Debug, Deserialize)]
pub struct CreateNamespaceRequest {
    pub name: String,
    #[serde(default)]
    pub repo_limit: Option<i32>,
    #[serde(default)]
    pub storage_limit_bytes: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePrincipalRequest {
    pub namespace_name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct CreatePrincipalTokenRequest {
    #[serde(default)]
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct NamespaceGrantRequest {
    pub namespace_id: String,
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RepoGrantRequest {
    pub repo_id: String,
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct NamespaceGrantResponse {
    pub namespace_id: String,
    pub allow: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct RepoGrantResponse {
    pub repo_id: String,
    pub allow: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub id: String,
    pub is_admin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub namespace_grants: Vec<NamespaceGrantResponse>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub repo_grants: Vec<RepoGrantResponse>,
}

#[derive(Debug, Serialize)]
pub struct CreateTokenResponse {
    pub token: String,
    pub metadata: TokenResponse,
}

#[derive(Debug, Default, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ListReposParams {
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRepoRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRepoRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub public: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTagRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFolderRequest {
    pub path: String,
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFolderRequest {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNamespaceRequest {
    #[serde(default)]
    pub repo_limit: Option<i32>,
    #[serde(default)]
    pub storage_limit_bytes: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RepoTagsRequest {
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DeleteTagParams {
    #[serde(default)]
    pub force: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ListTagsParams {
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ListFoldersParams {
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ListFolderReposParams {
    #[serde(default)]
    pub recursive: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SetRepoFolderRequest {
    #[serde(default)]
    pub folder_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PrincipalGrantResponse {
    pub principal_id: String,
    pub allow: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct NamespaceResponse {
    #[serde(flatten)]
    pub namespace: Namespace,
    pub is_primary: bool,
}
