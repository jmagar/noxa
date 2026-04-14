use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of a [`FilesystemContentStore::write`] call.
pub struct StoreResult {
    pub md_path: PathBuf,
    pub json_path: PathBuf,
    pub is_new: bool,
    pub changed: bool,
    pub word_count_delta: i64,
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
