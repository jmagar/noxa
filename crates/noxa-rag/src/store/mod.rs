use async_trait::async_trait;
use std::sync::Arc;

use crate::error::RagError;
pub use crate::types::{Point, SearchMetadataFilter, SearchResult};

/// Pluggable vector store backend.
///
/// Trait surface is minimal — only what ALL impls share.
/// Collection lifecycle (create_collection, collection_exists) lives in factory.rs
/// as concrete methods on each store struct, called during startup probes.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Upsert points into the store. Returns the number of points written.
    async fn upsert(&self, points: Vec<Point>) -> Result<usize, RagError>;
    /// Delete all points for a given URL. Returns the number of points deleted.
    async fn delete_by_url(&self, url: &str) -> Result<u64, RagError>;
    /// Delete all points for a given URL whose IDs are NOT in `keep_ids`.
    ///
    /// Used for two-phase replace: upsert new points first, then call this to
    /// evict only the stale points, so a transient upsert failure never leaves
    /// the collection empty.
    async fn delete_stale_by_url(
        &self,
        url: &str,
        keep_ids: &[uuid::Uuid],
    ) -> Result<u64, RagError>;
    /// Search by vector similarity, optionally constrained by landed metadata.
    async fn search(
        &self,
        vector: &[f32],
        limit: usize,
        filter: Option<&SearchMetadataFilter>,
    ) -> Result<Vec<SearchResult>, RagError>;
    /// Return the total number of indexed points in the collection.
    async fn collection_point_count(&self) -> Result<u64, RagError>;
    /// Return true iff there is at least one point with both `url` and `content_hash`
    /// matching the given values. Used by the startup delta scan to skip already-indexed
    /// files whose content has not changed.
    async fn url_with_hash_exists(&self, url: &str, hash: &str) -> Result<bool, RagError>;
    fn name(&self) -> &str;
}

pub type DynVectorStore = Arc<dyn VectorStore + Send + Sync>;

pub mod qdrant;
pub use qdrant::QdrantStore;
