use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    /// Public base URL for external access (e.g., "https://git.example.com").
    /// Used for generating LFS action URLs. If not set, URLs are derived from request headers.
    pub public_base_url: Option<String>,
}

impl ServerConfig {
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }

    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("cutman.db")
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            data_dir: PathBuf::from("./data"),
            public_base_url: None,
        }
    }
}
