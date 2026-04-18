/// HTTP client with browser TLS fingerprint impersonation.
/// Uses wreq (BoringSSL) for browser-grade TLS + HTTP/2 fingerprinting.
/// Supports single and batch operations with proxy rotation.
/// Automatically detects PDF responses and extracts text via noxa-pdf.
///
/// Two proxy modes:
/// - **Static**: single proxy (or none) baked into pre-built clients at construction.
/// - **Rotating**: pre-built pool of clients, each with a different proxy + fingerprint.
///   Same-host URLs are routed to the same client for HTTP/2 connection reuse.
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use noxa_pdf::PdfMode;
use rand::seq::SliceRandom;
use serde_json;
use tokio::sync::Semaphore;
use tracing::{debug, instrument, warn};

use crate::browser::{self, BrowserProfile, BrowserVariant};
use crate::error::FetchError;

const MAX_HTML_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_JSON_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_DOCUMENT_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_PDF_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

/// Configuration for building a [`FetchClient`].
#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub browser: BrowserProfile,
    /// Single proxy URL. Used when `proxy_pool` is empty.
    pub proxy: Option<String>,
    /// Pool of proxy URLs to rotate through.
    /// When non-empty, each proxy gets a pre-built client with a
    /// random browser fingerprint. Same-host URLs reuse the same client
    /// for HTTP/2 connection multiplexing.
    pub proxy_pool: Vec<String>,
    pub timeout: Duration,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub headers: HashMap<String, String>,
    pub pdf_mode: PdfMode,
    /// Optional content store. When set, every successful `fetch_and_extract`
    /// call automatically persists the result to disk. `None` (the default)
    /// disables persistence — existing call sites are unaffected.
    pub store: Option<noxa_store::FilesystemContentStore>,
    /// Optional operations log. When set, `map_site()` and other analytical
    /// operations append an entry to the domain `.operations.ndjson` log.
    pub ops_log: Option<Arc<noxa_store::FilesystemOperationsLog>>,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            browser: BrowserProfile::Chrome,
            proxy: None,
            proxy_pool: Vec::new(),
            timeout: Duration::from_secs(12),
            follow_redirects: true,
            max_redirects: 10,
            headers: HashMap::from([("Accept-Language".to_string(), "en-US,en;q=0.9".to_string())]),
            pdf_mode: PdfMode::default(),
            store: None,
            ops_log: None,
        }
    }
}

/// Result of a successful fetch.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub html: String,
    pub status: u16,
    /// Final URL after any redirects.
    pub url: String,
    pub headers: http::HeaderMap,
    pub elapsed: Duration,
}

/// Result for a single URL in a batch fetch operation.
#[derive(Debug)]
pub struct BatchResult {
    pub url: String,
    pub result: Result<FetchResult, FetchError>,
}

/// Result for a single URL in a batch fetch-and-extract operation.
#[derive(Debug)]
pub struct BatchExtractResult {
    pub url: String,
    pub result: Result<noxa_core::ExtractionResult, FetchError>,
}

/// Buffered response that owns its body. Provides the same sync API
/// that noxa-http::Response used to provide.
struct Response {
    status: u16,
    url: String,
    headers: http::HeaderMap,
    body: bytes::Bytes,
}

impl Response {
    /// Buffer a wreq response into an owned Response.
    async fn from_wreq(mut resp: wreq::Response) -> Result<Self, FetchError> {
        let status = resp.status().as_u16();
        let url = resp.uri().to_string();
        let headers = resp.headers().clone();
        let limit = response_body_limit(&headers, &url);

        if resp.content_length().is_some_and(|len| len > limit as u64) {
            return Err(FetchError::Limit(format!(
                "response body too large for {}: {} > {limit} bytes",
                response_kind(&headers, &url),
                resp.content_length().unwrap_or_default()
            )));
        }

        let mut body = bytes::BytesMut::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| FetchError::BodyDecode(e.to_string()))?
        {
            if body.len() + chunk.len() > limit {
                return Err(FetchError::Limit(format!(
                    "response body too large for {}: {} > {limit} bytes",
                    response_kind(&headers, &url),
                    body.len() + chunk.len()
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Self {
            status,
            url,
            headers,
            body: body.freeze(),
        })
    }

    fn status(&self) -> u16 {
        self.status
    }
    fn url(&self) -> &str {
        &self.url
    }
    fn headers(&self) -> &http::HeaderMap {
        &self.headers
    }
    fn body(&self) -> &[u8] {
        &self.body
    }
    fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    fn text(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }

    fn into_text(self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Internal representation of the client pool strategy.
enum ClientPool {
    /// Pre-built clients with a fixed proxy (or no proxy).
    /// Fingerprint rotation still works via the pool when `random` is true.
    Static {
        clients: Vec<wreq::Client>,
        random: bool,
    },
    /// Pre-built pool of clients, each with a different proxy + fingerprint.
    /// Requests pick a client deterministically by host for HTTP/2 connection reuse.
    Rotating { clients: Vec<wreq::Client> },
}

/// HTTP client with browser TLS + HTTP/2 fingerprinting via wreq.
///
/// Operates in two modes:
/// - **Static pool**: pre-built clients, optionally with fingerprint rotation.
///   Used when no `proxy_pool` is configured. Fast (no per-request construction).
/// - **Rotating pool**: pre-built clients, one per proxy in the pool.
///   Same-host URLs are routed to the same client for HTTP/2 multiplexing.
pub struct FetchClient {
    pool: ClientPool,
    pdf_mode: PdfMode,
    /// Optional content store for auto-persisting extraction results.
    store: Option<noxa_store::FilesystemContentStore>,
    /// Optional operations log for recording analytical operations (map, brand, etc.).
    ops_log: Option<Arc<noxa_store::FilesystemOperationsLog>>,
}

impl FetchClient {
    /// Build a new client from config.
    pub fn new(config: FetchConfig) -> Result<Self, FetchError> {
        let variants = collect_variants(&config.browser);
        let pdf_mode = config.pdf_mode.clone();
        // Extract store and ops_log before config is consumed by pool construction.
        let store = config.store.clone();
        let ops_log = config.ops_log.clone();

        let pool = if config.proxy_pool.is_empty() {
            let clients = variants
                .into_iter()
                .map(|v| {
                    crate::tls::build_client(
                        v,
                        config.timeout,
                        &config.headers,
                        config.proxy.as_deref(),
                        config.follow_redirects,
                        config.max_redirects,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            let random = matches!(config.browser, BrowserProfile::Random);
            debug!(
                count = clients.len(),
                random, "fetch client ready (static pool)"
            );

            ClientPool::Static { clients, random }
        } else {
            let mut rng = rand::thread_rng();

            let clients = config
                .proxy_pool
                .iter()
                .map(|proxy| {
                    let v = *variants.choose(&mut rng).unwrap();
                    crate::tls::build_client(
                        v,
                        config.timeout,
                        &config.headers,
                        Some(proxy),
                        config.follow_redirects,
                        config.max_redirects,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            debug!(
                clients = clients.len(),
                "fetch client ready (pre-built rotating pool)"
            );

            ClientPool::Rotating { clients }
        };

        Ok(Self {
            pool,
            pdf_mode,
            store,
            ops_log,
        })
    }

    /// Return the content store configured for this client, if any.
    pub fn store(&self) -> Option<&noxa_store::FilesystemContentStore> {
        self.store.as_ref()
    }

    /// Return the operations log configured for this client, if any.
    pub fn ops_log(&self) -> Option<&Arc<noxa_store::FilesystemOperationsLog>> {
        self.ops_log.as_ref()
    }

    /// Discover all sitemap URLs for the given site and append an ops-log entry.
    ///
    /// Delegates to [`crate::sitemap::discover`], then records an `Op::Map` entry
    /// in the domain-level `.operations.ndjson` when an ops log is configured.
    pub async fn map_site(&self, url: &str) -> Result<Vec<crate::SitemapEntry>, String> {
        let entries = crate::sitemap::discover(self, url)
            .await
            .map_err(|e| format!("sitemap discovery failed: {e}"))?;

        if let Some(ref log) = self.ops_log {
            let domain = noxa_store::domain_from_url(url);
            let entry = noxa_store::OperationEntry {
                op: noxa_store::Op::Map,
                at: chrono::Utc::now(),
                url: url.to_string(),
                input: serde_json::json!({}),
                output: serde_json::json!({
                    "count": entries.len(),
                    "urls": entries.iter().map(|e| e.url.clone()).collect::<Vec<_>>()
                }),
            };
            if let Err(e) = log.append(&domain, &entry).await {
                tracing::warn!("ops log append failed for map: {e}");
            }
        }

        Ok(entries)
    }

    /// Fetch a URL and return the raw HTML + response metadata.
    ///
    /// Automatically retries on transient failures (network errors, 5xx, 429)
    /// with exponential backoff: 0s, 1s (2 attempts total).
    #[instrument(skip(self), fields(url = %url))]
    pub async fn fetch(&self, url: &str) -> Result<FetchResult, FetchError> {
        let delays = [Duration::ZERO, Duration::from_secs(1)];
        let mut last_err = None;

        for (attempt, delay) in delays.iter().enumerate() {
            if attempt > 0 {
                tokio::time::sleep(*delay).await;
            }

            match self.fetch_once(url).await {
                Ok(result) => {
                    if is_retryable_status(result.status) && attempt < delays.len() - 1 {
                        warn!(
                            url,
                            status = result.status,
                            attempt = attempt + 1,
                            "retryable status, will retry"
                        );
                        last_err = Some(FetchError::Build(format!("HTTP {}", result.status)));
                        continue;
                    }
                    if attempt > 0 {
                        debug!(url, attempt = attempt + 1, "retry succeeded");
                    }
                    return Ok(result);
                }
                Err(e) => {
                    if !is_retryable_error(&e) || attempt == delays.len() - 1 {
                        return Err(e);
                    }
                    warn!(
                        url,
                        error = %e,
                        attempt = attempt + 1,
                        "transient error, will retry"
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| FetchError::Build("all retries exhausted".into())))
    }

    /// Single fetch attempt.
    async fn fetch_once(&self, url: &str) -> Result<FetchResult, FetchError> {
        let start = Instant::now();
        let client = self.pick_client(url);

        let resp = client.get(url).send().await?;
        let response = Response::from_wreq(resp).await?;
        response_to_result(response, start)
    }

    /// Fetch a URL then extract structured content.
    #[instrument(skip(self), fields(url = %url))]
    pub async fn fetch_and_extract(
        &self,
        url: &str,
    ) -> Result<noxa_core::ExtractionResult, FetchError> {
        self.fetch_and_extract_with_options(url, &noxa_core::ExtractionOptions::default())
            .await
    }

    /// Fetch a URL then extract structured content with custom extraction options.
    ///
    /// All extraction paths (HTML, Reddit JSON, PDF, document) converge through
    /// this method. A successful result is auto-persisted to the content store
    /// when `self.store` is set. Store failures are best-effort: they log a
    /// warning and do not affect the returned `Ok(result)`.
    #[instrument(skip(self, options), fields(url = %url))]
    pub async fn fetch_and_extract_with_options(
        &self,
        url: &str,
        options: &noxa_core::ExtractionOptions,
    ) -> Result<noxa_core::ExtractionResult, FetchError> {
        let mut result = self.fetch_and_extract_inner(url, options).await?;
        result.metadata.fetched_at = Some(Utc::now().to_rfc3339());

        // Auto-persist to content store (best-effort — failure never fails the
        // extraction). Covers all four extraction branches: HTML, Reddit JSON,
        // PDF, and document (DOCX/XLSX/CSV), since they all flow through here.
        if let Some(ref store) = self.store
            && let Err(e) = store.write(url, &result).await
        {
            warn!(url, error = %e, "content store write failed");
        }

        Ok(result)
    }

    /// Inner extraction logic. All branches return a single `Ok(result)` which
    /// is then handled by `fetch_and_extract_with_options` for `fetched_at`
    /// stamping and store persistence.
    async fn fetch_and_extract_inner(
        &self,
        url: &str,
        options: &noxa_core::ExtractionOptions,
    ) -> Result<noxa_core::ExtractionResult, FetchError> {
        if let Some(result) = self.try_fetch_reddit_json(url).await? {
            return Ok(result);
        }

        let start = Instant::now();
        let response = self.fetch_response_with_warmup(url).await?;
        self.extract_buffered_response(response, start, options)
    }

    /// Fetch multiple URLs concurrently with bounded parallelism.
    pub async fn fetch_batch(
        self: &Arc<Self>,
        urls: &[&str],
        concurrency: usize,
    ) -> Vec<BatchResult> {
        let concurrency = concurrency.max(1);
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let mut handles = Vec::with_capacity(urls.len());

        for (idx, url) in urls.iter().enumerate() {
            let permit = Arc::clone(&semaphore);
            let client = Arc::clone(self);
            let url = url.to_string();

            handles.push(tokio::spawn(async move {
                let _permit = permit.acquire().await.expect("semaphore closed");
                let result = client.fetch(&url).await;
                (idx, BatchResult { url, result })
            }));
        }

        collect_ordered(handles, urls.len()).await
    }

    /// Fetch and extract multiple URLs concurrently with bounded parallelism.
    pub async fn fetch_and_extract_batch(
        self: &Arc<Self>,
        urls: &[&str],
        concurrency: usize,
    ) -> Vec<BatchExtractResult> {
        self.fetch_and_extract_batch_with_options(
            urls,
            concurrency,
            &noxa_core::ExtractionOptions::default(),
        )
        .await
    }

    /// Fetch and extract multiple URLs concurrently with custom extraction options.
    pub async fn fetch_and_extract_batch_with_options(
        self: &Arc<Self>,
        urls: &[&str],
        concurrency: usize,
        options: &noxa_core::ExtractionOptions,
    ) -> Vec<BatchExtractResult> {
        let concurrency = concurrency.max(1);
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let mut handles = Vec::with_capacity(urls.len());

        for (idx, url) in urls.iter().enumerate() {
            let permit = Arc::clone(&semaphore);
            let client = Arc::clone(self);
            let url = url.to_string();
            let opts = options.clone();

            handles.push(tokio::spawn(async move {
                let _permit = permit.acquire().await.expect("semaphore closed");
                let result = client.fetch_and_extract_with_options(&url, &opts).await;
                (idx, BatchExtractResult { url, result })
            }));
        }

        collect_ordered(handles, urls.len()).await
    }

    /// Returns the number of proxies in the rotation pool, or 0 if static mode.
    pub fn proxy_pool_size(&self) -> usize {
        match &self.pool {
            ClientPool::Static { .. } => 0,
            ClientPool::Rotating { clients } => clients.len(),
        }
    }

    /// Pick a client from the pool for a given URL.
    fn pick_client(&self, url: &str) -> &wreq::Client {
        match &self.pool {
            ClientPool::Static { clients, random } => {
                if *random {
                    let host = extract_host(url);
                    pick_for_host(clients, &host)
                } else {
                    &clients[0]
                }
            }
            ClientPool::Rotating { clients } => {
                let host = extract_host(url);
                pick_for_host(clients, &host)
            }
        }
    }

    async fn try_fetch_reddit_json(
        &self,
        url: &str,
    ) -> Result<Option<noxa_core::ExtractionResult>, FetchError> {
        if !crate::reddit::is_reddit_url(url) {
            return Ok(None);
        }

        let json_url = crate::reddit::json_url(url);
        debug!("reddit detected, fetching {json_url}");

        let client = self.pick_client(url);
        let resp = client.get(&json_url).send().await?;
        let response = Response::from_wreq(resp).await?;
        if !response.is_success() {
            return Ok(None);
        }

        match crate::reddit::parse_reddit_json(response.body(), url) {
            Ok(result) => Ok(Some(result)),
            Err(e) => {
                warn!("reddit json fallback failed: {e}, falling back to HTML");
                Ok(None)
            }
        }
    }

    async fn fetch_response_with_warmup(&self, url: &str) -> Result<Response, FetchError> {
        let client = self.pick_client(url);
        let resp = client.get(url).send().await?;
        let mut response = Response::from_wreq(resp).await?;

        if is_challenge_response(&response)
            && let Some(homepage) = extract_homepage(url)
        {
            debug!("challenge detected, warming cookies via {homepage}");
            let _ = client.get(&homepage).send().await;
            let retry = client.get(url).send().await?;
            response = Response::from_wreq(retry).await?;
            debug!("retried after cookie warmup: status={}", response.status());
        }

        Ok(response)
    }

    fn extract_buffered_response(
        &self,
        response: Response,
        start: Instant,
        options: &noxa_core::ExtractionOptions,
    ) -> Result<noxa_core::ExtractionResult, FetchError> {
        let status = response.status();
        let final_url = response.url().to_string();
        let headers = response.headers().clone();

        if is_pdf_content_type(&headers) {
            debug!(status, "detected PDF response, using pdf extraction");
            let bytes = response.body();
            let elapsed = start.elapsed();
            debug!(
                status,
                bytes = bytes.len(),
                elapsed_ms = %elapsed.as_millis(),
                "PDF fetch complete"
            );

            let pdf_result = noxa_pdf::extract_pdf(bytes, self.pdf_mode.clone())?;
            return Ok(pdf_to_extraction_result(&pdf_result, &final_url));
        }

        if let Some(doc_type) = crate::document::is_document_content_type(&headers, &final_url) {
            debug!(status, doc_type = ?doc_type, "detected document response, extracting");
            let bytes = response.body();
            let elapsed = start.elapsed();
            debug!(
                status,
                bytes = bytes.len(),
                elapsed_ms = %elapsed.as_millis(),
                "document fetch complete"
            );

            let mut result = crate::document::extract_document(bytes, doc_type)?;
            result.metadata.url = Some(final_url);
            return Ok(result);
        }

        let html = response.into_text();
        let elapsed = start.elapsed();
        debug!(status, elapsed_ms = %elapsed.as_millis(), "fetch complete");

        if crate::linkedin::is_linkedin_post(&final_url) {
            if let Some(result) = crate::linkedin::extract_linkedin_post(&html, &final_url) {
                debug!("linkedin extraction succeeded");
                return Ok(result);
            }
            debug!("linkedin extraction failed, falling back to standard");
        }

        noxa_core::extract_with_options(&html, Some(&final_url), options).map_err(Into::into)
    }
}

/// Collect the browser variants to use based on the browser profile.
fn collect_variants(profile: &BrowserProfile) -> Vec<BrowserVariant> {
    match profile {
        BrowserProfile::Random => browser::all_variants(),
        BrowserProfile::Chrome => vec![browser::latest_chrome()],
        BrowserProfile::Firefox => vec![browser::latest_firefox()],
    }
}

/// Convert a buffered Response into a FetchResult.
fn response_to_result(response: Response, start: Instant) -> Result<FetchResult, FetchError> {
    let status = response.status();
    let final_url = response.url().to_string();
    let headers = response.headers().clone();
    let html = response.into_text();
    let elapsed = start.elapsed();

    debug!(status, elapsed_ms = %elapsed.as_millis(), "fetch complete");

    Ok(FetchResult {
        html,
        status,
        url: final_url,
        headers,
        elapsed,
    })
}

/// Extract the host from a URL, returning empty string on parse failure.
fn extract_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_default()
}

fn response_body_limit(headers: &http::HeaderMap, url: &str) -> usize {
    match response_kind(headers, url) {
        "pdf" => MAX_PDF_RESPONSE_BYTES,
        "document" => MAX_DOCUMENT_RESPONSE_BYTES,
        "json" => MAX_JSON_RESPONSE_BYTES,
        _ => MAX_HTML_RESPONSE_BYTES,
    }
}

fn response_kind(headers: &http::HeaderMap, url: &str) -> &'static str {
    if is_pdf_content_type(headers) {
        "pdf"
    } else if crate::document::is_document_content_type(headers, url).is_some() {
        "document"
    } else if is_json_content_type(headers, url) {
        "json"
    } else {
        "html"
    }
}

/// Pick a client deterministically based on a host string.
/// Same host always gets the same client, enabling HTTP/2 connection reuse.
fn pick_for_host<'a>(clients: &'a [wreq::Client], host: &str) -> &'a wreq::Client {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    host.hash(&mut hasher);
    let idx = (hasher.finish() as usize) % clients.len();
    &clients[idx]
}

/// Status codes worth retrying: server errors + rate limiting.
fn is_retryable_status(status: u16) -> bool {
    status == 429
        || status == 502
        || status == 503
        || status == 504
        || status == 520
        || status == 521
        || status == 522
        || status == 523
        || status == 524
}

/// Errors worth retrying: network/connection failures (not client errors).
fn is_retryable_error(err: &FetchError) -> bool {
    matches!(err, FetchError::Request(_) | FetchError::BodyDecode(_))
}

fn is_pdf_content_type(headers: &http::HeaderMap) -> bool {
    headers
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| {
            let mime = ct.split(';').next().unwrap_or("").trim();
            mime.eq_ignore_ascii_case("application/pdf")
        })
        .unwrap_or(false)
}

fn is_json_content_type(headers: &http::HeaderMap, url: &str) -> bool {
    headers
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| ct.split(';').next().unwrap_or("").trim())
        .is_some_and(|mime| mime.eq_ignore_ascii_case("application/json"))
        || url.ends_with(".json")
}

/// Detect if a response looks like a bot protection challenge page.
fn is_challenge_response(response: &Response) -> bool {
    let len = response.body().len();
    if len > 15_000 || len == 0 {
        return false;
    }

    let text = response.text();
    let lower = text.to_lowercase();

    if lower.contains("<title>challenge page</title>") {
        return true;
    }

    if lower.contains("bazadebezolkohpepadr") && len < 5_000 {
        return true;
    }

    false
}

/// Extract the homepage URL (scheme + host) from a full URL.
fn extract_homepage(url: &str) -> Option<String> {
    url::Url::parse(url).ok().map(|u| match u.port() {
        Some(port) => format!("{}://{}:{port}/", u.scheme(), u.host_str().unwrap_or("")),
        None => format!("{}://{}/", u.scheme(), u.host_str().unwrap_or("")),
    })
}

/// Convert a noxa-pdf PdfResult into a noxa-core ExtractionResult.
fn pdf_to_extraction_result(pdf: &noxa_pdf::PdfResult, url: &str) -> noxa_core::ExtractionResult {
    let markdown = noxa_pdf::to_markdown(pdf);
    let word_count = markdown.split_whitespace().count();

    noxa_core::ExtractionResult {
        metadata: noxa_core::Metadata {
            title: pdf.metadata.title.clone(),
            description: pdf.metadata.subject.clone(),
            author: pdf.metadata.author.clone(),
            published_date: None,
            language: None,
            url: Some(url.to_string()),
            site_name: None,
            image: None,
            favicon: None,
            word_count,
            content_hash: None,
            source_type: Some("web".into()),
            file_path: None,
            last_modified: None,
            is_truncated: None,
            technologies: Vec::new(),
            seed_url: None,
            crawl_depth: None,
            search_query: None,
            fetched_at: None,
        },
        content: noxa_core::Content {
            markdown,
            plain_text: pdf.text.clone(),
            links: Vec::new(),
            images: Vec::new(),
            code_blocks: Vec::new(),
            raw_html: None,
        },
        domain_data: None,
        structured_data: vec![],
    }
}

/// Collect spawned tasks and reorder results to match input order.
async fn collect_ordered<T>(
    handles: Vec<tokio::task::JoinHandle<(usize, T)>>,
    len: usize,
) -> Vec<T> {
    let mut slots: Vec<Option<T>> = (0..len).map(|_| None).collect();

    for handle in handles {
        match handle.await {
            Ok((idx, result)) => {
                slots[idx] = Some(result);
            }
            Err(e) => {
                warn!(error = %e, "batch task panicked");
            }
        }
    }

    slots.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn spawn_redirect_test_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");
        let base_url_for_task = base_url.clone();

        tokio::spawn(async move {
            for _ in 0..8 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let base_url = base_url_for_task.clone();
                tokio::spawn(async move {
                    let mut buf = [0_u8; 1024];
                    let read = stream.read(&mut buf).await.unwrap();
                    let request = String::from_utf8_lossy(&buf[..read]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/");

                    let (status_line, extra_headers, body) = match path {
                        "/start" => (
                            "302 Found",
                            format!("Location: {base_url}/final\r\n"),
                            "redirecting",
                        ),
                        "/hop1" => (
                            "302 Found",
                            format!("Location: {base_url}/hop2\r\n"),
                            "hop1",
                        ),
                        "/hop2" => (
                            "302 Found",
                            format!("Location: {base_url}/final\r\n"),
                            "hop2",
                        ),
                        "/final" => ("200 OK", String::new(), "ok"),
                        "/oversized" => (
                            "200 OK",
                            format!("Content-Length: {}\r\n", MAX_HTML_RESPONSE_BYTES as u64 + 1),
                            "tiny",
                        ),
                        _ => ("404 Not Found", String::new(), "missing"),
                    };

                    let content_length_header = if extra_headers.contains("Content-Length:") {
                        String::new()
                    } else {
                        format!("Content-Length: {}\r\n", body.len())
                    };
                    let response = format!(
                        "HTTP/1.1 {status_line}\r\nContent-Type: text/plain\r\n{content_length_header}Connection: close\r\n{extra_headers}\r\n{body}",
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });

        base_url
    }

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

    #[tokio::test]
    async fn test_fetch_batch_zero_concurrency_does_not_hang() {
        let base_url = spawn_redirect_test_server().await;
        let client = Arc::new(FetchClient::new(FetchConfig::default()).unwrap());
        let url = format!("{base_url}/final");
        let results = tokio::time::timeout(
            Duration::from_secs(1),
            client.fetch_batch(&[url.as_str()], 0),
        )
        .await
        .expect("fetch_batch should not hang with zero concurrency");

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_fetch_and_extract_batch_zero_concurrency_does_not_hang() {
        let base_url = spawn_redirect_test_server().await;
        let client = Arc::new(FetchClient::new(FetchConfig::default()).unwrap());
        let url = format!("{base_url}/final");
        let results = tokio::time::timeout(
            Duration::from_secs(1),
            client.fetch_and_extract_batch(&[url.as_str()], 0),
        )
        .await
        .expect("fetch_and_extract_batch should not hang with zero concurrency");

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_fetch_respects_follow_redirects_false() {
        let base_url = spawn_redirect_test_server().await;
        let client = FetchClient::new(FetchConfig {
            follow_redirects: false,
            ..Default::default()
        })
        .unwrap();

        let result = client.fetch(&format!("{base_url}/start")).await.unwrap();

        assert_eq!(result.status, 302);
        assert_eq!(result.url, format!("{base_url}/start"));
    }

    #[tokio::test]
    async fn test_fetch_respects_max_redirects() {
        let base_url = spawn_redirect_test_server().await;
        let client = FetchClient::new(FetchConfig {
            follow_redirects: true,
            max_redirects: 1,
            ..Default::default()
        })
        .unwrap();

        let err = client.fetch(&format!("{base_url}/hop1")).await.unwrap_err();

        assert!(
            matches!(&err, FetchError::Request(source) if source.is_redirect()),
            "expected redirect error, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_fetch_rejects_oversized_html_response() {
        let base_url = spawn_redirect_test_server().await;
        let client = FetchClient::new(FetchConfig::default()).unwrap();

        let err = client
            .fetch(&format!("{base_url}/oversized"))
            .await
            .expect_err("oversized HTML responses should be rejected");

        assert!(
            matches!(err, FetchError::Limit(_)),
            "expected size limit error, got {err:?}"
        );
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
    fn test_response_body_limit_matches_detected_kind() {
        let mut html_headers = http::HeaderMap::new();
        html_headers.insert("content-type", "text/html".parse().unwrap());
        assert_eq!(
            response_body_limit(&html_headers, "https://example.com"),
            MAX_HTML_RESPONSE_BYTES
        );

        let mut pdf_headers = http::HeaderMap::new();
        pdf_headers.insert("content-type", "application/pdf".parse().unwrap());
        assert_eq!(
            response_body_limit(&pdf_headers, "https://example.com/file.pdf"),
            MAX_PDF_RESPONSE_BYTES
        );

        let mut doc_headers = http::HeaderMap::new();
        doc_headers.insert(
            "content-type",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            response_body_limit(&doc_headers, "https://example.com/file.docx"),
            MAX_DOCUMENT_RESPONSE_BYTES
        );

        let mut json_headers = http::HeaderMap::new();
        json_headers.insert("content-type", "application/json".parse().unwrap());
        assert_eq!(
            response_body_limit(&json_headers, "https://example.com/data.json"),
            MAX_JSON_RESPONSE_BYTES
        );
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
    fn test_rotating_pool_is_host_sticky() {
        let config = FetchConfig {
            proxy_pool: (0..10).map(|i| format!("http://proxy{i}:8080")).collect(),
            ..Default::default()
        };
        let client = FetchClient::new(config).unwrap();

        let picks: HashSet<usize> = (0..8)
            .map(|_| {
                let ptr = client.pick_client("https://example.com/path") as *const _;
                match &client.pool {
                    ClientPool::Static { clients, .. } | ClientPool::Rotating { clients } => {
                        clients
                            .iter()
                            .position(|candidate| std::ptr::eq(candidate, ptr))
                            .unwrap()
                    }
                }
            })
            .collect();

        assert_eq!(
            picks.len(),
            1,
            "same host should always reuse the same rotating client, got picks {picks:?}"
        );
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
            .map(|h| {
                let ptr = pick_for_host(clients, h) as *const _;
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
    fn test_extract_homepage_preserves_explicit_port() {
        assert_eq!(
            extract_homepage("http://example.com:8443/challenge"),
            Some("http://example.com:8443/".to_string())
        );
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
}
