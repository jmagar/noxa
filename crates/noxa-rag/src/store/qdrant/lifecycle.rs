use serde_json::json;

use crate::error::RagError;

use super::QdrantStore;
use super::http::{CollectionInfoResponse, parse_collection_vector_size};

const FILE_METADATA_INDEXES: &[(&str, &str)] = &[
    ("file_path", "keyword"),
    ("last_modified", "keyword"),
    ("git_branch", "keyword"),
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
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!(
                "create_collection failed: {preview}"
            )));
        }

        let indexes: &[(&str, &str)] = &[
            ("url", "keyword"),
            ("domain", "keyword"),
            ("source_type", "keyword"),
            ("language", "keyword"),
            ("file_path", "keyword"),
            ("last_modified", "keyword"),
            ("git_branch", "keyword"),
        ];
        let idx_url = format!("{}/collections/{}/index", self.base_url, self.collection);
        for (field, schema_type) in indexes {
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
        for (field, schema_type) in FILE_METADATA_INDEXES {
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
