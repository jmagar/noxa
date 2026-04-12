// QdrantStore — implemented in noxa-68r.4
use async_trait::async_trait;
use crate::error::RagError;
use crate::store::VectorStore;
use crate::types::{Point, SearchResult};

pub struct QdrantStore {
    pub(crate) client: qdrant_client::Qdrant,
    pub(crate) collection: String,
}

#[async_trait]
impl VectorStore for QdrantStore {
    async fn upsert(&self, _points: Vec<Point>) -> Result<(), RagError> {
        // Full implementation in noxa-68r.4
        Err(RagError::Store("QdrantStore not yet implemented".to_string()))
    }

    async fn delete_by_url(&self, _url: &str) -> Result<(), RagError> {
        Err(RagError::Store("QdrantStore not yet implemented".to_string()))
    }

    async fn search(&self, _vector: &[f32], _limit: usize) -> Result<Vec<SearchResult>, RagError> {
        Err(RagError::Store("QdrantStore not yet implemented".to_string()))
    }

    fn name(&self) -> &str {
        "qdrant"
    }
}
