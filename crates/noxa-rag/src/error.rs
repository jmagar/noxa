use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum RagError {
    #[error("embed error: {message}")]
    Embed {
        message: String,
        status: Option<u16>,
    },
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

#[cfg(test)]
mod tests {
    use super::RagError;

    #[test]
    fn embed_error_exposes_status() {
        let err = RagError::Embed {
            message: "payload too large".to_string(),
            status: Some(413),
        };

        match err {
            RagError::Embed {
                status: Some(413), ..
            } => {}
            other => panic!("expected structured 413 embed error, got {other:?}"),
        }
    }
}
