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
    /// Filesystem watcher could not be created or a watch directory could not
    /// be registered. Fatal at pipeline startup.
    #[error("watcher setup error: {0}")]
    WatcherSetup(String),
    /// Workers did not drain within the configured timeout on shutdown.
    #[error("pipeline drain timed out")]
    DrainTimeout,
    /// A file path was rejected because it escaped the configured watch roots.
    #[error("path confinement violation: {0}")]
    PathConfinement(std::path::PathBuf),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    Parse(String),
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
