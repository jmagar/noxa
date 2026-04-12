use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

use crate::error::RagError;
use crate::store::VectorStore;
use crate::types::{Point, SearchResult};

// ── REST request/response shapes ─────────────────────────────────────────────

#[derive(Deserialize)]
struct CollectionInfoResponse {
    result: Option<CollectionResult>,
}

#[derive(Deserialize)]
struct CollectionResult {
    config: CollectionConfig,
}

#[derive(Deserialize)]
struct CollectionConfig {
    params: CollectionParams,
}

#[derive(Deserialize)]
struct CollectionParams {
    vectors: serde_json::Value,
}

#[derive(Deserialize)]
struct CollectionVectors {
    size: usize,
}

#[derive(Deserialize)]
struct CollectionNamedVectors {
    vectors: HashMap<String, CollectionVectors>,
}

#[derive(Serialize)]
struct UpsertRequest {
    points: Vec<QdrantPoint>,
}

#[derive(Serialize)]
struct QdrantPoint {
    id: String, // UUID string
    vector: Vec<f32>,
    payload: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Serialize)]
struct DeleteByFilterRequest {
    filter: serde_json::Value,
}

#[derive(Serialize)]
struct SearchRequest {
    vector: Vec<f32>,
    limit: usize,
    with_payload: bool,
    score_threshold: Option<f32>,
}

#[derive(Deserialize)]
struct SearchResponse {
    result: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    score: f32,
    payload: Option<std::collections::HashMap<String, serde_json::Value>>,
}

// ── QdrantStore ───────────────────────────────────────────────────────────────

pub struct QdrantStore {
    client: reqwest::Client,
    base_url: String, // e.g. "http://127.0.0.1:53333"
    collection: String,
    uuid_namespace: uuid::Uuid,
}

impl QdrantStore {
    pub fn new(
        url: &str,
        collection: String,
        api_key: Option<String>,
        uuid_namespace: uuid::Uuid,
    ) -> Result<Self, RagError> {
        let mut headers = reqwest::header::HeaderMap::new();
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
            uuid_namespace,
        })
    }

    /// GET /collections/{name} → true if 200, false if 404.
    pub async fn collection_exists(&self) -> Result<bool, RagError> {
        let url = format!("{}/collections/{}", self.base_url, self.collection);
        let resp = self.client.get(&url).send().await?;
        match resp.status().as_u16() {
            200 => Ok(true),
            404 => Ok(false),
            s => Err(RagError::Store(format!(
                "collection_exists: unexpected HTTP {s}"
            ))),
        }
    }

    /// PUT /collections/{name} — create with Cosine/HNSW + payload indexes.
    pub async fn create_collection(&self, dims: usize) -> Result<(), RagError> {
        let url = format!("{}/collections/{}", self.base_url, self.collection);
        let body = json!({
            "vectors": {
                "size": dims,
                "distance": "Cosine",
                "on_disk": true,
                "hnsw_config": { "m": 16, "ef_construct": 200 }
            },
            "on_disk_payload": true
        });

        let resp = self.client.put(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(RagError::Store(format!("create_collection failed: {text}")));
        }

        // Payload indexes for fast URL/domain filtering.
        for (field, schema_type) in [("url", "keyword"), ("domain", "keyword")] {
            let idx_url = format!("{}/collections/{}/index", self.base_url, self.collection);
            let idx_body = json!({ "field_name": field, "field_schema": schema_type });
            let r = self.client.put(&idx_url).json(&idx_body).send().await?;
            if !r.status().is_success() {
                let text = r.text().await.unwrap_or_default();
                return Err(RagError::Store(format!(
                    "create_field_index({field}) failed: {text}"
                )));
            }
        }

        Ok(())
    }

    /// GET /collections/{name} and return the configured vector size.
    ///
    /// Used by `factory::build_vector_store` to validate that an existing
    /// collection's dimensions match the embed provider's output dimensions.
    pub(crate) async fn collection_vector_size(&self) -> Result<usize, RagError> {
        let endpoint = format!("{}/collections/{}", self.base_url, self.collection);
        let resp = self.client.get(&endpoint).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(RagError::Store(format!("collection_info failed: {text}")));
        }
        let info: CollectionInfoResponse = resp
            .json()
            .await
            .map_err(|e| RagError::Store(format!("collection_info parse failed: {e}")))?;
        info.result
            .map(|r| parse_collection_vector_size(r.config.params.vectors))
            .transpose()?
            .ok_or_else(|| RagError::Store("collection_info missing result".to_string()))
    }
}

fn parse_collection_vector_size(vectors: serde_json::Value) -> Result<usize, RagError> {
    if let Ok(config) = serde_json::from_value::<CollectionVectors>(vectors.clone()) {
        return Ok(config.size);
    }

    let named: CollectionNamedVectors = serde_json::from_value(json!({ "vectors": vectors }))
        .map_err(|e| RagError::Store(format!("collection_info parse failed: {e}")))?;

    let mut sizes = named.vectors.into_iter().map(|(_, config)| config.size);
    let first = sizes
        .next()
        .ok_or_else(|| RagError::Store("collection_info missing vectors".to_string()))?;

    if sizes.all(|size| size == first) {
        Ok(first)
    } else {
        Err(RagError::Store(
            "collection_info has named vectors with mismatched sizes".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_collection_vector_size;

    #[test]
    fn parses_named_vector_collection_size() {
        let payload = serde_json::json!({
            "default": { "size": 1024 },
            "title": { "size": 1024 }
        });

        let size = parse_collection_vector_size(payload).expect("named vectors should parse");
        assert_eq!(size, 1024);
    }

    #[test]
    fn rejects_mixed_named_vector_sizes() {
        let payload = serde_json::json!({
            "default": { "size": 1024 },
            "title": { "size": 768 }
        });

        let err = parse_collection_vector_size(payload).expect_err("mixed sizes should fail");
        assert!(
            err.to_string().contains("mismatched sizes"),
            "unexpected error: {err}"
        );
    }
}

#[async_trait]
impl VectorStore for QdrantStore {
    /// PUT /collections/{name}/points?wait=true in batches of 256.
    async fn upsert(&self, points: Vec<Point>) -> Result<(), RagError> {
        let url = format!(
            "{}/collections/{}/points?wait=true",
            self.base_url, self.collection
        );

        let qdrant_points: Vec<QdrantPoint> = points
            .iter()
            .map(|p| {
                let mut payload = std::collections::HashMap::new();
                payload.insert("text".into(), json!(p.payload.text));
                payload.insert("url".into(), json!(p.payload.url));
                payload.insert("domain".into(), json!(p.payload.domain));
                payload.insert("chunk_index".into(), json!(p.payload.chunk_index));
                payload.insert("total_chunks".into(), json!(p.payload.total_chunks));
                payload.insert("token_estimate".into(), json!(p.payload.token_estimate));
                QdrantPoint {
                    id: p.id.to_string(),
                    vector: p.vector.clone(),
                    payload,
                }
            })
            .collect();

        let resp = self
            .client
            .put(&url)
            .json(&UpsertRequest {
                points: qdrant_points,
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(RagError::Store(format!("upsert failed: {text}")));
        }

        Ok(())
    }

    /// POST /collections/{name}/points/delete?wait=true filtered by url payload.
    async fn delete_by_url(&self, url: &str) -> Result<(), RagError> {
        let normalized = normalize_url(url);
        let endpoint = format!(
            "{}/collections/{}/points/delete?wait=true",
            self.base_url, self.collection
        );
        let body = DeleteByFilterRequest {
            filter: json!({
                "must": [{ "key": "url", "match": { "value": normalized } }]
            }),
        };

        let resp = self.client.post(&endpoint).json(&body).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(RagError::Store(format!("delete_by_url failed: {text}")));
        }

        Ok(())
    }

    /// POST /collections/{name}/points/search
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchResult>, RagError> {
        let url = format!(
            "{}/collections/{}/points/search",
            self.base_url, self.collection
        );
        let body = SearchRequest {
            vector: vector.to_vec(),
            limit,
            with_payload: true,
            score_threshold: None,
        };

        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(RagError::Store(format!("search failed: {text}")));
        }

        let response: SearchResponse = resp.json().await?;

        let results = response
            .result
            .into_iter()
            .filter_map(|hit| {
                let payload = hit.payload?;
                let text = payload
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let url = payload
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                match (text, url) {
                    (Some(text), Some(url)) => {
                        let chunk_index = payload
                            .get("chunk_index")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        let token_estimate = payload
                            .get("token_estimate")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        Some(SearchResult {
                            text,
                            url,
                            score: hit.score,
                            chunk_index,
                            token_estimate,
                        })
                    }
                    _ => {
                        tracing::warn!(
                            "search hit dropped: missing required payload field (text or url) \
                             — possible schema mismatch or data corruption"
                        );
                        None
                    }
                }
            })
            .collect();

        Ok(results)
    }

    fn name(&self) -> &str {
        "qdrant"
    }
}

/// Strip fragment, trailing path slash, lowercase scheme+host (url crate already does the latter).
pub(crate) fn normalize_url(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(&path);
    parsed.to_string()
}
