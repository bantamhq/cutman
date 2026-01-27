use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
}

impl ServerConfig {
    #[must_use]
    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("invalid socket address")
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
        }
    }
}
