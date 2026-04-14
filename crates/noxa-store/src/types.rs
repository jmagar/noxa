use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Errors produced by the noxa-store crate.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Filesystem I/O failure.
    #[error("store I/O: {0}")]
    Io(#[from] std::io::Error),

    /// Filesystem I/O failure with path context.
    #[error("store I/O error on {path}: {source}")]
    IoPath {
        source: std::io::Error,
        path: PathBuf,
    },

    /// JSON (de)serialization failure.
    #[error("store JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A computed store path would escape the root directory.
    #[error("store: path escapes root for url: {0}")]
    PathEscape(String),

    /// `$HOME` is not set; cannot determine the default store root.
    #[error("cannot determine home directory: $HOME is unset")]
    HomeDirUnavailable,

    /// A background task (`spawn_blocking`) failed to join.
    #[error("store task join: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

/// Result of a [`FilesystemContentStore::write`] call.
///
/// Marked `#[non_exhaustive]` so that future additive fields do not constitute
/// a semver-breaking change for downstream crates that pattern-match or
/// construct this struct.
#[non_exhaustive]
#[derive(Debug)]
pub struct StoreResult {
    pub md_path: PathBuf,
    pub json_path: PathBuf,
    pub is_new: bool,
    pub changed: bool,
    pub word_count_delta: i64,
    /// The diff computed against the previous version, if any.
    pub diff: Option<noxa_core::ContentDiff>,
}

/// Operation variant recorded in `.operations.ndjson`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Map,
    Brand,
    Summarize,
    Extract,
    Diff,
}

/// One line in `.operations.ndjson`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationEntry {
    pub op: Op,
    pub at: DateTime<Utc>,
    pub url: String,
    pub input: serde_json::Value,
    /// Truncated to 1 MiB; `output_truncated: true` field added when exceeded.
    pub output: serde_json::Value,
}
