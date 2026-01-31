//! # Cutman
//!
//! A Git hosting server, usable both as a standalone binary and as a library.
//!
//! ## Library Usage
//!
//! ```toml
//! [dependencies]
//! cutman = { version = "0.1", default-features = false }
//! ```
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use std::path::PathBuf;
//! use cutman::server::{AppState, create_router};
//! use cutman::store::SqliteStore;
//!
//! let store = SqliteStore::new(&PathBuf::from("./data/cutman.db")).unwrap();
//! store.initialize().unwrap();
//!
//! let state = Arc::new(AppState::new(
//!     Arc::new(store),
//!     PathBuf::from("./data"),
//!     None,
//! ));
//! let router = create_router(state);
//! // Serve with axum...
//! ```
//!
//! ## Feature Flags
//!
//! - `cli` (default): Includes CLI module. Disable with `default-features = false`.

pub mod auth;
#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod error;
pub mod lfs;
pub mod server;
pub mod store;
pub mod types;
