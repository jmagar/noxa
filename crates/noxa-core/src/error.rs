/// Extraction errors — kept minimal since this crate does no I/O.
/// Most failures come from malformed HTML or invalid URLs.
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("failed to parse HTML")]
    ParseError,

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("no content found")]
    NoContent,

    #[error("failed to spawn extraction worker: {reason}")]
    WorkerSpawn { reason: String },

    #[error("extraction worker timed out after {timeout_ms}ms")]
    WorkerTimeout { timeout_ms: u64 },

    #[error("extraction worker panicked: {message}")]
    WorkerPanic { message: String },

    #[error("failed to initialize JavaScript runtime: {reason}")]
    JavaScriptRuntimeInit { reason: String },

    #[error("JavaScript runtime failed during {stage}: {reason}")]
    JavaScriptRuntimeFailure {
        stage: &'static str,
        reason: String,
    },

    #[error("JavaScript execution timed out after {timeout_ms}ms")]
    JavaScriptTimeout { timeout_ms: u64 },
}
