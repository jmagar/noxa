use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum RagError {
    #[error("embed error: {0}")]
    Embed(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("chunk error: {0}")]
    Chunk(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("error: {0}")]
    Generic(String),
}
