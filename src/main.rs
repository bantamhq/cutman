use std::fs;
use std::sync::Arc;

use anyhow::bail;
use chrono::Utc;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use cutman::auth::TokenGenerator;
use cutman::cli::{
    AdminCommands, AuthCommands, CredentialCommands, FolderCommands, NamespaceCommands,
    PermissionCommands, PrincipalCommands, RepoCommands, TagCommands, TokenCommands,
    print_credential_help, run_auth_login, run_auth_logout, run_credential_erase,
    run_credential_get, run_credential_store, run_folder_create, run_folder_delete,
    run_folder_list, run_folder_move, run_info, run_namespace_add, run_namespace_remove, run_new,
    run_permission_grant, run_permission_repo_grant, run_permission_repo_revoke,
    run_permission_revoke, run_principal_add, run_principal_remove, run_repo_clone, run_repo_delete,
    run_repo_move, run_repo_tag, run_tag_create, run_tag_delete, run_token_create, run_token_revoke,
};
use cutman::config::{ServerConfig, ServerConfigOverrides};
use cutman::server::{AppState, create_router};
use cutman::store::{SqliteStore, Store};
use cutman::types::{Namespace, Principal, Token};

fn create_token(
    generator: &TokenGenerator,
    is_admin: bool,
    principal_id: Option<String>,
) -> anyhow::Result<(Token, String)> {
    let (raw_token, lookup, hash) = generator.generate()?;
    let token = Token {
        id: Uuid::new_v4().to_string(),
        token_hash: hash,
        token_lookup: lookup,
        is_admin,
        principal_id,
        created_at: Utc::now(),
        expires_at: None,
        last_used_at: None,
    };
    Ok((token, raw_token))
}

#[cfg(unix)]
fn set_restrictive_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
        tracing::warn!("Failed to set permissions on {}: {e}", path.display());
    }
}

#[derive(Parser)]
#[command(name = "cutman")]
#[command(about = "A Git hosting server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Administrative commands (requires direct database access)
    Admin {
        #[command(subcommand)]
        command: AdminCommands,
    },

    /// Start the server
    Serve {
        /// Config file path (default: ./server.toml or /etc/cutman/server.toml)
        #[arg(long, short)]
        config: Option<String>,

        /// Host to bind to (default: 127.0.0.1)
        #[arg(long)]
        host: Option<String>,

        /// Port to bind to (default: 8080)
        #[arg(long, short)]
        port: Option<u16>,

        /// Data directory for database and repositories (default: ./data)
        #[arg(long)]
        data_dir: Option<String>,

        /// Public base URL for external access (e.g., "https://git.example.com").
        /// Used for generating LFS action URLs. If not set, URLs are derived from request headers.
        #[arg(long)]
        public_base_url: Option<String>,
    },

    /// Authentication commands
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },

    /// Create a new repository
    New {
        /// Repository (format: namespace/repo or just repo for primary namespace)
        name: Option<String>,

        /// Git remote name (default: origin)
        #[arg(short, long, default_value = "origin")]
        remote: String,
    },

    /// Repository management
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
    },

    /// Tag management
    Tag {
        #[command(subcommand)]
        command: TagCommands,
    },

    /// Git credential helper (use: git config credential.helper "cutman credential")
    Credential {
        #[command(subcommand)]
        command: Option<CredentialCommands>,
    },

    /// Folder management
    Folder {
        #[command(subcommand)]
        command: FolderCommands,
    },
}

fn run_init(data_dir: String, non_interactive: bool) -> anyhow::Result<()> {
    let data_path: std::path::PathBuf = data_dir.into();
    fs::create_dir_all(&data_path)?;

    let db_path = data_path.join("cutman.db");
    let store = SqliteStore::new(&db_path)?;
    store.initialize()?;

    let token_file = data_path.join(".admin_token");

    if store.has_admin_token()? {
        bail!(
            "Server already initialized. Admin token exists at: {}",
            token_file.display()
        );
    }

    let generator = TokenGenerator::new();
    let (token, raw_token) = create_token(&generator, true, None)?;

    store.create_token(&token)?;
    fs::write(&token_file, &raw_token)?;

    #[cfg(unix)]
    set_restrictive_permissions(&token_file);

    println!();
    println!("========================================");
    println!("Admin token (save this, it won't be shown again):");
    println!();
    println!("  {raw_token}");
    println!();
    println!("Token also written to: {}", token_file.display());
    println!("========================================");
    println!();

    if !non_interactive {
        create_default_principal_prompt(&store, &generator)?;
    }

    Ok(())
}

fn create_default_principal_prompt(
    store: &SqliteStore,
    generator: &TokenGenerator,
) -> anyhow::Result<()> {
    let create_principal = inquire::Confirm::new("Would you like to create a default principal?")
        .with_default(false)
        .prompt()?;

    if !create_principal {
        return Ok(());
    }

    let username = inquire::Text::new("Username:")
        .with_validator(|input: &str| {
            if input.trim().is_empty() {
                Err("Username cannot be empty".into())
            } else if input.contains(char::is_whitespace) {
                Err("Username cannot contain whitespace".into())
            } else {
                Ok(inquire::validator::Validation::Valid)
            }
        })
        .prompt()?;

    let now = Utc::now();
    let namespace_id = Uuid::new_v4().to_string();
    let principal_id = Uuid::new_v4().to_string();

    let namespace = Namespace {
        id: namespace_id.clone(),
        name: username.clone(),
        created_at: now,
        repo_limit: None,
        storage_limit_bytes: None,
        external_id: None,
    };

    let principal = Principal {
        id: principal_id.clone(),
        primary_namespace_id: namespace_id,
        created_at: now,
        updated_at: now,
    };

    store.create_namespace(&namespace)?;
    store.create_principal(&principal)?;

    let (principal_token, raw_token) = create_token(generator, false, Some(principal_id))?;
    store.create_token(&principal_token)?;

    println!();
    println!("========================================");
    println!("Created principal '{username}' with token:");
    println!();
    println!("  {raw_token}");
    println!();
    println!("========================================");
    println!();

    Ok(())
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("cutman=info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Admin { command } => match command {
            AdminCommands::Init {
                data_dir,
                non_interactive,
            } => {
                run_init(data_dir, non_interactive)?;
            }
            AdminCommands::Principal { command } => match command {
                PrincipalCommands::Add {
                    data_dir,
                    username,
                    create_token,
                    non_interactive,
                } => {
                    run_principal_add(data_dir, username, create_token, non_interactive)?;
                }
                PrincipalCommands::Remove {
                    data_dir,
                    principal_id,
                    non_interactive,
                    yes,
                } => {
                    run_principal_remove(data_dir, principal_id, non_interactive, yes)?;
                }
            },
            AdminCommands::Token { command } => match command {
                TokenCommands::Create {
                    data_dir,
                    principal_id,
                    expires_days,
                    non_interactive,
                } => {
                    run_token_create(data_dir, principal_id, expires_days, non_interactive)?;
                }
                TokenCommands::Revoke {
                    data_dir,
                    token_id,
                    non_interactive,
                    yes,
                } => {
                    run_token_revoke(data_dir, token_id, non_interactive, yes)?;
                }
            },
            AdminCommands::Namespace { command } => match command {
                NamespaceCommands::Add {
                    data_dir,
                    name,
                    non_interactive,
                } => {
                    run_namespace_add(data_dir, name, non_interactive)?;
                }
                NamespaceCommands::Remove {
                    data_dir,
                    namespace_id,
                    non_interactive,
                    yes,
                } => {
                    run_namespace_remove(data_dir, namespace_id, non_interactive, yes)?;
                }
            },
            AdminCommands::Permission { command } => match command {
                PermissionCommands::Grant {
                    data_dir,
                    principal_id,
                    namespace_id,
                    permissions,
                    non_interactive,
                } => {
                    run_permission_grant(
                        data_dir,
                        principal_id,
                        namespace_id,
                        permissions,
                        non_interactive,
                    )?;
                }
                PermissionCommands::Revoke {
                    data_dir,
                    principal_id,
                    namespace_id,
                    non_interactive,
                    yes,
                } => {
                    run_permission_revoke(data_dir, principal_id, namespace_id, non_interactive, yes)?;
                }
                PermissionCommands::RepoGrant {
                    data_dir,
                    principal_id,
                    repo_id,
                    permissions,
                    non_interactive,
                } => {
                    run_permission_repo_grant(
                        data_dir,
                        principal_id,
                        repo_id,
                        permissions,
                        non_interactive,
                    )?;
                }
                PermissionCommands::RepoRevoke {
                    data_dir,
                    principal_id,
                    repo_id,
                    non_interactive,
                    yes,
                } => {
                    run_permission_repo_revoke(data_dir, principal_id, repo_id, non_interactive, yes)?;
                }
            },
            AdminCommands::Info { data_dir, json } => {
                run_info(data_dir, json)?;
            }
        },
        Commands::Serve {
            config,
            host,
            port,
            data_dir,
            public_base_url,
        } => {
            let overrides = ServerConfigOverrides {
                host,
                port,
                data_dir: data_dir.map(Into::into),
                public_base_url,
            };
            let config_path = config.as_ref().map(std::path::Path::new);
            let server_config = ServerConfig::load_with_overrides(config_path, overrides)?;
            run_server(server_config)?;
        }
        Commands::Auth { command } => match command {
            AuthCommands::Login {
                server,
                token,
                non_interactive,
            } => {
                run_auth_login(server, token, non_interactive)?;
            }
            AuthCommands::Logout => {
                run_auth_logout()?;
            }
        },
        Commands::New { name, remote } => {
            run_new(name, remote)?;
        }
        Commands::Repo { command } => match command {
            RepoCommands::Delete {
                repo,
                non_interactive,
                yes,
            } => {
                run_repo_delete(repo, non_interactive, yes)?;
            }
            RepoCommands::Clone {
                repo,
                non_interactive,
            } => {
                run_repo_clone(repo, non_interactive)?;
            }
            RepoCommands::Tag {
                repo,
                tags,
                non_interactive,
            } => {
                run_repo_tag(repo, tags, non_interactive)?;
            }
            RepoCommands::Move {
                repo,
                folder,
                non_interactive,
            } => {
                run_repo_move(repo, folder, non_interactive)?;
            }
        },
        Commands::Tag { command } => match command {
            TagCommands::Create {
                name,
                color,
                namespace,
                non_interactive,
            } => {
                run_tag_create(name, color, namespace, non_interactive)?;
            }
            TagCommands::Delete {
                tag_id,
                namespace,
                non_interactive,
                yes,
                force,
            } => {
                run_tag_delete(tag_id, namespace, non_interactive, yes, force)?;
            }
        },
        Commands::Credential { command } => match command {
            Some(CredentialCommands::Get) => {
                run_credential_get()?;
            }
            Some(CredentialCommands::Store) => {
                run_credential_store()?;
            }
            Some(CredentialCommands::Erase) => {
                run_credential_erase()?;
            }
            None => {
                print_credential_help();
            }
        },
        Commands::Folder { command } => match command {
            FolderCommands::Create {
                path,
                namespace,
                non_interactive,
            } => {
                run_folder_create(path, namespace, non_interactive)?;
            }
            FolderCommands::List {
                namespace,
                non_interactive,
            } => {
                run_folder_list(namespace, non_interactive)?;
            }
            FolderCommands::Delete {
                path,
                namespace,
                non_interactive,
                yes,
            } => {
                run_folder_delete(path, namespace, non_interactive, yes)?;
            }
            FolderCommands::Move {
                old_path,
                new_path,
                namespace,
                non_interactive,
            } => {
                run_folder_move(old_path, new_path, namespace, non_interactive)?;
            }
        },
    }

    Ok(())
}

#[tokio::main]
async fn run_server(config: ServerConfig) -> anyhow::Result<()> {
    let token_file = config.data_dir.join(".admin_token");
    if !token_file.exists() {
        bail!(
            "Server not initialized. Run 'cutman admin init' first to create the database and admin token."
        );
    }

    let store = SqliteStore::new(config.db_path())?;
    if !store.has_admin_token()? {
        bail!(
            "Server not initialized. Run 'cutman admin init' first to create the database and admin token."
        );
    }

    info!("Admin token available at {}", token_file.display());

    let state = Arc::new(AppState::new(
        Arc::new(store),
        config.data_dir.clone(),
        config.public_base_url.clone(),
    ));

    let app = create_router(state);
    let addr = config.socket_addr()?;

    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
