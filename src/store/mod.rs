pub mod path;
mod schema;
mod sqlite;

pub use sqlite::SqliteStore;

use crate::error::Result;
use crate::types::*;

/// Store defines the database interface.
pub trait Store: Send + Sync {
    fn initialize(&self) -> Result<()>;

    // Namespace operations
    fn create_namespace(&self, ns: &Namespace) -> Result<()>;
    fn get_namespace(&self, id: &str) -> Result<Option<Namespace>>;
    fn get_namespace_by_name(&self, name: &str) -> Result<Option<Namespace>>;
    fn list_namespaces(&self, cursor: &str, limit: i32) -> Result<Vec<Namespace>>;
    fn update_namespace(&self, ns: &Namespace) -> Result<()>;
    fn delete_namespace(&self, id: &str) -> Result<bool>;

    // User operations
    fn create_user(&self, user: &User) -> Result<()>;
    fn get_user(&self, id: &str) -> Result<Option<User>>;
    fn get_user_by_primary_namespace_id(&self, namespace_id: &str) -> Result<Option<User>>;
    fn list_users(&self, cursor: &str, limit: i32) -> Result<Vec<User>>;
    fn update_user(&self, user: &User) -> Result<()>;
    fn delete_user(&self, id: &str) -> Result<bool>;

    // Token operations
    fn create_token(&self, token: &Token) -> Result<()>;
    fn get_token_by_id(&self, id: &str) -> Result<Option<Token>>;
    fn get_token_by_lookup(&self, lookup: &str) -> Result<Option<Token>>;
    fn list_tokens(&self, cursor: &str, limit: i32) -> Result<Vec<Token>>;
    fn list_user_tokens(&self, user_id: &str) -> Result<Vec<Token>>;
    fn delete_token(&self, id: &str) -> Result<bool>;
    fn update_token_last_used(&self, id: &str) -> Result<()>;

    // Repo operations
    fn create_repo(&self, repo: &Repo) -> Result<()>;
    fn get_repo(&self, namespace_id: &str, name: &str) -> Result<Option<Repo>>;
    fn get_repo_by_id(&self, id: &str) -> Result<Option<Repo>>;
    fn list_repos(&self, namespace_id: &str, cursor: &str, limit: i32) -> Result<Vec<Repo>>;
    fn update_repo(&self, repo: &Repo) -> Result<()>;
    fn delete_repo(&self, id: &str) -> Result<bool>;
    fn update_repo_last_push(&self, id: &str) -> Result<()>;
    fn update_repo_size(&self, id: &str, size_bytes: i64) -> Result<()>;

    // Tag operations (many-to-many with repos)
    fn create_tag(&self, tag: &Tag) -> Result<()>;
    fn get_tag_by_id(&self, id: &str) -> Result<Option<Tag>>;
    fn get_tag_by_name(&self, namespace_id: &str, name: &str) -> Result<Option<Tag>>;
    fn list_tags(&self, namespace_id: &str, cursor: &str, limit: i32) -> Result<Vec<Tag>>;
    fn update_tag(&self, tag: &Tag) -> Result<()>;
    fn delete_tag(&self, id: &str) -> Result<bool>;
    fn count_tag_repos(&self, id: &str) -> Result<i32>;

    // Repo-Tag M2M operations
    fn add_repo_tag(&self, repo_id: &str, tag_id: &str) -> Result<()>;
    fn remove_repo_tag(&self, repo_id: &str, tag_id: &str) -> Result<bool>;
    fn list_repo_tags(&self, repo_id: &str) -> Result<Vec<Tag>>;
    fn list_tag_repos(&self, tag_id: &str) -> Result<Vec<Repo>>;
    fn set_repo_tags(&self, repo_id: &str, tag_ids: &[String]) -> Result<()>;

    // Folder operations (materialized path, one-to-many with repos)
    fn get_folder_by_id(&self, id: i64) -> Result<Option<Folder>>;
    fn get_folder_by_path(&self, namespace_id: &str, path: &str) -> Result<Option<Folder>>;
    fn ensure_folder_path(&self, namespace_id: &str, path: &str) -> Result<i64>;
    fn list_all_folders(&self, namespace_id: &str) -> Result<Vec<Folder>>;
    fn move_folder(&self, id: i64, new_path: &str) -> Result<()>;
    fn delete_folder(&self, id: i64) -> Result<bool>;

    // Repo-Folder operations (one-to-many)
    fn set_repo_folder(&self, repo_id: &str, folder_id: Option<i64>) -> Result<()>;
    fn set_repo_folder_by_path(
        &self,
        repo_id: &str,
        namespace_id: &str,
        path: Option<&str>,
    ) -> Result<Option<i64>>;
    fn list_folder_repos(
        &self,
        namespace_id: &str,
        path: &str,
        recursive: bool,
    ) -> Result<Vec<Repo>>;

    // Namespace grant operations
    fn upsert_namespace_grant(&self, grant: &NamespaceGrant) -> Result<()>;
    fn delete_namespace_grant(&self, user_id: &str, namespace_id: &str) -> Result<bool>;
    fn get_namespace_grant(
        &self,
        user_id: &str,
        namespace_id: &str,
    ) -> Result<Option<NamespaceGrant>>;
    fn list_user_namespace_grants(&self, user_id: &str) -> Result<Vec<NamespaceGrant>>;
    fn list_namespace_grants_for_namespace(
        &self,
        namespace_id: &str,
    ) -> Result<Vec<NamespaceGrant>>;
    fn count_namespace_users(&self, namespace_id: &str) -> Result<i32>;

    // Repo grant operations
    fn upsert_repo_grant(&self, grant: &RepoGrant) -> Result<()>;
    fn delete_repo_grant(&self, user_id: &str, repo_id: &str) -> Result<bool>;
    fn get_repo_grant(&self, user_id: &str, repo_id: &str) -> Result<Option<RepoGrant>>;
    fn list_user_repo_grants(&self, user_id: &str) -> Result<Vec<RepoGrant>>;
    fn list_user_repos_with_grants(&self, user_id: &str, namespace_id: &str) -> Result<Vec<Repo>>;
    fn has_repo_grants_in_namespace(&self, user_id: &str, namespace_id: &str) -> Result<bool>;

    // LFS object operations
    fn create_lfs_object(&self, obj: &LfsObject) -> Result<()>;
    fn get_lfs_object(&self, repo_id: &str, oid: &str) -> Result<Option<LfsObject>>;
    fn list_lfs_objects(&self, repo_id: &str) -> Result<Vec<LfsObject>>;
    fn delete_lfs_object(&self, repo_id: &str, oid: &str) -> Result<bool>;
    fn get_repo_lfs_size(&self, repo_id: &str) -> Result<i64>;

    // Admin token check
    fn has_admin_token(&self) -> Result<bool>;

    fn close(&self) -> Result<()>;
}
