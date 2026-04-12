use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A chunk produced from an ExtractionResult.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    pub source_url: String,
    pub domain: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub char_offset: usize,
    pub token_estimate: usize,
}

/// A point ready for upsert into the vector store.
#[derive(Debug, Clone)]
pub struct Point {
    /// UUID v5 deterministic ID: url#chunkN
    pub id: Uuid,
    pub vector: Vec<f32>,
    pub payload: PointPayload,
}

/// Payload stored alongside each vector in the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointPayload {
    pub text: String,
    /// Normalized URL (strip fragment, trailing slash, lowercase scheme+host).
    pub url: String,
    pub domain: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub token_estimate: usize,
}

/// A search result returned by VectorStore::search().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub text: String,
    pub url: String,
    pub score: f32,
    pub chunk_index: usize,
    pub token_estimate: usize,
}
