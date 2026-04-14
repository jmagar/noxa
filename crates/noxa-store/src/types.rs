use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of a [`FilesystemContentStore::write`] call.
///
/// Marked `#[non_exhaustive]` so that future additive fields do not constitute
/// a semver-breaking change for downstream crates that pattern-match or
/// construct this struct.
#[non_exhaustive]
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
