mod auth;
mod commands;
mod credential;
pub mod credentials;
mod folder;
pub mod http_client;
mod info;
mod namespace;
mod new;
mod permission;
pub mod pickers;
mod principal;
pub mod repo;
mod tag;
mod token;

pub use auth::{run_auth_login, run_auth_logout};
pub use commands::{
    AdminCommands, AuthCommands, CredentialCommands, FolderCommands, NamespaceCommands,
    PermissionCommands, PrincipalCommands, RepoCommands, TagCommands, TokenCommands,
};
pub use credential::{
    print_credential_help, run_credential_erase, run_credential_get, run_credential_store,
};
pub use folder::{run_folder_create, run_folder_delete, run_folder_list, run_folder_move};
pub use info::run_info;
pub use namespace::{run_namespace_add, run_namespace_remove};
pub use new::run_new;
pub use permission::{
    run_permission_grant, run_permission_repo_grant, run_permission_repo_revoke,
    run_permission_revoke,
};
pub use principal::{run_principal_add, run_principal_remove};
pub use repo::{run_repo_clone, run_repo_delete, run_repo_move, run_repo_tag};
pub use tag::{run_tag_create, run_tag_delete};
pub use token::{run_token_create, run_token_revoke};

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
