use std::fs;
use std::sync::Arc;

use anyhow::bail;
use chrono::Utc;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use cutman::auth::TokenGenerator;
use cutman::config::ServerConfig;
use cutman::server::{AppState, create_router};
use cutman::store::{SqliteStore, Store};
use cutman::types::{Namespace, Token, User};

fn create_token(
    generator: &TokenGenerator,
    is_admin: bool,
    user_id: Option<String>,
) -> anyhow::Result<(Token, String)> {
    let (raw_token, lookup, hash) = generator.generate()?;
    let token = Token {
        id: Uuid::new_v4().to_string(),
        token_hash: hash,
        token_lookup: lookup,
        is_admin,
        user_id,
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
    /// Administrative commands
    Admin {
        #[command(subcommand)]
        command: AdminCommands,
    },

    /// Start the server
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to bind to
        #[arg(long, short, default_value = "8080")]
        port: u16,

        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// Public base URL for external access (e.g., "https://git.example.com").
        /// Used for generating LFS action URLs. If not set, URLs are derived from request headers.
        #[arg(long)]
        public_base_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum AdminCommands {
    /// Initialize the server (create database and admin token)
    Init {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// Skip interactive prompts
        #[arg(long)]
        non_interactive: bool,
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
        create_default_user_prompt(&store, &generator)?;
    }

    Ok(())
}

fn create_default_user_prompt(store: &SqliteStore, generator: &TokenGenerator) -> anyhow::Result<()> {
    let create_user = inquire::Confirm::new("Would you like to create a default user?")
        .with_default(false)
        .prompt()?;

    if !create_user {
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
    let user_id = Uuid::new_v4().to_string();

    let namespace = Namespace {
        id: namespace_id.clone(),
        name: username.clone(),
        created_at: now,
        repo_limit: None,
        storage_limit_bytes: None,
        external_id: None,
    };

    let user = User {
        id: user_id.clone(),
        primary_namespace_id: namespace_id,
        created_at: now,
        updated_at: now,
    };

    store.create_namespace(&namespace)?;
    store.create_user(&user)?;

    let (user_token, raw_token) = create_token(generator, false, Some(user_id))?;
    store.create_token(&user_token)?;

    println!();
    println!("========================================");
    println!("Created user '{username}' with token:");
    println!();
    println!("  {raw_token}");
    println!();
    println!("========================================");
    println!();

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        },
        Commands::Serve {
            host,
            port,
            data_dir,
            public_base_url,
        } => {
            let config = ServerConfig {
                host,
                port,
                data_dir: data_dir.into(),
                public_base_url,
            };

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

            let state = Arc::new(AppState {
                store: Arc::new(store),
                data_dir: config.data_dir.clone(),
                public_base_url: config.public_base_url.clone(),
            });

            let app = create_router(state);
            let addr = config.socket_addr()?;

            info!("Starting server on {}", addr);

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
