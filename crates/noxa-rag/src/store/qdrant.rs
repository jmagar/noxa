use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

use crate::error::RagError;
use crate::store::VectorStore;
use crate::types::{Point, SearchMetadataFilter, SearchResult};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<serde_json::Value>,
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
    _uuid_namespace: uuid::Uuid,
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
            _uuid_namespace: uuid_namespace,
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
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!(
                "create_collection failed: {preview}"
            )));
        }

        // Payload indexes for fast filtering.
        //
        // Only index fields with real query callers today — speculative indexes waste
        // Qdrant disk and add index creation time on every startup.
        //
        // WARNING: Adding indexes to a populated collection is expensive (full
        // sequential scan, 30-120s per index for 100k points). For production
        // collections, prefer the shadow-collection migration strategy:
        //   1. Create 'noxa-v2' with all desired indexes
        //   2. Bulk-copy all points from old collection to noxa-v2
        //   3. Verify point counts match
        //   4. Update config to point at noxa-v2
        //   5. Delete old collection
        // For development / small collections (<10k points), direct creation is fine.
        //
        // PUT to /index is idempotent — Qdrant returns 200 if the index already exists,
        // so this loop is safe to run on every startup against an existing collection.
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

    /// GET /collections/{name} and return the configured vector size.
    ///
    /// Used by `factory::build_vector_store` to validate that an existing
    /// collection's dimensions match the embed provider's output dimensions.
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

fn parse_collection_vector_size(vectors: serde_json::Value) -> Result<usize, RagError> {
    if let Ok(config) = serde_json::from_value::<CollectionVectors>(vectors.clone()) {
        return Ok(config.size);
    }

    let named: CollectionNamedVectors = serde_json::from_value(json!({ "vectors": vectors }))
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

#[cfg(test)]
mod tests {
    use super::QdrantStore;
    use super::parse_collection_vector_size;
    use crate::store::VectorStore;
    use crate::types::SearchMetadataFilter;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

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

    #[derive(Clone, Debug)]
    struct RecordedRequest {
        method: String,
        path: String,
        body: String,
    }

    async fn spawn_test_server<F>(
        responder: F,
    ) -> (
        String,
        Arc<Mutex<Vec<RecordedRequest>>>,
        tokio::task::JoinHandle<()>,
    )
    where
        F: Fn(&RecordedRequest) -> String + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let responder = Arc::new(responder);

        let handle = tokio::spawn(async move {
            'connection: loop {
                let Ok((mut stream, _peer)) = listener.accept().await else {
                    break;
                };

                let mut buffer = Vec::new();
                let header_end = loop {
                    let mut chunk = [0u8; 1024];
                    let n = match stream.read(&mut chunk).await {
                        Ok(n) => n,
                        Err(_) => continue 'connection,
                    };
                    if n == 0 {
                        continue 'connection;
                    }
                    buffer.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = find_subslice(&buffer, b"\r\n\r\n") {
                        break pos + 4;
                    }
                };

                let headers = String::from_utf8_lossy(&buffer[..header_end]);
                let mut content_length = 0usize;
                let mut method = String::new();
                let mut path = String::new();
                for (i, line) in headers.lines().enumerate() {
                    if i == 0 {
                        let mut parts = line.split_whitespace();
                        method = parts.next().unwrap_or_default().to_string();
                        path = parts.next().unwrap_or_default().to_string();
                    } else if let Some((name, value)) = line.split_once(':') {
                        if name.trim().eq_ignore_ascii_case("content-length") {
                            content_length = value.trim().parse().unwrap_or(0);
                        }
                    }
                }

                while buffer.len() < header_end + content_length {
                    let mut chunk = [0u8; 1024];
                    let n = match stream.read(&mut chunk).await {
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    if n == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..n]);
                }

                let body =
                    String::from_utf8_lossy(&buffer[header_end..header_end + content_length])
                        .to_string();
                let request = RecordedRequest { method, path, body };
                recorded.lock().unwrap().push(request.clone());

                let response_body = responder.as_ref()(&request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        (format!("http://{}", addr), requests, handle)
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    #[tokio::test]
    async fn search_filters_by_landed_file_path_and_returns_it() {
        let (base_url, requests, handle) = spawn_test_server(|_request| {
            serde_json::json!({
                "result": [
                    {
                        "score": 0.91,
                        "payload": {
                            "text": "chunk text",
                            "url": "file:///tmp/report.md",
                            "chunk_index": 2,
                            "token_estimate": 123,
                            "file_path": "/tmp/report.md",
                            "last_modified": "2026-04-15T12:34:56Z",
                            "git_branch": "main"
                        }
                    }
                ]
            })
            .to_string()
        })
        .await;
        let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
            .expect("store");

        let filter = SearchMetadataFilter {
            file_path: Some("/tmp/report.md".to_string()),
            last_modified: None,
            git_branch: None,
        };

        let results = store
            .search(&[0.25, 0.75], 3, Some(&filter))
            .await
            .expect("search");

        handle.abort();

        let recorded = requests.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].method, "POST");
        assert_eq!(recorded[0].path, "/collections/noxa-test/points/search");

        let body: serde_json::Value = serde_json::from_str(&recorded[0].body).expect("json body");
        assert_eq!(body["filter"]["must"][0]["key"], "file_path");
        assert_eq!(
            body["filter"]["must"][0]["match"]["value"],
            "/tmp/report.md"
        );

        assert_eq!(results.len(), 1);
        let result = &results[0];
        assert_eq!(result.file_path.as_deref(), Some("/tmp/report.md"));
        assert_eq!(
            result.last_modified.as_deref(),
            Some("2026-04-15T12:34:56Z")
        );
        assert_eq!(result.git_branch.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn create_collection_indexes_only_landed_filter_fields() {
        let (base_url, requests, handle) = spawn_test_server(|_request| "{}".to_string()).await;
        let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
            .expect("store");

        store
            .create_collection(1536)
            .await
            .expect("create collection");

        handle.abort();

        let recorded = requests.lock().unwrap();
        let index_fields: Vec<String> = recorded
            .iter()
            .filter(|req| req.method == "PUT" && req.path.ends_with("/index"))
            .map(|req| {
                let body: serde_json::Value = serde_json::from_str(&req.body).expect("json");
                body["field_name"].as_str().unwrap_or_default().to_string()
            })
            .collect();

        assert!(index_fields.contains(&"file_path".to_string()));
        assert!(index_fields.contains(&"last_modified".to_string()));
        assert!(index_fields.contains(&"git_branch".to_string()));
        assert!(!index_fields.contains(&"external_id".to_string()));
        assert!(!index_fields.contains(&"platform_url".to_string()));
        assert!(!index_fields.contains(&"seed_url".to_string()));
        assert!(!index_fields.contains(&"search_query".to_string()));
        assert!(!index_fields.contains(&"crawl_depth".to_string()));
    }
}

#[async_trait]
impl VectorStore for QdrantStore {
    /// PUT /collections/{name}/points?wait=true. Returns the number of points written.
    async fn upsert(&self, points: Vec<Point>) -> Result<usize, RagError> {
        let n = points.len();
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
                // Extended metadata — only insert when present so payload stays compact.
                if let Some(v) = &p.payload.title {
                    payload.insert("title".into(), json!(v));
                }
                if let Some(v) = &p.payload.author {
                    payload.insert("author".into(), json!(v));
                }
                if let Some(v) = &p.payload.published_date {
                    payload.insert("published_date".into(), json!(v));
                }
                if let Some(v) = &p.payload.language {
                    payload.insert("language".into(), json!(v));
                }
                if let Some(v) = &p.payload.source_type {
                    payload.insert("source_type".into(), json!(v));
                }
                if let Some(v) = &p.payload.content_hash {
                    payload.insert("content_hash".into(), json!(v));
                }
                if !p.payload.technologies.is_empty() {
                    payload.insert("technologies".into(), json!(p.payload.technologies));
                }
                if let Some(v) = p.payload.is_truncated {
                    payload.insert("is_truncated".into(), json!(v));
                }
                if let Some(v) = &p.payload.file_path {
                    payload.insert("file_path".into(), json!(v));
                }
                if let Some(v) = &p.payload.last_modified {
                    payload.insert("last_modified".into(), json!(v));
                }
                if let Some(v) = &p.payload.external_id {
                    payload.insert("external_id".into(), json!(v));
                }
                if let Some(v) = &p.payload.platform_url {
                    payload.insert("platform_url".into(), json!(v));
                }
                if let Some(v) = &p.payload.seed_url {
                    payload.insert("seed_url".into(), json!(v));
                }
                if let Some(v) = &p.payload.search_query {
                    payload.insert("search_query".into(), json!(v));
                }
                if let Some(v) = p.payload.crawl_depth {
                    payload.insert("crawl_depth".into(), json!(v));
                }
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
            let preview: String = text.chars().take(512).collect();
            return Err(RagError::Store(format!("upsert failed: {preview}")));
        }

        Ok(n)
    }

    /// POST /collections/{name}/points/delete?wait=true filtered by url payload.
    ///
    /// Queries the stale point count before deleting and returns it.
    /// Qdrant's delete response does not include a deleted count, so we count first.
    async fn delete_by_url(&self, url: &str) -> Result<u64, RagError> {
        let normalized = normalize_url(url);

        // Count stale points before delete so callers can log reindex vs first-index.
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
            _ => 0, // non-fatal: best-effort count
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

    /// POST /collections/{name}/points/delete?wait=true — delete points for a URL
    /// whose IDs are NOT in `keep_ids`.
    ///
    /// Used for two-phase replace so that a transient upsert failure never empties
    /// the collection: new points are upserted first, then only stale points are
    /// removed.  If `keep_ids` is empty all points for the URL are deleted (same as
    /// `delete_by_url`).
    async fn delete_stale_by_url(
        &self,
        url: &str,
        keep_ids: &[uuid::Uuid],
    ) -> Result<u64, RagError> {
        let normalized = normalize_url(url);

        // Build filter: url == normalized AND id NOT IN keep_ids.
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

        // Count stale points before delete for logging.
        let count_endpoint = format!(
            "{}/collections/{}/points/count",
            self.base_url, self.collection
        );
        let stale_count: u64 = match self
            .client
            .post(&count_endpoint)
            .json(&json!({ "filter": filter, "exact": true }))
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

    /// POST /collections/{name}/points/search
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
        let filter = filter.and_then(|filter| {
            let mut must = Vec::new();
            if let Some(value) = &filter.file_path {
                must.push(json!({ "key": "file_path", "match": { "value": value } }));
            }
            if let Some(value) = &filter.last_modified {
                must.push(json!({ "key": "last_modified", "match": { "value": value } }));
            }
            if let Some(value) = &filter.git_branch {
                must.push(json!({ "key": "git_branch", "match": { "value": value } }));
            }
            if must.is_empty() {
                None
            } else {
                Some(json!({ "must": must }))
            }
        });
        let body = SearchRequest {
            vector: vector.to_vec(),
            limit,
            with_payload: true,
            score_threshold: None,
            filter,
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
                        let title = payload
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let author = payload
                            .get("author")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let published_date = payload
                            .get("published_date")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let language = payload
                            .get("language")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let source_type = payload
                            .get("source_type")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let content_hash = payload
                            .get("content_hash")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let technologies = payload
                            .get("technologies")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|t| t.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let file_path = payload
                            .get("file_path")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let last_modified = payload
                            .get("last_modified")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let git_branch = payload
                            .get("git_branch")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        Some(SearchResult {
                            text,
                            url,
                            score: hit.score,
                            chunk_index,
                            token_estimate,
                            title,
                            author,
                            published_date,
                            language,
                            source_type,
                            content_hash,
                            technologies,
                            file_path,
                            last_modified,
                            git_branch,
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

    /// GET /collections/{name} → total vectors_count.
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

    /// Check whether any point exists with both `url` == `url` AND `content_hash` == `hash`.
    ///
    /// Used by the startup delta scan so the daemon can skip re-indexing files whose
    /// content has not changed since the last run.  Returns `false` when `hash` is empty
    /// (no stored hash means we cannot skip).
    async fn url_with_hash_exists(&self, url: &str, hash: &str) -> Result<bool, RagError> {
        if hash.is_empty() {
            return Ok(false);
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

        let resp = self
            .client
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(5))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            let preview: String = text.chars().take(512).collect();
            tracing::warn!(
                status,
                url = %normalized,
                body = preview,
                "url_with_hash_exists count request failed — assuming not indexed"
            );
            return Ok(false);
        }

        let json: serde_json::Value = resp.json().await?;
        Ok(json["result"]["count"].as_u64().unwrap_or(0) > 0)
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
