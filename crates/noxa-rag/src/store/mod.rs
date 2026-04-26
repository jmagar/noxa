use async_trait::async_trait;
use std::sync::Arc;

use crate::error::RagError;
pub use crate::types::{Point, SearchMetadataFilter, SearchResult};

/// Three-way result for startup delta-scan existence checks.
///
/// Distinguishes a confirmed "already indexed" state from a genuine "not found"
/// vs. an indeterminate backend failure.  The startup scan MUST NOT re-queue a
/// file when the result is `BackendError` — that would turn a transient Qdrant
/// outage into a full reindex storm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashExistsResult {
    /// At least one point with matching URL+hash exists — file is up to date.
    Exists,
    /// No matching point found — file should be re-indexed.
    NotIndexed,
    /// The backend returned an error or unexpected status — outcome is unknown.
    /// The caller should treat this conservatively (skip re-queue, keep current index).
    BackendError(String),
}

/// Pluggable vector store backend.
///
/// Trait surface is minimal — only what ALL impls share.
/// Collection lifecycle (create_collection, collection_exists) lives in factory.rs
/// as concrete methods on each store struct, called during startup probes.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Upsert points into the store. Returns the number of points written.
    async fn upsert(&self, points: Vec<Point>) -> Result<usize, RagError>;
    /// Delete all points for a given URL.
    async fn delete_by_url(&self, url: &str) -> Result<(), RagError>;
    /// Delete all points for a given URL whose IDs are NOT in `keep_ids`.
    ///
    /// Used for two-phase replace: upsert new points first, then call this to
    /// evict only the stale points, so a transient upsert failure never leaves
    /// the collection empty.
    async fn delete_stale_by_url(&self, url: &str, keep_ids: &[uuid::Uuid])
    -> Result<(), RagError>;
    /// Search by vector similarity, optionally constrained by landed metadata.
    async fn search(
        &self,
        vector: &[f32],
        limit: usize,
        filter: Option<&SearchMetadataFilter>,
    ) -> Result<Vec<SearchResult>, RagError>;
    /// Return the total number of indexed points in the collection.
    async fn collection_point_count(&self) -> Result<u64, RagError>;
    /// Three-way existence check used by the startup delta scan.
    ///
    /// Returns [`HashExistsResult::BackendError`] instead of `Ok(false)` on any
    /// Qdrant communication failure so callers can avoid triggering a reindex.
    async fn url_with_hash_exists_checked(&self, url: &str, hash: &str) -> HashExistsResult;

    /// Three-way existence check on `file_hash` (xxHash3 of raw bytes).
    ///
    /// Used by the startup delta scan to skip re-embedding files whose raw bytes
    /// have not changed since last indexing. Faster than SHA-256 content_hash checks.
    async fn url_with_file_hash_exists_checked(
        &self,
        url: &str,
        file_hash: &str,
    ) -> HashExistsResult;

    fn name(&self) -> &str;
}

pub type DynVectorStore = Arc<dyn VectorStore + Send + Sync>;

pub mod qdrant;
pub use qdrant::QdrantStore;
