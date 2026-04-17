use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::RagError;

#[derive(Deserialize)]
pub(crate) struct CollectionInfoResponse {
    pub result: Option<CollectionResult>,
}

#[derive(Deserialize)]
pub(crate) struct CollectionResult {
    pub config: CollectionConfig,
}

#[derive(Deserialize)]
pub(crate) struct CollectionConfig {
    pub params: CollectionParams,
}

#[derive(Deserialize)]
pub(crate) struct CollectionParams {
    pub vectors: serde_json::Value,
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
pub(crate) struct UpsertRequest {
    pub points: Vec<QdrantPoint>,
}

#[derive(Serialize)]
pub(crate) struct QdrantPoint {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: HashMap<String, serde_json::Value>,
}

#[derive(Serialize)]
pub(crate) struct DeleteByFilterRequest {
    pub filter: serde_json::Value,
}

#[derive(Serialize)]
pub(crate) struct SearchRequest {
    pub vector: Vec<f32>,
    pub limit: usize,
    pub with_payload: bool,
    pub score_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub(crate) struct SearchResponse {
    pub result: Vec<SearchHit>,
}

#[derive(Deserialize)]
pub(crate) struct SearchHit {
    pub score: f32,
    pub payload: Option<HashMap<String, serde_json::Value>>,
}

pub(crate) fn parse_collection_vector_size(vectors: serde_json::Value) -> Result<usize, RagError> {
    if let Ok(config) = serde_json::from_value::<CollectionVectors>(vectors.clone()) {
        return Ok(config.size);
    }

    let named: CollectionNamedVectors =
        serde_json::from_value(serde_json::json!({ "vectors": vectors }))
            .map_err(|e| RagError::Store(format!("collection_info parse failed: {e}")))?;

    let mut sizes = named.vectors.into_values().map(|config| config.size);
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
