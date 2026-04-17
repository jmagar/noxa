/// HTTP client with browser TLS fingerprint impersonation.
/// Uses wreq (BoringSSL) for browser-grade TLS + HTTP/2 fingerprinting.
/// Supports single and batch operations with proxy rotation.
/// Automatically detects PDF responses and extracts text via noxa-pdf.
mod batch;
mod fetch;
mod pool;
mod response;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use noxa_pdf::PdfMode;

use crate::browser::BrowserProfile;
use crate::error::FetchError;

#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub browser: BrowserProfile,
    pub proxy: Option<String>,
    pub proxy_pool: Vec<String>,
    pub timeout: Duration,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub headers: HashMap<String, String>,
    pub pdf_mode: PdfMode,
    pub store: Option<noxa_store::FilesystemContentStore>,
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

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub html: String,
    pub status: u16,
    pub url: String,
    pub headers: http::HeaderMap,
    pub elapsed: Duration,
}

#[derive(Debug)]
pub struct BatchResult {
    pub url: String,
    pub result: Result<FetchResult, FetchError>,
}

#[derive(Debug)]
pub struct BatchExtractResult {
    pub url: String,
    pub result: Result<noxa_core::ExtractionResult, FetchError>,
}

struct Response {
    status: u16,
    url: String,
    headers: http::HeaderMap,
    body: bytes::Bytes,
}

enum ClientPool {
    Static {
        clients: Vec<wreq::Client>,
        random: bool,
    },
    Rotating {
        clients: Vec<wreq::Client>,
    },
}

pub struct FetchClient {
    pool: ClientPool,
    pdf_mode: PdfMode,
    store: Option<noxa_store::FilesystemContentStore>,
    ops_log: Option<Arc<noxa_store::FilesystemOperationsLog>>,
}

impl FetchClient {
    pub fn store(&self) -> Option<&noxa_store::FilesystemContentStore> {
        self.store.as_ref()
    }

    pub fn ops_log(&self) -> Option<&Arc<noxa_store::FilesystemOperationsLog>> {
        self.ops_log.as_ref()
    }
}

#[cfg(test)]
mod tests;
