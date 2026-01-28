mod commands;
mod info;
mod namespace;
mod permission;
mod pickers;
mod token;
mod user;

pub use commands::{
    AdminCommands, NamespaceCommands, PermissionCommands, TokenCommands, UserCommands,
};
pub use info::run_info;
pub use namespace::{run_namespace_add, run_namespace_remove};
pub use permission::{run_permission_grant, run_permission_revoke};
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
