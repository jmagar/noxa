use async_trait::async_trait;

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder,
    Distance, FieldType, Filter, HnswConfigDiffBuilder, PointStruct, SearchPointsBuilder,
    UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Payload;

use crate::error::RagError;
use crate::store::VectorStore;
use crate::types::{Point, SearchResult};

pub struct QdrantStore {
    client: Qdrant,
    collection: String,
    #[allow(dead_code)]
    uuid_namespace: uuid::Uuid,
}

impl QdrantStore {
    /// Create a new QdrantStore.
    ///
    /// `url` should be the gRPC endpoint, typically `http://localhost:6334`.
    /// The crate uses gRPC transport via tonic — the `reqwest` feature only
    /// enables snapshot downloads, not a REST transport.
    pub fn new(
        url: &str,
        collection: String,
        api_key: Option<String>,
        uuid_namespace: uuid::Uuid,
    ) -> Result<Self, RagError> {
        let mut builder = Qdrant::from_url(url);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }
        let client = builder
            .build()
            .map_err(|e| RagError::Store(format!("failed to build qdrant client: {e}")))?;

        Ok(Self {
            client,
            collection,
            uuid_namespace,
        })
    }

    /// Check whether the collection already exists.
    pub async fn collection_exists(&self) -> Result<bool, RagError> {
        self.client
            .collection_exists(&self.collection)
            .await
            .map_err(|e| RagError::Store(format!("collection_exists failed: {e}")))
    }

    /// Create the collection with cosine distance, HNSW m=16/ef_construct=200,
    /// and payload indexes on `url` + `domain`.
    pub async fn create_collection(&self, dims: usize) -> Result<(), RagError> {
        let hnsw = HnswConfigDiffBuilder::default()
            .m(16)
            .ef_construct(200)
            .build();

        let vectors = VectorParamsBuilder::new(dims as u64, Distance::Cosine)
            .on_disk(true)
            .hnsw_config(hnsw);

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection)
                    .vectors_config(vectors)
                    .on_disk_payload(true),
            )
            .await
            .map_err(|e| RagError::Store(format!("create_collection failed: {e}")))?;

        // Payload indexes for fast filtering by url and domain.
        for field in ["url", "domain"] {
            self.client
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    &self.collection,
                    field,
                    FieldType::Keyword,
                ))
                .await
                .map_err(|e| {
                    RagError::Store(format!("create_field_index({field}) failed: {e}"))
                })?;
        }

        Ok(())
    }
}

#[async_trait]
impl VectorStore for QdrantStore {
    /// Upsert points into the collection in batches of 256.
    async fn upsert(&self, points: Vec<Point>) -> Result<(), RagError> {
        for chunk in points.chunks(256) {
            let qdrant_points: Vec<PointStruct> = chunk
                .iter()
                .map(|p| {
                    let mut payload = Payload::new();
                    payload.insert("text", p.payload.text.as_str());
                    payload.insert("url", p.payload.url.as_str());
                    payload.insert("domain", p.payload.domain.as_str());
                    payload.insert("chunk_index", p.payload.chunk_index as i64);
                    payload.insert("total_chunks", p.payload.total_chunks as i64);
                    payload.insert("token_estimate", p.payload.token_estimate as i64);

                    PointStruct::new(
                        p.id.to_string(), // UUID as string PointId
                        p.vector.clone(),
                        payload,
                    )
                })
                .collect();

            self.client
                .upsert_points(
                    UpsertPointsBuilder::new(&self.collection, qdrant_points).wait(true),
                )
                .await
                .map_err(|e| RagError::Store(format!("upsert_points failed: {e}")))?;
        }

        Ok(())
    }

    /// Delete all points whose `url` payload field matches the normalized URL.
    async fn delete_by_url(&self, url: &str) -> Result<(), RagError> {
        let normalized = normalize_url(url);

        self.client
            .delete_points(
                DeletePointsBuilder::new(&self.collection)
                    .points(Filter::must([Condition::matches(
                        "url",
                        normalized.clone(),
                    )]))
                    .wait(true),
            )
            .await
            .map_err(|e| RagError::Store(format!("delete_points failed: {e}")))?;

        Ok(())
    }

    /// Search for the nearest `limit` vectors and return their payloads.
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchResult>, RagError> {
        let response = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection, vector.to_vec(), limit as u64)
                    .with_payload(true),
            )
            .await
            .map_err(|e| RagError::Store(format!("search_points failed: {e}")))?;

        let results = response
            .result
            .into_iter()
            .filter_map(|hit| {
                let text = hit.get("text").as_str()?.to_string();
                let url = hit.get("url").as_str()?.to_string();
                let chunk_index = hit.get("chunk_index").as_integer().unwrap_or(0) as usize;
                let token_estimate =
                    hit.get("token_estimate").as_integer().unwrap_or(0) as usize;

                Some(SearchResult {
                    text,
                    url,
                    score: hit.score,
                    chunk_index,
                    token_estimate,
                })
            })
            .collect();

        Ok(results)
    }

    fn name(&self) -> &str {
        "qdrant"
    }
}

/// Normalize a URL for consistent storage and lookup:
/// - Strip fragment
/// - Strip trailing slash from path
/// - Scheme and host are already lowercased by the `url` crate
fn normalize_url(url: &str) -> String {
    use url::Url;
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(&path);
    parsed.to_string()
}
