use std::time::Duration;

use crate::browser::BrowserProfile;
use crate::client::batch::collect_ordered;
use crate::client::fetch::{is_pdf_content_type, pdf_to_extraction_result};
use crate::client::pool::{extract_host, pick_for_host};
use crate::client::{
    BatchExtractResult, BatchResult, ClientPool, FetchClient, FetchConfig, FetchResult,
};
use crate::error::FetchError;

#[test]
fn test_batch_result_struct() {
    let ok = BatchResult {
        url: "https://example.com".to_string(),
        result: Ok(FetchResult {
            html: "<html></html>".to_string(),
            status: 200,
            url: "https://example.com".to_string(),
            headers: http::HeaderMap::new(),
            elapsed: Duration::from_millis(42),
        }),
    };
    assert_eq!(ok.url, "https://example.com");
    assert!(ok.result.is_ok());
    assert_eq!(ok.result.unwrap().status, 200);

    let err = BatchResult {
        url: "https://bad.example".to_string(),
        result: Err(FetchError::InvalidUrl("bad url".into())),
    };
    assert!(err.result.is_err());
}

#[test]
fn test_batch_extract_result_struct() {
    let err = BatchExtractResult {
        url: "https://example.com".to_string(),
        result: Err(FetchError::BodyDecode("timeout".into())),
    };
    assert_eq!(err.url, "https://example.com");
    assert!(err.result.is_err());
}

#[tokio::test]
async fn test_batch_preserves_order() {
    let handles: Vec<tokio::task::JoinHandle<(usize, String)>> = vec![
        tokio::spawn(async { (2, "c".to_string()) }),
        tokio::spawn(async { (0, "a".to_string()) }),
        tokio::spawn(async { (1, "b".to_string()) }),
    ];

    let results = collect_ordered(handles, 3).await;
    assert_eq!(results, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn test_collect_ordered_handles_gaps() {
    let handles: Vec<tokio::task::JoinHandle<(usize, String)>> = vec![
        tokio::spawn(async { (0, "first".to_string()) }),
        tokio::spawn(async { (2, "third".to_string()) }),
    ];

    let results = collect_ordered(handles, 3).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], "first");
    assert_eq!(results[1], "third");
}

#[test]
fn test_is_pdf_content_type() {
    let mut headers = http::HeaderMap::new();
    headers.insert("content-type", "application/pdf".parse().unwrap());
    assert!(is_pdf_content_type(&headers));

    headers.insert(
        "content-type",
        "application/pdf; charset=utf-8".parse().unwrap(),
    );
    assert!(is_pdf_content_type(&headers));

    headers.insert("content-type", "Application/PDF".parse().unwrap());
    assert!(is_pdf_content_type(&headers));

    headers.insert("content-type", "text/html".parse().unwrap());
    assert!(!is_pdf_content_type(&headers));

    let empty = http::HeaderMap::new();
    assert!(!is_pdf_content_type(&empty));
}

#[test]
fn test_pdf_to_extraction_result() {
    let pdf = noxa_pdf::PdfResult {
        text: "Hello from PDF.".into(),
        page_count: 2,
        metadata: noxa_pdf::PdfMetadata {
            title: Some("My Doc".into()),
            author: Some("Author".into()),
            subject: Some("Testing".into()),
            creator: None,
        },
    };

    let result = pdf_to_extraction_result(&pdf, "https://example.com/doc.pdf");

    assert_eq!(result.metadata.title.as_deref(), Some("My Doc"));
    assert_eq!(result.metadata.author.as_deref(), Some("Author"));
    assert_eq!(result.metadata.description.as_deref(), Some("Testing"));
    assert_eq!(
        result.metadata.url.as_deref(),
        Some("https://example.com/doc.pdf")
    );
    assert!(result.content.markdown.contains("# My Doc"));
    assert!(result.content.markdown.contains("Hello from PDF."));
    assert_eq!(result.content.plain_text, "Hello from PDF.");
    assert!(result.content.links.is_empty());
    assert!(result.domain_data.is_none());
    assert!(result.metadata.word_count > 0);
}

#[test]
fn test_static_pool_no_proxy() {
    let config = FetchConfig::default();
    let client = FetchClient::new(config).unwrap();
    assert_eq!(client.proxy_pool_size(), 0);
}

#[test]
fn test_rotating_pool_prebuilds_clients() {
    let config = FetchConfig {
        proxy_pool: vec![
            "http://proxy1:8080".into(),
            "http://proxy2:8080".into(),
            "http://proxy3:8080".into(),
        ],
        ..Default::default()
    };
    let client = FetchClient::new(config).unwrap();
    assert_eq!(client.proxy_pool_size(), 3);
}

#[test]
fn test_pick_for_host_deterministic() {
    let config = FetchConfig {
        browser: BrowserProfile::Random,
        ..Default::default()
    };
    let client = FetchClient::new(config).unwrap();

    let clients = match &client.pool {
        ClientPool::Static { clients, .. } => clients,
        ClientPool::Rotating { clients } => clients,
    };

    let a1 = pick_for_host(clients, "example.com") as *const _;
    let a2 = pick_for_host(clients, "example.com") as *const _;
    let a3 = pick_for_host(clients, "example.com") as *const _;
    assert_eq!(a1, a2);
    assert_eq!(a2, a3);
}

#[test]
fn test_pick_for_host_distributes() {
    let config = FetchConfig {
        proxy_pool: (0..10).map(|i| format!("http://proxy{i}:8080")).collect(),
        ..Default::default()
    };
    let client = FetchClient::new(config).unwrap();

    let clients = match &client.pool {
        ClientPool::Static { clients, .. } | ClientPool::Rotating { clients } => clients,
    };

    let hosts = [
        "example.com",
        "google.com",
        "github.com",
        "rust-lang.org",
        "crates.io",
    ];

    let indices: Vec<usize> = hosts
        .iter()
        .map(|host| {
            let ptr = pick_for_host(clients, host) as *const _;
            clients.iter().position(|c| std::ptr::eq(c, ptr)).unwrap()
        })
        .collect();

    let unique: std::collections::HashSet<_> = indices.iter().collect();
    assert!(
        unique.len() >= 2,
        "expected host distribution across clients, got indices: {indices:?}"
    );
}

#[test]
fn test_extract_host() {
    assert_eq!(extract_host("https://example.com/path"), "example.com");
    assert_eq!(
        extract_host("https://sub.example.com:8080/foo"),
        "sub.example.com"
    );
    assert_eq!(extract_host("not-a-url"), "");
}

#[test]
fn test_default_config_has_empty_proxy_pool() {
    let config = FetchConfig::default();
    assert!(config.proxy_pool.is_empty());
    assert!(config.proxy.is_none());
}

#[test]
fn test_default_config_store_is_none() {
    let config = FetchConfig::default();
    assert!(config.store.is_none());
}

#[test]
fn test_fetch_config_clone_preserves_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = noxa_store::FilesystemContentStore::new(dir.path());
    let config = FetchConfig {
        store: Some(store),
        ..Default::default()
    };
    let cloned = config.clone();
    assert!(cloned.store.is_some());
}

#[test]
fn test_fetch_client_new_extracts_store_from_config() {
    let dir = tempfile::tempdir().unwrap();
    let store = noxa_store::FilesystemContentStore::new(dir.path());
    let config = FetchConfig {
        store: Some(store),
        ..Default::default()
    };
    let client = FetchClient::new(config).unwrap();
    assert!(client.store.is_some());
}

#[test]
fn test_fetch_client_new_without_store() {
    let config = FetchConfig::default();
    let client = FetchClient::new(config).unwrap();
    assert!(client.store.is_none());
}

/// Spin up a TCP listener that returns a fixed HTTP status for every request.
/// Handles multiple connections so retry loops get the same status each time.
#[cfg(test)]
async fn spawn_status_server(status: u16, body: &'static str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let response = format!(
        "HTTP/1.1 {status} Status\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );

    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                let resp = response.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let _ = socket.read(&mut buf).await;
                    let _ = socket.write_all(resp.as_bytes()).await;
                });
            }
        }
    });

    format!("http://{addr}/")
}

#[tokio::test]
async fn fetch_rejects_retryable_status_after_exhaustion() {
    // fetch() had a latent bug: on the last retry attempt with a retryable
    // status (429/5xx), it returned Ok(FetchResult{status:429}) because the
    // condition `attempt < delays.len()-1` was false and the result fell
    // through to `return Ok`. Fixed by removing that guard so the `continue`
    // on the last iteration exits the loop and Err(last_err) is returned.
    let url = spawn_status_server(429, "# 429 Too Many Requests\n\nnginx").await;
    let client = FetchClient::new(FetchConfig::default()).unwrap();
    let result = client.fetch(&url).await;
    assert!(
        result.is_err(),
        "fetch must return Err for 429 after retry exhaustion, got Ok"
    );
    assert!(
        matches!(result.unwrap_err(), FetchError::HttpStatus(429)),
        "expected HttpStatus(429)"
    );
}

#[tokio::test]
async fn fetch_and_extract_rejects_non_2xx_status() {
    let url = spawn_status_server(429, "# 429 Too Many Requests\n\nnginx").await;
    let client = FetchClient::new(FetchConfig::default()).unwrap();
    let result = client.fetch_and_extract(&url).await;
    assert!(
        result.is_err(),
        "fetch_and_extract must return Err for 429 responses, got Ok"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("429"),
        "error message should include status code, got: {msg}"
    );
}

#[tokio::test]
async fn fetch_and_extract_rejects_500_status() {
    let url = spawn_status_server(500, "<html><body>Internal Server Error</body></html>").await;
    let client = FetchClient::new(FetchConfig::default()).unwrap();
    let result = client.fetch_and_extract(&url).await;
    assert!(
        result.is_err(),
        "fetch_and_extract must return Err for 500 responses, got Ok"
    );
}

#[tokio::test]
async fn fetch_and_extract_rejects_404_status() {
    let url = spawn_status_server(404, "<html><body>Not Found</body></html>").await;
    let client = FetchClient::new(FetchConfig::default()).unwrap();
    let result = client.fetch_and_extract(&url).await;
    assert!(
        result.is_err(),
        "fetch_and_extract must return Err for 404 responses, got Ok"
    );
}
