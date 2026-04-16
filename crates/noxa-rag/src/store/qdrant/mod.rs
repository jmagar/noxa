use reqwest::header::HeaderMap;

use crate::error::RagError;

mod http;
mod lifecycle;
mod normalize;
mod payload;
mod vector_store;

#[cfg(test)]
mod tests;

pub(crate) use normalize::normalize_url;

/// Qdrant-backed vector store.
pub struct QdrantStore {
    client: reqwest::Client,
    base_url: String,
    collection: String,
    _uuid_namespace: uuid::Uuid,
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
        })
    }
}
