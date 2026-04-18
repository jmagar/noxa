use async_trait::async_trait;
use serde_json::json;

use crate::error::RagError;
use crate::store::{HashExistsResult, VectorStore};
use crate::types::{Point, SearchMetadataFilter, SearchResult};

use super::QdrantStore;
use super::http::{DeleteByFilterRequest, SearchRequest, SearchResponse, UpsertRequest};
use crate::url_util::normalize_url;
use super::payload::{point_to_qdrant_payload, search_filter, search_result_from_payload};

#[async_trait]
impl VectorStore for QdrantStore {
    /// PUT /collections/{name}/points?wait=true. Returns the number of points written.
    async fn upsert(&self, points: Vec<Point>) -> Result<usize, RagError> {
        let n = points.len();
        let url = format!(
            "{}/collections/{}/points?wait=true",
            self.base_url, self.collection
        );

        let qdrant_points = points
            .iter()
            .map(point_to_qdrant_payload)
            .collect::<Result<Vec<_>, _>>()?;

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
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!("upsert failed: {preview}")));
        }

        Ok(n)
    }

    /// POST /collections/{name}/points/delete?wait=true filtered by url payload.
    async fn delete_by_url(&self, url: &str) -> Result<u64, RagError> {
        let normalized = normalize_url(url);
        let count_endpoint = format!(
            "{}/collections/{}/points/count",
            self.base_url, self.collection
        );
        let count_body = json!({
            "filter": {
                "must": [{ "key": "url", "match": { "value": normalized } }]
            },
            "exact": true
        });
        let stale_count: u64 = match self
            .client
            .post(&count_endpoint)
            .json(&count_body)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v["result"]["count"].as_u64())
                .unwrap_or(0),
            _ => 0,
        };

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
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!("delete_by_url failed: {preview}")));
        }

        Ok(stale_count)
    }

    async fn delete_stale_by_url(
        &self,
        url: &str,
        keep_ids: &[uuid::Uuid],
    ) -> Result<u64, RagError> {
        let normalized = normalize_url(url);
        let filter = if keep_ids.is_empty() {
            json!({
                "must": [{ "key": "url", "match": { "value": normalized } }]
            })
        } else {
            let id_strs: Vec<String> = keep_ids.iter().map(|id| id.to_string()).collect();
            json!({
                "must": [{ "key": "url", "match": { "value": normalized } }],
                "must_not": [{ "has_id": id_strs }]
            })
        };

        let count_endpoint = format!(
            "{}/collections/{}/points/count",
            self.base_url, self.collection
        );
        let stale_count: u64 = match self
            .client
            .post(&count_endpoint)
            .json(&json!({ "filter": filter.clone(), "exact": true }))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v["result"]["count"].as_u64())
                .unwrap_or(0),
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                let preview: String = text.chars().take(512).collect();
                return Err(RagError::Store(format!(
                    "delete_stale_by_url count failed with HTTP {status}: {preview}"
                )));
            }
            Err(e) => {
                return Err(RagError::Store(format!(
                    "delete_stale_by_url count request failed: {e}"
                )));
            }
        };

        if stale_count == 0 {
            return Ok(0);
        }

        let endpoint = format!(
            "{}/collections/{}/points/delete?wait=true",
            self.base_url, self.collection
        );
        let resp = self
            .client
            .post(&endpoint)
            .json(&DeleteByFilterRequest { filter })
            .send()
            .await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!(
                "delete_stale_by_url failed: {preview}"
            )));
        }

        Ok(stale_count)
    }

    async fn search(
        &self,
        vector: &[f32],
        limit: usize,
        filter: Option<&SearchMetadataFilter>,
    ) -> Result<Vec<SearchResult>, RagError> {
        let url = format!(
            "{}/collections/{}/points/search",
            self.base_url, self.collection
        );
        let body = SearchRequest {
            vector: vector.to_vec(),
            limit,
            with_payload: true,
            score_threshold: None,
            filter: search_filter(filter),
        };

        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!("search failed: {preview}")));
        }

        let response: SearchResponse = resp.json().await?;
        let results = response
            .result
            .into_iter()
            .filter_map(|hit| {
                hit.payload
                    .and_then(|payload| search_result_from_payload(hit.score, payload))
            })
            .collect();

        Ok(results)
    }

    async fn collection_point_count(&self) -> Result<u64, RagError> {
        let endpoint = format!("{}/collections/{}", self.base_url, self.collection);
        let resp = self.client.get(&endpoint).send().await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!(
                "collection_point_count failed: {preview}"
            )));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| RagError::Store(format!("collection_point_count parse failed: {e}")))?;
        Ok(body["result"]["vectors_count"].as_u64().unwrap_or(0))
    }

    async fn url_with_hash_exists_checked(&self, url: &str, hash: &str) -> HashExistsResult {
        if hash.is_empty() {
            return HashExistsResult::NotIndexed;
        }
        let normalized = normalize_url(url);
        let endpoint = format!(
            "{}/collections/{}/points/count",
            self.base_url, self.collection
        );
        let body = serde_json::json!({
            "filter": {
                "must": [
                    { "key": "url", "match": { "value": normalized } },
                    { "key": "content_hash", "match": { "value": hash } }
                ]
            }
        });

        let resp = match self
            .client
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(5))
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    url = %normalized,
                    error = %e,
                    "url_with_hash_exists_checked: network error — treating as backend error"
                );
                return HashExistsResult::BackendError(format!("network error: {e}"));
            }
        };

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            let preview: String = text.chars().take(512).collect();
            tracing::warn!(
                status,
                url = %normalized,
                body = %preview,
                "url_with_hash_exists_checked: non-success HTTP status — treating as backend error"
            );
            return HashExistsResult::BackendError(format!(
                "HTTP {status}: {preview}"
            ));
        }

        let json: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    url = %normalized,
                    error = %e,
                    "url_with_hash_exists_checked: JSON parse error — treating as backend error"
                );
                return HashExistsResult::BackendError(format!("JSON parse error: {e}"));
            }
        };

        if json["result"]["count"].as_u64().unwrap_or(0) > 0 {
            HashExistsResult::Exists
        } else {
            HashExistsResult::NotIndexed
        }
    }

    fn name(&self) -> &str {
        "qdrant"
    }
}
