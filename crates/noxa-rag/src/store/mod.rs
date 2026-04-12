use async_trait::async_trait;
use std::sync::Arc;

use crate::error::RagError;
use crate::types::{Point, SearchResult};

/// Pluggable vector store backend.
///
/// Trait surface is minimal — only what ALL impls share.
/// Collection lifecycle (create_collection, collection_exists) lives in factory.rs
/// as concrete methods on each store struct, called during startup probes.
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(&self, points: Vec<Point>) -> Result<(), RagError>;
    async fn delete_by_url(&self, url: &str) -> Result<(), RagError>;
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchResult>, RagError>;
    fn name(&self) -> &str;
}

pub type DynVectorStore = Arc<dyn VectorStore + Send + Sync>;

pub mod qdrant;
pub use qdrant::QdrantStore;
