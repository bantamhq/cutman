use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("not found")]
    NotFound,

    #[error("already exists")]
    AlreadyExists,

    #[error("token lookup collision")]
    TokenLookupCollision,

    #[error("cannot grant permissions to primary namespace owner")]
    PrimaryNamespaceGrant,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("invalid token format")]
    InvalidTokenFormat,

    #[error("token expired")]
    TokenExpired,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid permission: {0}")]
    InvalidPermission(String),
}

pub type Result<T> = std::result::Result<T, Error>;
