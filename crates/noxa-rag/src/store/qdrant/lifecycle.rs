use serde_json::json;

use crate::error::RagError;

use super::QdrantStore;
use super::http::{CollectionInfoResponse, parse_collection_vector_size};

// KNOWLEDGE: last_modified is stored as RFC 3339 / ISO 8601 string (via chrono::to_rfc3339()).
// A "keyword" index only supports exact-match; range queries (gte/lte) on timestamps require
// "datetime". See pipeline/process.rs ~L236 and noxa-core/types.rs (Option<String>).
// content_hash is a hex digest string — "keyword" is correct (exact-match dedup / filter).
const BASE_COLLECTION_INDEXES: &[(&str, &str)] = &[
    ("url", "keyword"),
    ("content_hash", "keyword"),
    ("domain", "keyword"),
    ("source_type", "keyword"),
    ("language", "keyword"),
    ("file_path", "keyword"),
    ("last_modified", "datetime"), // was "keyword"; datetime enables range (gte/lte) queries
    ("git_branch", "keyword"),
    ("content_hash", "keyword"), // needed for dedup / content-hash filter queries (noxa-3fi.5)
    ("section_header", "keyword"), // enables section-level filtered search
];

impl QdrantStore {
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

    /// PUT /collections/{name} — create with Dot/HNSW + payload indexes.
    ///
    /// Dot (inner product) is used because TEI always outputs L2-normalized vectors
    /// (normalize=true). For unit vectors dot product == cosine similarity, skipping
    /// the redundant normalization step Qdrant applies for "Cosine" distance.
    /// Changing this on an existing collection requires deleting and recreating it.
    pub async fn create_collection(&self, dims: usize) -> Result<(), RagError> {
        let url = format!("{}/collections/{}", self.base_url, self.collection);
        let body = json!({
            "vectors": {
                "size": dims,
                "distance": "Dot",
                "on_disk": true,
                "hnsw_config": { "m": 16, "ef_construct": 200 }
            },
            "on_disk_payload": true,
            "quantization_config": {
                "scalar": {
                    "type": "int8",
                    "quantile": 0.99,
                    "always_ram": true
                }
            }
        });

        let resp = self.client.put(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!(
                "create_collection failed: {preview}"
            )));
        }

        let idx_url = format!("{}/collections/{}/index", self.base_url, self.collection);
        for (field, schema_type) in BASE_COLLECTION_INDEXES {
            let idx_body = json!({ "field_name": field, "field_schema": schema_type });
            let r = self.client.put(&idx_url).json(&idx_body).send().await?;
            if !r.status().is_success() {
                let text = r.text().await.unwrap_or_default();
                let preview: String = text.chars().take(512).collect();
                return Err(RagError::Store(format!(
                    "create_field_index({field}) failed: {preview}"
                )));
            }
        }

        Ok(())
    }

    /// Reconcile the landed file-metadata indexes on an already-existing collection.
    pub(crate) async fn reconcile_landed_file_metadata_indexes(&self) -> Result<(), RagError> {
        let idx_url = format!("{}/collections/{}/index", self.base_url, self.collection);
        for (field, schema_type) in BASE_COLLECTION_INDEXES
            .iter()
            .filter(|(field, _)| matches!(*field, "file_path" | "last_modified" | "git_branch" | "content_hash" | "section_header"))
        {
            let idx_body = json!({ "field_name": field, "field_schema": schema_type });
            let r = self.client.put(&idx_url).json(&idx_body).send().await?;
            if !r.status().is_success() {
                let text = r.text().await.unwrap_or_default();
                let preview: String = text.chars().take(512).collect();
                return Err(RagError::Store(format!(
                    "create_field_index({field}) failed: {preview}"
                )));
            }
        }

        Ok(())
    }

    /// GET /collections/{name} and return the configured vector size.
    pub(crate) async fn collection_vector_size(&self) -> Result<usize, RagError> {
        let endpoint = format!("{}/collections/{}", self.base_url, self.collection);
        let resp = self.client.get(&endpoint).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!(
                "collection_info failed: {preview}"
            )));
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
