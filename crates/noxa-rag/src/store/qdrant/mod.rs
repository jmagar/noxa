use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use reqwest::header::HeaderMap;

use crate::error::RagError;

mod http;
mod lifecycle;
mod payload;
mod vector_store;

#[cfg(test)]
mod tests;

/// Qdrant-backed vector store.
pub struct QdrantStore {
    client: reqwest::Client,
    base_url: String,
    collection: String,
    _uuid_namespace: uuid::Uuid,
    /// Cumulative count of payload decode failures observed during search.
    /// Incremented atomically so tests can assert failures were counted.
    pub(crate) decode_errors: Arc<AtomicU64>,
}

impl QdrantStore {
    pub fn new(
        url: &str,
        collection: String,
        api_key: Option<String>,
        uuid_namespace: uuid::Uuid,
    ) -> Result<Self, RagError> {
        let mut headers = HeaderMap::new();
        if let Some(key) = api_key {
            headers.insert(
                "api-key",
                key.parse()
                    .map_err(|_| RagError::Config("invalid Qdrant api-key".into()))?,
            );
        }
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| RagError::Config(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: url.trim_end_matches('/').to_string(),
            collection,
            _uuid_namespace: uuid_namespace,
            decode_errors: Arc::new(AtomicU64::new(0)),
        })
    }
}
