use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

/// Configuration for the server, loadable from TOML file.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// Public base URL for external access (e.g., "https://git.example.com").
    /// Used for generating LFS action URLs. If not set, URLs are derived from request headers.
    #[serde(default)]
    pub public_base_url: Option<String>,
}

/// CLI overrides that can be applied on top of a config file.
#[derive(Debug, Default)]
pub struct ServerConfigOverrides {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub data_dir: Option<PathBuf>,
    pub public_base_url: Option<String>,
}

impl ServerConfig {
    /// Default config file search paths.
    const SEARCH_PATHS: &'static [&'static str] = &["./server.toml", "/etc/cutman/server.toml"];

    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }

    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("cutman.db")
    }

    /// Load config from a specific file path.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Search for config file in default locations and load if found.
    pub fn load_from_search_paths() -> Option<Self> {
        for path_str in Self::SEARCH_PATHS {
            let path = Path::new(path_str);
            if path.exists() {
                if let Ok(config) = Self::load(path) {
                    return Some(config);
                }
            }
        }
        None
    }

    /// Load config with CLI overrides.
    ///
    /// Priority: CLI args > config file > defaults
    pub fn load_with_overrides(
        config_path: Option<&Path>,
        overrides: ServerConfigOverrides,
    ) -> anyhow::Result<Self> {
        let mut config = if let Some(path) = config_path {
            Self::load(path)?
        } else {
            Self::load_from_search_paths().unwrap_or_default()
        };

        if let Some(host) = overrides.host {
            config.host = host;
        }
        if let Some(port) = overrides.port {
            config.port = port;
        }
        if let Some(data_dir) = overrides.data_dir {
            config.data_dir = data_dir;
        }
        if overrides.public_base_url.is_some() {
            config.public_base_url = overrides.public_base_url;
        }

        Ok(config)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            data_dir: default_data_dir(),
            public_base_url: None,
        }
    }
}
