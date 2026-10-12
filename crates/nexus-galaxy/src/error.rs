//! Error type of the galaxy database.

/// Errors from [`crate::GalaxyDb`].
#[derive(Debug, thiserror::Error)]
pub enum GalaxyError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the galaxy database writer thread has stopped")]
    WriterGone,
    #[error("{0}")]
    Other(String),
}

pub type Result<T, E = GalaxyError> = std::result::Result<T, E>;
