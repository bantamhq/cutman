use std::fs;
use std::sync::Arc;

use chrono::Utc;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use cutman::auth::TokenGenerator;
use cutman::config::ServerConfig;
use cutman::server::{AppState, create_router};
use cutman::store::{SqliteStore, Store};
use cutman::types::Token;

#[derive(Parser)]
#[command(name = "cutman")]
#[command(about = "A Git hosting server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("cutman=info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            host,
            port,
            data_dir,
        } => {
            let config = ServerConfig {
                host,
                port,
                data_dir: data_dir.into(),
            };

            fs::create_dir_all(&config.data_dir)?;

            let store = SqliteStore::new(config.db_path())?;
            store.initialize()?;

            if !store.has_admin_token()? {
                let generator = TokenGenerator::new();
                let (raw_token, lookup, hash) = generator.generate()?;

                let token = Token {
                    id: Uuid::new_v4().to_string(),
                    token_hash: hash,
                    token_lookup: lookup,
                    is_admin: true,
                    user_id: None,
                    created_at: Utc::now(),
                    expires_at: None,
                    last_used_at: None,
                };

                store.create_token(&token)?;

                println!();
                println!("========================================");
                println!("Initial admin token (save this, it won't be shown again):");
                println!();
                println!("  {raw_token}");
                println!();
                println!("========================================");
                println!();
            }

            let state = Arc::new(AppState {
                store: Arc::new(store),
            });

            let app = create_router(state);
            let addr = config.socket_addr();

            info!("Starting server on {}", addr);

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
