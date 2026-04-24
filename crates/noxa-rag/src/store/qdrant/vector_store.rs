use std::sync::atomic::Ordering;

use async_trait::async_trait;
use serde_json::json;
use tracing::warn;

use crate::error::RagError;
use crate::store::{HashExistsResult, VectorStore};
use crate::types::{Point, SearchMetadataFilter, SearchResult};

use super::QdrantStore;
use super::http::{DeleteByFilterRequest, QuantizationSearchParams, SearchParams, SearchRequest, SearchResponse, UpsertRequest};
use super::payload::{point_to_qdrant_payload, search_filter, search_result_from_payload};
use crate::url_util::normalize_url;

/// Error-check a Qdrant HTTP response: if the status is non-2xx, read the
/// body preview and return `RagError::Store("<method> failed: <preview>")`.
/// Otherwise return the response so the caller can continue reading its body.
///
/// `url_with_hash_exists_checked` and `url_with_file_hash_exists_checked`
/// intentionally use their own bespoke error handling (returning
/// `HashExistsResult::BackendError` instead of `RagError`) and do NOT go
/// through this helper.
async fn check_response(
    resp: reqwest::Response,
    method: &str,
) -> Result<reqwest::Response, RagError> {
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        let preview: String = text.chars().take(512).collect();
        return Err(RagError::Store(format!("{method} failed: {preview}")));
    }
    Ok(resp)
}

#[async_trait]
impl VectorStore for QdrantStore {
    /// PUT /collections/{name}/points?wait=false — idempotent with deterministic
    /// UUID v5 point IDs, so we do not block on WAL+index flush. This saves the
    /// ~30ms RTT per upsert that `wait=true` imposes. Delete operations still
    /// use `wait=true` because stale-chunk cleanup must observably complete
    /// before the URL-lock is released.
    async fn upsert(&self, points: Vec<Point>) -> Result<usize, RagError> {
        let n = points.len();
        let url = format!(
            "{}/collections/{}/points?wait=false",
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

        check_response(resp, "upsert").await?;

        Ok(n)
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
        check_response(resp, "delete_by_url").await?;

        Ok(())
    }

    async fn delete_stale_by_url(
        &self,
        url: &str,
        keep_ids: &[uuid::Uuid],
    ) -> Result<(), RagError> {
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
        check_response(resp, "delete_stale_by_url").await?;

        Ok(())
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
        // Knowledge: hnsw_ef=128 is below ef_construct=200 (Qdrant default collection
        // config) — good recall/latency balance for interactive queries. Caller can
        // override via SearchMetadataFilter::hnsw_ef; None falls back to this default.
        let hnsw_ef = filter
            .and_then(|f| f.hnsw_ef)
            .unwrap_or(128);
        let body = SearchRequest {
            vector: vector.to_vec(),
            limit,
            with_payload: true,
            score_threshold: None,
            filter: search_filter(filter),
            params: Some(SearchParams {
                hnsw_ef: Some(hnsw_ef),
                quantization: Some(QuantizationSearchParams {
                    ignore: false,
                    rescore: true,
                    oversampling: 2.0,
                }),
            }),
        };

        let resp = self.client.post(&url).json(&body).send().await?;
        let resp = check_response(resp, "search").await?;

        let response: SearchResponse = resp.json().await?;
        let mut decode_failures: u64 = 0;
        let results = response
            .result
            .into_iter()
            .filter_map(|hit| {
                let point_id = hit.id.as_ref().map(|v| v.to_string());
                match hit.payload {
                    None => {
                        decode_failures += 1;
                        self.decode_errors.fetch_add(1, Ordering::Relaxed);
                        warn!(
                            point_id = ?point_id,
                            "qdrant search hit has no payload; dropping from results"
                        );
                        None
                    }
                    Some(payload) => match search_result_from_payload(hit.score, payload) {
                        Ok(result) => Some(result),
                        Err(err) => {
                            decode_failures += 1;
                            self.decode_errors.fetch_add(1, Ordering::Relaxed);
                            warn!(
                                point_id = ?point_id,
                                error = %err,
                                "qdrant search payload decode failed; dropping from results"
                            );
                            None
                        }
                    },
                }
            })
            .collect();

        if decode_failures > 0 {
            warn!(
                count = decode_failures,
                collection = %self.collection,
                "qdrant search returned {decode_failures} point(s) with malformed or missing payloads"
            );
        }

        Ok(results)
    }

    async fn collection_point_count(&self) -> Result<u64, RagError> {
        let endpoint = format!("{}/collections/{}", self.base_url, self.collection);
        let resp = self.client.get(&endpoint).send().await?;
        let resp = check_response(resp, "collection_point_count").await?;
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
            return HashExistsResult::BackendError(format!("HTTP {status}: {preview}"));
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

        let Some(count) = json
            .get("result")
            .and_then(|result| result.get("count"))
            .and_then(|count| count.as_u64())
        else {
            tracing::warn!(
                url = %normalized,
                body = %json,
                "url_with_hash_exists_checked: missing numeric result.count — treating as backend error"
            );
            return HashExistsResult::BackendError(
                "missing or non-integer result.count in Qdrant response".to_string(),
            );
        };
        if count > 0 {
            HashExistsResult::Exists
        } else {
            HashExistsResult::NotIndexed
        }
    }

    async fn url_with_file_hash_exists_checked(&self, url: &str, file_hash: &str) -> HashExistsResult {
        if file_hash.is_empty() {
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
                    { "key": "file_hash", "match": { "value": file_hash } }
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
                    "url_with_file_hash_exists_checked: network error — treating as backend error"
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
                "url_with_file_hash_exists_checked: non-success HTTP — treating as backend error"
            );
            return HashExistsResult::BackendError(format!("HTTP {status}: {preview}"));
        }

        let json: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    url = %normalized,
                    error = %e,
                    "url_with_file_hash_exists_checked: JSON parse error — treating as backend error"
                );
                return HashExistsResult::BackendError(format!("JSON parse error: {e}"));
            }
        };

        let Some(count) = json
            .get("result")
            .and_then(|r| r.get("count"))
            .and_then(|c| c.as_u64())
        else {
            tracing::warn!(
                url = %normalized,
                body = %json,
                "url_with_file_hash_exists_checked: missing result.count — treating as backend error"
            );
            return HashExistsResult::BackendError(
                "missing or non-integer result.count in Qdrant response".to_string(),
            );
        };

        if count > 0 {
            HashExistsResult::Exists
        } else {
            HashExistsResult::NotIndexed
        }
    }

    fn name(&self) -> &str {
        "qdrant"
    }
}
