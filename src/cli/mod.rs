mod auth;
mod commands;
pub mod credentials;
pub mod http_client;
mod info;
mod namespace;
mod new;
mod permission;
pub mod pickers;
mod repo;
mod tag;
mod token;
mod user;

pub use auth::run_auth_login;
pub use commands::{
    AdminCommands, AuthCommands, NamespaceCommands, PermissionCommands, RepoCommands, TagCommands,
    TokenCommands, UserCommands,
};
pub use info::run_info;
pub use namespace::{run_namespace_add, run_namespace_remove};
pub use new::run_new;
pub use permission::{run_permission_grant, run_permission_revoke};
pub use repo::{run_repo_clone, run_repo_delete, run_repo_tag};
pub use tag::{run_tag_create, run_tag_delete};
pub use token::{run_token_create, run_token_revoke};
pub use user::{run_user_add, run_user_remove};

use crate::store::SqliteStore;

/// Initialize store from data directory, checking it exists
pub fn init_store(data_dir: &str) -> anyhow::Result<SqliteStore> {
    let data_path: std::path::PathBuf = data_dir.into();
    let db_path = data_path.join("cutman.db");

    if !db_path.exists() {
        anyhow::bail!(
            "Database not found at {}. Run 'cutman admin init' first.",
            db_path.display()
        );
    }

    SqliteStore::new(&db_path).map_err(Into::into)
}
