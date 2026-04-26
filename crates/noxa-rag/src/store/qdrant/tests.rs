use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use crate::config::{
    ChunkerConfig, EmbedProviderConfig, PipelineConfig, RagConfig, SourceConfig, VectorStoreConfig,
};
use crate::factory::build_vector_store;
use crate::store::{HashExistsResult, VectorStore};
use crate::types::SearchMetadataFilter;

use super::QdrantStore;
use super::http::parse_collection_vector_size;

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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

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
                } else if let Some((name, value)) = line.split_once(':')
                    && name.trim().eq_ignore_ascii_case("content-length")
                {
                    content_length = value.trim().parse().unwrap_or(0);
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

            let body = String::from_utf8_lossy(&buffer[header_end..header_end + content_length])
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
    assert!(err.to_string().contains("mismatched sizes"));
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
                        "git_branch": "main",
                        "email_to": ["team@example.com"],
                        "email_message_id": "msg@example.com",
                        "feed_url": "https://example.com/feed.xml",
                        "pptx_slide_count": 12,
                        "subtitle_source_file": "demo.mp4"
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
        hnsw_ef: None,
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
    assert_eq!(result.email_to, vec!["team@example.com".to_string()]);
    assert_eq!(result.email_message_id.as_deref(), Some("msg@example.com"));
    assert_eq!(
        result.feed_url.as_deref(),
        Some("https://example.com/feed.xml")
    );
    assert_eq!(result.pptx_slide_count, Some(12));
    assert_eq!(result.subtitle_source_file.as_deref(), Some("demo.mp4"));
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

#[tokio::test]
async fn build_vector_store_reconciles_existing_indexes_and_searches_with_metadata_filter() {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    let phase = Arc::new(AtomicUsize::new(0));
    let responder_phase = Arc::clone(&phase);
    let (base_url, requests, handle) =
        spawn_test_server(
            move |request| match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/collections/noxa-test") => {
                    let call = responder_phase.fetch_add(1, Ordering::SeqCst);
                    if call == 0 {
                        "{}".to_string()
                    } else {
                        serde_json::json!({
                            "result": {
                                "config": {
                                    "params": {
                                        "vectors": { "size": 1536 }
                                    }
                                }
                            }
                        })
                        .to_string()
                    }
                }
                ("PUT", "/collections/noxa-test/index") => "{}".to_string(),
                ("POST", "/collections/noxa-test/points/search") => serde_json::json!({
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
                .to_string(),
                other => panic!("unexpected request: {:?}", other),
            },
        )
        .await;

    let watch_dir = tempdir().expect("temp watch dir");
    let config = RagConfig {
        source: SourceConfig::FsWatcher {
            watch_dirs: vec![watch_dir.path().to_path_buf()],
            watch_dir: None,
            debounce_ms: 500,
        },
        embed_provider: EmbedProviderConfig::Tei {
            url: "http://tei.invalid".to_string(),
            model: "dummy".to_string(),
            local_path: Some(PathBuf::from("/tmp/tokenizer")),
            auth_token: None,
            query_instruction: None,
            dimensions: None,
        },
        vector_store: VectorStoreConfig::Qdrant {
            url: base_url.clone(),
            collection: "noxa-test".to_string(),
            api_key: None,
        },
        chunker: ChunkerConfig::default(),
        pipeline: PipelineConfig::default(),
        uuid_namespace: uuid::Uuid::nil(),
    };

    let store = build_vector_store(&config, 1536)
        .await
        .expect("build vector store");

    let filter = SearchMetadataFilter {
        file_path: Some("/tmp/report.md".to_string()),
        last_modified: None,
        git_branch: None,
        hnsw_ef: None,
    };
    let results = store
        .search(&[0.25, 0.75], 3, Some(&filter))
        .await
        .expect("search");

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

    let search_request = recorded
        .iter()
        .find(|req| req.method == "POST" && req.path == "/collections/noxa-test/points/search")
        .expect("search request recorded");
    let body: serde_json::Value = serde_json::from_str(&search_request.body).expect("json");
    assert_eq!(body["filter"]["must"][0]["key"], "file_path");
    assert_eq!(
        body["filter"]["must"][0]["match"]["value"],
        "/tmp/report.md"
    );

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_path.as_deref(), Some("/tmp/report.md"));
    assert_eq!(results[0].git_branch.as_deref(), Some("main"));
}

// ── helpers for status-aware test server ────────────────────────────────────

/// Like `spawn_test_server` but the responder returns `(status_code, body)` so
/// tests can simulate non-200 responses.
async fn spawn_test_server_with_status<F>(
    responder: F,
) -> (
    String,
    Arc<Mutex<Vec<RecordedRequest>>>,
    tokio::task::JoinHandle<()>,
)
where
    F: Fn(&RecordedRequest) -> (u16, String) + Send + Sync + 'static,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

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
                } else if let Some((name, value)) = line.split_once(':')
                    && name.trim().eq_ignore_ascii_case("content-length")
                {
                    content_length = value.trim().parse().unwrap_or(0);
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

            let body = String::from_utf8_lossy(&buffer[header_end..header_end + content_length])
                .to_string();
            let request = RecordedRequest { method, path, body };
            recorded.lock().unwrap().push(request.clone());

            let (status_code, response_body) = responder.as_ref()(&request);
            let status_text = match status_code {
                200 => "OK",
                400 => "Bad Request",
                500 => "Internal Server Error",
                503 => "Service Unavailable",
                _ => "Unknown",
            };
            let response = format!(
                "HTTP/1.1 {status_code} {status_text}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });

    (format!("http://{}", addr), requests, handle)
}

// ── regression tests: startup delta scan does not requeue on backend errors ─

/// When the Qdrant count endpoint returns a non-success HTTP status,
/// `url_with_hash_exists_checked` must return `BackendError`, NOT `NotIndexed`.
/// This prevents a degraded backend from triggering a full reindex storm.
#[tokio::test]
async fn url_with_hash_exists_checked_returns_backend_error_on_5xx() {
    let (base_url, _requests, handle) = spawn_test_server_with_status(|_req| {
        (500, r#"{"status":"error","error":"internal"}"#.to_string())
    })
    .await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let result = store
        .url_with_hash_exists_checked("https://example.com/doc.md", "abc123")
        .await;

    handle.abort();

    assert!(
        matches!(result, HashExistsResult::BackendError(_)),
        "expected BackendError on 500 response, got {result:?}"
    );
}

/// When the count endpoint returns 503 (Qdrant overloaded/restarting),
/// `url_with_hash_exists_checked` must return `BackendError`.
#[tokio::test]
async fn url_with_hash_exists_checked_returns_backend_error_on_503() {
    let (base_url, _requests, handle) =
        spawn_test_server_with_status(|_req| (503, "Service Unavailable".to_string())).await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let result = store
        .url_with_hash_exists_checked("https://example.com/doc.md", "deadbeef")
        .await;

    handle.abort();

    assert!(
        matches!(result, HashExistsResult::BackendError(_)),
        "expected BackendError on 503 response, got {result:?}"
    );
}

/// When the count endpoint returns a successful response with count > 0,
/// `url_with_hash_exists_checked` must return `Exists`.
#[tokio::test]
async fn url_with_hash_exists_checked_returns_exists_on_match() {
    let (base_url, _requests, handle) = spawn_test_server_with_status(|_req| {
        (
            200,
            serde_json::json!({ "result": { "count": 3 } }).to_string(),
        )
    })
    .await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let result = store
        .url_with_hash_exists_checked("https://example.com/doc.md", "abc123")
        .await;

    handle.abort();

    assert_eq!(
        result,
        HashExistsResult::Exists,
        "expected Exists when count > 0"
    );
}

/// When the count endpoint returns count == 0, result is `NotIndexed`.
#[tokio::test]
async fn url_with_hash_exists_checked_returns_not_indexed_on_zero_count() {
    let (base_url, _requests, handle) = spawn_test_server_with_status(|_req| {
        (
            200,
            serde_json::json!({ "result": { "count": 0 } }).to_string(),
        )
    })
    .await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let result = store
        .url_with_hash_exists_checked("https://example.com/doc.md", "abc123")
        .await;

    handle.abort();

    assert_eq!(
        result,
        HashExistsResult::NotIndexed,
        "expected NotIndexed when count == 0"
    );
}

/// Search responses with one valid hit and one malformed payload should:
/// - Return only the valid result (malformed hit is excluded).
/// - Increment the `decode_errors` counter on the store so the failure is observable.
#[tokio::test]
async fn search_logs_and_counts_malformed_payload_without_dropping_valid_results() {
    let (base_url, _requests, handle) = spawn_test_server(|_request| {
        serde_json::json!({
            "result": [
                {
                    // Valid hit — all required fields present.
                    "id": "aaaaaaaa-0000-0000-0000-000000000001",
                    "score": 0.85,
                    "payload": {
                        "text": "valid chunk",
                        "url": "https://example.com/page",
                        "chunk_index": 0,
                        "token_estimate": 42
                    }
                },
                {
                    // Malformed hit — missing required `text` and `url` fields.
                    "id": "aaaaaaaa-0000-0000-0000-000000000002",
                    "score": 0.72,
                    "payload": {
                        "chunk_index": 1,
                        "token_estimate": 10
                    }
                }
            ]
        })
        .to_string()
    })
    .await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let results = store
        .search(&[0.1, 0.9], 5, None)
        .await
        .expect("search should succeed despite malformed hit");

    handle.abort();

    // Only the valid hit should be returned.
    assert_eq!(results.len(), 1, "expected exactly 1 valid result");
    assert_eq!(results[0].text, "valid chunk");
    assert_eq!(results[0].url, "https://example.com/page");

    // The decode_errors counter must reflect the one malformed hit.
    let errors = store.decode_errors.load(Ordering::Relaxed);
    assert_eq!(
        errors, 1,
        "expected decode_errors counter to be 1 after one malformed payload"
    );
}

/// When a search hit has an empty payload object `{}`, it should be excluded
/// (missing required `text` and `url`) and counted in decode_errors.
#[tokio::test]
async fn search_counts_empty_payload_object_as_decode_error() {
    let (base_url, _requests, handle) = spawn_test_server(|_request| {
        serde_json::json!({
            "result": [
                {
                    "id": "cccccccc-0000-0000-0000-000000000001",
                    "score": 0.88,
                    "payload": {
                        "text": "good chunk",
                        "url": "https://example.com/good",
                        "chunk_index": 0,
                        "token_estimate": 50
                    }
                },
                {
                    // Empty payload object — missing required `text` and `url`.
                    "id": "cccccccc-0000-0000-0000-000000000002",
                    "score": 0.55,
                    "payload": {}
                }
            ]
        })
        .to_string()
    })
    .await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let results = store
        .search(&[0.5, 0.5], 5, None)
        .await
        .expect("search should succeed despite empty payload");

    handle.abort();

    assert_eq!(results.len(), 1, "expected exactly 1 valid result");
    assert_eq!(results[0].text, "good chunk");

    let errors = store.decode_errors.load(Ordering::Relaxed);
    assert_eq!(
        errors, 1,
        "expected decode_errors counter to be 1 after one empty payload"
    );
}

/// When a search hit has a null value for a required field (`text`), it should
/// be excluded and counted in decode_errors.
#[tokio::test]
async fn search_counts_null_required_field_as_decode_error() {
    let (base_url, _requests, handle) = spawn_test_server(|_request| {
        serde_json::json!({
            "result": [
                {
                    "id": "dddddddd-0000-0000-0000-000000000001",
                    "score": 0.92,
                    "payload": {
                        "text": "good chunk",
                        "url": "https://example.com/good",
                        "chunk_index": 0,
                        "token_estimate": 30
                    }
                },
                {
                    // `text` is null — required field, must not be None.
                    "id": "dddddddd-0000-0000-0000-000000000002",
                    "score": 0.65,
                    "payload": {
                        "text": null,
                        "url": "https://example.com/bad",
                        "chunk_index": 1,
                        "token_estimate": 20
                    }
                }
            ]
        })
        .to_string()
    })
    .await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let results = store
        .search(&[0.5, 0.5], 5, None)
        .await
        .expect("search should succeed despite null required field");

    handle.abort();

    assert_eq!(results.len(), 1, "expected exactly 1 valid result");
    assert_eq!(results[0].text, "good chunk");

    let errors = store.decode_errors.load(Ordering::Relaxed);
    assert_eq!(
        errors, 1,
        "expected decode_errors counter to be 1 after one null required field"
    );
}

/// When a search hit has a wrong type for a field that has no `#[serde(default)]`
/// override (e.g. `chunk_index` as a string instead of usize), serde should
/// reject the payload and it should be counted in decode_errors.
#[tokio::test]
async fn search_counts_wrong_field_type_as_decode_error() {
    let (base_url, _requests, handle) = spawn_test_server(|_request| {
        serde_json::json!({
            "result": [
                {
                    "id": "eeeeeeee-0000-0000-0000-000000000001",
                    "score": 0.87,
                    "payload": {
                        "text": "good chunk",
                        "url": "https://example.com/good",
                        "chunk_index": 0,
                        "token_estimate": 40
                    }
                },
                {
                    // `chunk_index` is a string — should be usize.
                    "id": "eeeeeeee-0000-0000-0000-000000000002",
                    "score": 0.71,
                    "payload": {
                        "text": "bad chunk",
                        "url": "https://example.com/bad",
                        "chunk_index": "not-a-number",
                        "token_estimate": 15
                    }
                }
            ]
        })
        .to_string()
    })
    .await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let results = store
        .search(&[0.5, 0.5], 5, None)
        .await
        .expect("search should succeed despite wrong field type");

    handle.abort();

    assert_eq!(results.len(), 1, "expected exactly 1 valid result");
    assert_eq!(results[0].text, "good chunk");

    let errors = store.decode_errors.load(Ordering::Relaxed);
    assert_eq!(
        errors, 1,
        "expected decode_errors counter to be 1 after one wrong-type field"
    );
}

/// When a search hit has no payload at all (None), it should be excluded from
/// results and counted in decode_errors — not silently vanish.
#[tokio::test]
async fn search_counts_missing_payload_as_decode_error() {
    let (base_url, _requests, handle) = spawn_test_server(|_request| {
        serde_json::json!({
            "result": [
                {
                    "id": "bbbbbbbb-0000-0000-0000-000000000001",
                    "score": 0.90,
                    "payload": {
                        "text": "good chunk",
                        "url": "https://example.com/good"
                    }
                },
                {
                    // Hit with no payload key at all.
                    "id": "bbbbbbbb-0000-0000-0000-000000000002",
                    "score": 0.60
                }
            ]
        })
        .to_string()
    })
    .await;

    let store = QdrantStore::new(&base_url, "noxa-test".to_string(), None, uuid::Uuid::nil())
        .expect("store");

    let results = store
        .search(&[0.5, 0.5], 5, None)
        .await
        .expect("search should succeed despite missing payload");

    handle.abort();

    assert_eq!(results.len(), 1, "expected exactly 1 valid result");
    assert_eq!(results[0].text, "good chunk");

    let errors = store.decode_errors.load(Ordering::Relaxed);
    assert_eq!(
        errors, 1,
        "expected decode_errors counter to be 1 after one missing payload"
    );
}
