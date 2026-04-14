#![allow(dead_code)]
/// CLI entry point -- wires noxa-core and noxa-fetch into a single command.
/// All extraction and fetching logic lives in sibling crates; this is pure plumbing.
mod cloud;
mod config;
mod setup;
mod theme;

use theme::*;

use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{CommandFactory, FromArgMatches, Parser, ValueEnum};
use noxa_core::{
    ChangeStatus, ContentDiff, ExtractionOptions, ExtractionResult, Metadata, extract_with_options,
    to_llm_text,
};
use noxa_fetch::{
    BatchExtractResult, BrowserProfile, CrawlConfig, CrawlResult, Crawler, FetchClient,
    FetchConfig, FetchResult, PageResult, SitemapEntry,
};
use noxa_store::{FilesystemContentStore, FilesystemOperationsLog, Op, OperationEntry, domain_from_url, url_to_store_path};
use noxa_llm::LlmProvider;
use noxa_mcp;
use noxa_pdf::PdfMode;
use serde::Deserialize;
use tracing_subscriber::EnvFilter;

/// Known anti-bot challenge page titles (case-insensitive prefix match).
const ANTIBOT_TITLES: &[&str] = &[
    "just a moment",
    "attention required",
    "access denied",
    "checking your browser",
    "please wait",
    "one more step",
    "verify you are human",
    "bot verification",
    "security check",
    "ddos protection",
];

/// Detect why a page returned empty content.
enum EmptyReason {
    /// Anti-bot challenge page (Cloudflare, Akamai, etc.)
    Antibot,
    /// JS-only SPA that returns an empty shell without a browser
    JsRequired,
    /// Page has content — not empty
    None,
}

fn detect_empty(result: &ExtractionResult) -> EmptyReason {
    // Has real content — nothing to warn about
    if result.metadata.word_count > 50 || !result.content.markdown.is_empty() {
        return EmptyReason::None;
    }

    // Check for known anti-bot challenge titles
    if let Some(ref title) = result.metadata.title {
        let lower = title.to_lowercase();
        if ANTIBOT_TITLES.iter().any(|t| lower.starts_with(t)) {
            return EmptyReason::Antibot;
        }
    }

    // Empty content with no title or a generic SPA shell = JS-only site
    if result.metadata.word_count == 0 && result.content.links.is_empty() {
        return EmptyReason::JsRequired;
    }

    EmptyReason::None
}

fn warn_empty(url: &str, reason: &EmptyReason) {
    match reason {
        EmptyReason::Antibot => eprintln!(
            "{}\nThis site requires CAPTCHA solving or browser rendering.\n\
             Use the noxa Cloud API for automatic bypass: https://noxa.io/pricing",
            warning(&format!("Anti-bot protection detected on {url}"))
        ),
        EmptyReason::JsRequired => eprintln!(
            "{}\nThis site requires JavaScript rendering (SPA).\n\
             Use the noxa Cloud API for JS rendering: https://noxa.io/pricing",
            warning(&format!("No content extracted from {url}"))
        ),
        EmptyReason::None => {}
    }
}

#[derive(Parser)]
#[command(name = "noxa", about = "Extract web content for LLMs", version)]
struct Cli {
    /// Path to config.json (default: ./config.json, override with NOXA_CONFIG env var)
    #[arg(long, global = true)]
    config: Option<String>,

    /// URLs to fetch (multiple allowed)
    #[arg()]
    urls: Vec<String>,

    /// File with URLs (one per line)
    #[arg(long)]
    urls_file: Option<String>,

    /// Output format (markdown, json, text, llm, html)
    #[arg(short, long, default_value = "markdown")]
    format: OutputFormat,

    /// Browser to impersonate
    #[arg(short, long, default_value = "chrome")]
    browser: Browser,

    /// Proxy URL (http://user:pass@host:port or socks5://host:port)
    #[arg(short, long, env = "NOXA_PROXY")]
    proxy: Option<String>,

    /// File with proxies (host:port:user:pass, one per line). Rotates per request.
    #[arg(long, env = "NOXA_PROXY_FILE")]
    proxy_file: Option<String>,

    /// Request timeout in seconds
    #[arg(short, long, default_value = "30")]
    timeout: u64,

    /// Extract from local HTML file instead of fetching
    #[arg(long)]
    file: Option<String>,

    /// Read HTML from stdin
    #[arg(long)]
    stdin: bool,

    /// Include metadata in output (always included in JSON)
    #[arg(long)]
    metadata: bool,

    /// Output raw fetched HTML instead of extracting
    #[arg(long)]
    raw_html: bool,

    /// CSS selectors to include (comma-separated, e.g. "article,.content")
    #[arg(long)]
    include: Option<String>,

    /// CSS selectors to exclude (comma-separated, e.g. "nav,.sidebar,footer")
    #[arg(long)]
    exclude: Option<String>,

    /// Only extract main content (article/main element)
    #[arg(long)]
    only_main_content: bool,

    /// Custom headers (repeatable, e.g. -H "Cookie: foo=bar")
    #[arg(short = 'H', long = "header")]
    headers: Vec<String>,

    /// Cookie string (shorthand for -H "Cookie: ...")
    #[arg(long)]
    cookie: Option<String>,

    /// JSON cookie file (Chrome extension format: [{name, value, domain, ...}])
    #[arg(long)]
    cookie_file: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Compare against a previous JSON snapshot
    #[arg(long)]
    diff_with: Option<String>,

    /// Watch a URL for changes. Checks at the specified interval and reports diffs.
    #[arg(long)]
    watch: bool,

    /// Watch interval in seconds [default: 300]
    #[arg(long, default_value = "300")]
    watch_interval: u64,

    /// Command to run when changes are detected (receives diff JSON on stdin)
    #[arg(long)]
    on_change: Option<String>,

    /// Webhook URL: POST a JSON payload when an operation completes.
    /// Works with crawl, batch, watch (on change), and single URL modes.
    #[arg(long, env = "NOXA_WEBHOOK_URL")]
    webhook: Option<String>,

    /// Extract brand identity (colors, fonts, logo)
    #[arg(long)]
    brand: bool,

    // -- PDF options --
    /// PDF extraction mode: auto (error on empty) or fast (return whatever text is found)
    #[arg(long, default_value = "auto")]
    pdf_mode: PdfModeArg,

    // -- Crawl options --
    /// Enable recursive crawling of same-domain links. Runs in background by default.
    /// Use --wait to block and stream live progress.
    #[arg(long)]
    crawl: bool,

    /// Block and stream live crawl progress instead of running in background.
    #[arg(long)]
    wait: bool,

    /// Max crawl depth [default: 1]
    #[arg(long, default_value = "1")]
    depth: usize,

    /// Max pages to crawl [default: 20]
    #[arg(long, default_value = "20")]
    max_pages: usize,

    /// Max concurrent requests [default: 5]
    #[arg(long, default_value = "5")]
    concurrency: usize,

    /// Delay between requests in ms [default: 100]
    #[arg(long, default_value = "100")]
    delay: u64,

    /// Only crawl URLs matching this path prefix
    #[arg(long)]
    path_prefix: Option<String>,

    /// Glob patterns for crawl URL paths to include (comma-separated, e.g. "/api/*,/guides/**")
    #[arg(long)]
    include_paths: Option<String>,

    /// Glob patterns for crawl URL paths to exclude (comma-separated, e.g. "/changelog/*,/blog/*")
    #[arg(long)]
    exclude_paths: Option<String>,

    /// Path to save/resume crawl state. On Ctrl+C: saves progress. On start: resumes if file exists.
    #[arg(long)]
    crawl_state: Option<PathBuf>,

    /// Seed crawl frontier from sitemap discovery (robots.txt + /sitemap.xml)
    #[arg(long)]
    sitemap: bool,

    /// Discover URLs from sitemap and print them (one per line; JSON array with --format json)
    #[arg(long)]
    map: bool,

    // -- LLM options --
    /// Extract structured JSON using LLM (pass a JSON schema string or @file)
    #[arg(long)]
    extract_json: Option<String>,

    /// Extract using natural language prompt
    #[arg(long)]
    extract_prompt: Option<String>,

    /// Summarize content using LLM (optional: number of sentences, default 3)
    #[arg(long, num_args = 0..=1, default_missing_value = "3")]
    summarize: Option<usize>,

    /// Force a specific LLM provider (gemini, ollama, openai, anthropic)
    #[arg(long)]
    llm_provider: Option<String>,

    /// Override the LLM model name
    #[arg(long)]
    llm_model: Option<String>,

    /// Override the LLM base URL (Ollama or OpenAI-compatible)
    #[arg(long, env = "NOXA_LLM_BASE_URL")]
    llm_base_url: Option<String>,

    // -- Cloud API options --
    /// Noxa Cloud API key for automatic fallback on bot-protected or JS-rendered sites
    #[arg(long, env = "NOXA_API_KEY")]
    api_key: Option<String>,

    /// Force all requests through the cloud API (skip local extraction)
    #[arg(long)]
    cloud: bool,

    /// Cloud provider to use (e.g. "gcp", "aws")
    #[arg(long, env = "NOXA_CLOUD_PROVIDER")]
    cloud_provider: Option<String>,

    /// Cloud project ID
    #[arg(long, env = "NOXA_CLOUD_PROJECT")]
    cloud_project: Option<String>,

    /// Cloud zone or region
    #[arg(long, env = "NOXA_CLOUD_ZONE")]
    cloud_zone: Option<String>,

    /// Cloud cluster name
    #[arg(long, env = "NOXA_CLOUD_CLUSTER")]
    cloud_cluster: Option<String>,

    /// Path to cloud service account key file
    #[arg(long, env = "NOXA_CLOUD_SERVICE_ACCOUNT_KEY")]
    cloud_service_account_key: Option<String>,

    /// Disable cloud features
    #[arg(long)]
    cloud_disabled: bool,

    /// Run deep research on a topic via the cloud API. Requires --api-key.
    /// Saves full result (report + sources + findings) to a JSON file.
    #[arg(long)]
    research: Option<String>,

    /// Enable deep research mode (longer, more thorough report). Used with --research.
    #[arg(long)]
    deep: bool,

    /// Search via SearXNG (SEARXNG_URL) or noxa cloud (NOXA_API_KEY).
    #[arg(long)]
    search: Option<String>,

    /// Number of search results (1-50, default: 10).
    #[arg(long, default_value = "10")]
    num_results: u32,

    /// Print snippets only; skip scraping result URLs.
    #[arg(long)]
    no_scrape: bool,

    /// Disable automatic content store persistence (~/.noxa/content/).
    /// Also respected via the NOXA_NO_STORE environment variable.
    #[arg(long, env = "NOXA_NO_STORE")]
    no_store: bool,

    /// Concurrency for scraping search result URLs (default: 3).
    #[arg(long, default_value = "3")]
    num_scrape_concurrency: usize,

    /// Search through locally stored docs (~/.noxa/content/) with ripgrep.
    /// Falls back to built-in grep if rg is not installed.
    /// Example: noxa --grep "authentication"
    #[arg(long)]
    grep: Option<String>,

    /// List locally stored docs. No value = all domains. Pass a domain to list
    /// its individual docs with URL → path mapping.
    /// Example: noxa --list   or   noxa --list code.claude.com
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    list: Option<String>,

    /// Show status of a background crawl. Pass the domain or URL.
    /// Example: noxa --status code.claude.com
    #[arg(long)]
    status: Option<String>,

    /// Return a cached doc by exact URL or fuzzy query.
    /// Example: noxa --retrieve https://code.claude.com/docs/en/setup
    /// Example: noxa --retrieve "claude code setup"
    #[arg(long)]
    retrieve: Option<String>,

    /// Output directory: save each page to a separate file instead of stdout.
    /// Works with --crawl, batch (multiple URLs), and single URL mode.
    /// Filenames are derived from URL paths (e.g. /docs/api -> docs/api.md).
    #[arg(long)]
    output_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OutputFormat {
    Markdown,
    Json,
    Text,
    Llm,
    Html,
}

#[derive(Clone, Debug, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Browser {
    Chrome,
    Firefox,
    Random,
}

#[derive(Clone, Debug, ValueEnum, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PdfModeArg {
    /// Error if PDF has no extractable text (catches scanned PDFs)
    #[default]
    Auto,
    /// Return whatever text is found, even if empty
    Fast,
}

impl From<PdfModeArg> for PdfMode {
    fn from(arg: PdfModeArg) -> Self {
        match arg {
            PdfModeArg::Auto => PdfMode::Auto,
            PdfModeArg::Fast => PdfMode::Fast,
        }
    }
}

impl From<Browser> for BrowserProfile {
    fn from(b: Browser) -> Self {
        match b {
            Browser::Chrome => BrowserProfile::Chrome,
            Browser::Firefox => BrowserProfile::Firefox,
            Browser::Random => BrowserProfile::Random,
        }
    }
}

fn init_logging(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("noxa=debug")
    } else {
        EnvFilter::try_from_env("NOXA_LOG").unwrap_or_else(|_| EnvFilter::new("warn"))
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn init_mcp_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init()
        .ok();
}

/// Build an operations log from CLI flags.
///
/// Returns `None` when `--no-store` or `NOXA_NO_OPERATIONS_LOG` / `NOXA_NO_STORE` is set.
fn build_ops_log(cli: &Cli, resolved: &config::ResolvedConfig) -> Option<Arc<FilesystemOperationsLog>> {
    if cli.no_store
        || std::env::var("NOXA_NO_OPERATIONS_LOG").is_ok()
        || std::env::var("NOXA_NO_STORE").is_ok()
    {
        return None;
    }
    let root = content_store_root(resolved.output_dir.as_deref());
    Some(Arc::new(FilesystemOperationsLog::new(root)))
}

/// Append an entry to the operations log if one is configured.
///
/// Centralises the repeated `if let Some(ref log) … append … warn` pattern
/// that appears in every command handler.
async fn log_operation(
    ops_log: &Option<Arc<FilesystemOperationsLog>>,
    url: &str,
    op: Op,
    input: impl FnOnce() -> serde_json::Value,
    output: impl FnOnce() -> serde_json::Value,
) {
    if let Some(log) = ops_log {
        let domain = domain_from_url(url);
        let op_dbg = format!("{op:?}");
        let entry = OperationEntry {
            op,
            at: chrono::Utc::now(),
            url: url.to_string(),
            input: input(),
            output: output(),
        };
        if let Err(e) = log.append(&domain, &entry).await {
            tracing::warn!(op = %op_dbg, url, %domain, error = %e, "ops log append failed");
        }
    }
}

/// Build FetchConfig from CLI flags.
///
/// `--proxy` sets a single static proxy (no rotation).
/// `--proxy-file` loads a pool of proxies and rotates per-request.
/// `--proxy` takes priority: if both are set, only the single proxy is used.
fn build_fetch_config(cli: &Cli, resolved: &config::ResolvedConfig) -> FetchConfig {
    let (proxy, proxy_pool) = if cli.proxy.is_some() {
        (cli.proxy.clone(), Vec::new())
    } else if let Some(ref path) = cli.proxy_file {
        match noxa_fetch::parse_proxy_file(path) {
            Ok(pool) => (None, pool),
            Err(e) => {
                eprintln!("warning: {e}");
                (None, Vec::new())
            }
        }
    } else if std::path::Path::new("proxies.txt").exists() {
        // Auto-load proxies.txt from working directory if present
        match noxa_fetch::parse_proxy_file("proxies.txt") {
            Ok(pool) if !pool.is_empty() => {
                eprintln!("loaded {} proxies from proxies.txt", pool.len());
                (None, pool)
            }
            _ => (None, Vec::new()),
        }
    } else {
        (None, Vec::new())
    };

    let mut headers = std::collections::HashMap::from([(
        "Accept-Language".to_string(),
        "en-US,en;q=0.9".to_string(),
    )]);

    // Parse -H "Key: Value" flags
    for h in &cli.headers {
        if let Some((key, val)) = h.split_once(':') {
            headers.insert(key.trim().to_string(), val.trim().to_string());
        }
    }

    // --cookie shorthand
    if let Some(ref cookie) = cli.cookie {
        headers.insert("Cookie".to_string(), cookie.clone());
    }

    // --cookie-file: parse JSON array of {name, value, domain, ...}
    if let Some(ref path) = cli.cookie_file {
        match parse_cookie_file(path) {
            Ok(cookie_str) => {
                // Merge with existing cookies if --cookie was also provided
                if let Some(existing) = headers.get("Cookie") {
                    headers.insert("Cookie".to_string(), format!("{existing}; {cookie_str}"));
                } else {
                    headers.insert("Cookie".to_string(), cookie_str);
                }
            }
            Err(e) => {
                eprintln!("error: failed to parse cookie file: {e}");
                process::exit(1);
            }
        }
    }

    let store = if cli.no_store {
        None
    } else {
        let root = content_store_root(resolved.output_dir.as_deref());
        // Ensure the root directory exists so that ContentStore::resolve_path can
        // canonicalize it.  A fresh output_dir would otherwise cause all writes to
        // fail silently because std::fs::canonicalize returns an error for a
        // non-existent path.
        std::fs::create_dir_all(&root).ok();
        Some(FilesystemContentStore::new(root))
    };

    let ops_log = build_ops_log(cli, resolved);

    FetchConfig {
        browser: resolved.browser.clone().into(),
        proxy,
        proxy_pool,
        timeout: std::time::Duration::from_secs(resolved.timeout),
        pdf_mode: resolved.pdf_mode.clone().into(),
        headers,
        store,
        ops_log,
        ..Default::default()
    }
}

/// Parse a JSON cookie file (Chrome extension format) into a Cookie header string.
/// Supports: [{name, value, domain, path, secure, httpOnly, expirationDate, ...}]
fn parse_cookie_file(path: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let cookies: Vec<serde_json::Value> =
        serde_json::from_str(&content).map_err(|e| format!("invalid JSON: {e}"))?;

    let pairs: Vec<String> = cookies
        .iter()
        .filter_map(|c| {
            let name = c.get("name")?.as_str()?;
            let value = c.get("value")?.as_str()?;
            Some(format!("{name}={value}"))
        })
        .collect();

    if pairs.is_empty() {
        return Err("no cookies found in file".to_string());
    }

    Ok(pairs.join("; "))
}

fn build_extraction_options(resolved: &config::ResolvedConfig) -> ExtractionOptions {
    ExtractionOptions {
        include_selectors: resolved.include_selectors.clone(),
        exclude_selectors: resolved.exclude_selectors.clone(),
        only_main_content: resolved.only_main_content,
        include_raw_html: resolved.raw_html || matches!(resolved.format, OutputFormat::Html),
    }
}

// content_store_root is defined below (delegates to noxa_store::content_store_root).

/// Normalize a URL: prepend `https://` if no scheme is present.
fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// Derive a filename from a URL for `--output-dir`.
///
/// Strips the scheme/host, maps the path to a filesystem path, and appends
/// an extension matching the output format.
fn url_to_filename(raw_url: &str, format: &OutputFormat) -> String {
    let ext = match format {
        OutputFormat::Markdown | OutputFormat::Llm => "md",
        OutputFormat::Json => "json",
        OutputFormat::Text => "txt",
        OutputFormat::Html => "html",
    };

    let parsed = url::Url::parse(raw_url);
    let (host, path, query) = match &parsed {
        Ok(u) => (
            u.host_str().unwrap_or("unknown").to_string(),
            u.path().to_string(),
            u.query().map(String::from),
        ),
        Err(_) => (String::new(), String::new(), None),
    };

    let clean_host = host.strip_prefix("www.").unwrap_or(&host);
    let host_dir = clean_host.replace('.', "_");

    let mut stem = path.trim_matches('/').to_string();
    if stem.is_empty() {
        stem = format!("{host_dir}/index");
    } else {
        stem = format!("{host_dir}/{stem}");
    }

    // Append query params so /p?id=123 doesn't collide with /p?id=456
    if let Some(q) = query {
        stem = format!("{stem}_{q}");
    }

    // Sanitize: keep alphanumeric, dash, underscore, dot, slash
    let sanitized: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/') {
                c
            } else {
                '_'
            }
        })
        .collect();

    format!("{sanitized}.{ext}")
}

/// Returns true if the IP address is loopback, private, link-local, or otherwise reserved.
fn is_private_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_loopback()                               // 127.0.0.0/8
                || v4.is_unspecified()                     // 0.0.0.0
                || v4.is_link_local()                      // 169.254.0.0/16 (IMDS)
                || octets[0] == 10                         // 10.0.0.0/8
                || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31) // 172.16-31.x
                || (octets[0] == 192 && octets[1] == 168) // 192.168.0.0/16
                || (octets[0] == 100 && octets[1] >= 64 && octets[1] <= 127) // 100.64.0.0/10 (Tailscale/CGNAT)
        }
        std::net::IpAddr::V6(v6) => {
            // Unmap IPv4-mapped addresses (::ffff:x.x.x.x) and check as IPv4.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_ip(std::net::IpAddr::V4(v4));
            }
            let seg0 = v6.segments()[0];
            v6.is_loopback()                         // ::1
                || v6.is_unspecified()               // ::
                || v6.is_multicast()                 // ff00::/8
                || (seg0 & 0xffc0) == 0xfe80         // fe80::/10 link-local
                || (seg0 & 0xfe00) == 0xfc00         // fc00::/7  unique-local (ULA)
        }
    }
}

/// Validate that a URL is non-empty, has an http/https scheme, and does not target
/// private/loopback/reserved hosts (SSRF prevention).
///
/// For literal IP addresses the check is synchronous. For hostnames all A/AAAA
/// records are resolved and rejected if any resolves to a private range.
async fn validate_url(url: &str) -> Result<(), String> {
    validate_url_impl(url, |host| async move {
        tokio::net::lookup_host(host)
            .await
            .map(|iter| iter.collect::<Vec<_>>())
    })
    .await
}

/// Inner validation logic with an injectable resolver for deterministic testing.
/// `resolve` receives `"host:80"` and returns the resolved socket addresses.
async fn validate_url_impl<F, Fut>(url: &str, resolve: F) -> Result<(), String>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = std::io::Result<Vec<std::net::SocketAddr>>>,
{
    if url.is_empty() {
        return Err("Invalid URL: must not be empty".into());
    }
    let parsed = url::Url::parse(url)
        .map_err(|e| format!("Invalid URL: {e}. Must start with http:// or https://"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(format!(
            "Invalid URL: scheme '{}' not allowed, must start with http:// or https://",
            parsed.scheme()
        ));
    }
    let Some(host) = parsed.host_str() else {
        return Ok(());
    };
    let lower = host.to_lowercase();

    if lower == "localhost" || lower.ends_with(".localhost") {
        return Err(format!(
            "Invalid URL: host '{host}' is a private or reserved address"
        ));
    }

    if let Ok(ip) = lower.parse::<std::net::IpAddr>() {
        if is_private_ip(ip) {
            return Err(format!(
                "Invalid URL: host '{host}' is a private or reserved address"
            ));
        }
    } else {
        // Resolve hostname; reject if any resolved address is private (fail-closed).
        match resolve(format!("{host}:80")).await {
            Ok(addrs) => {
                for addr in addrs {
                    if is_private_ip(addr.ip()) {
                        return Err(format!(
                            "Invalid URL: host '{host}' resolves to a private or reserved address"
                        ));
                    }
                }
            }
            Err(e) => {
                return Err(format!(
                    "Invalid URL: could not resolve host '{host}': {e}"
                ));
            }
        }
    }
    Ok(())
}

/// Canonical content store root: `output_dir/.noxa/content` when configured,
/// otherwise `~/.noxa/content`. Every fetch path writes here so the same URL
/// always maps to the same file regardless of how it was fetched.
///
/// Exits the process with an error message if the home directory cannot be
/// determined. Using `"."` as a fallback scatters data unpredictably; hard
/// error is intentional.
fn content_store_root(output_dir: Option<&Path>) -> PathBuf {
    noxa_store::content_store_root(output_dir).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    })
}

/// Write extraction output to a file inside `dir`, creating parent dirs as needed.
fn write_to_file(dir: &Path, filename: &str, content: &str) -> Result<(), String> {
    // Reject path traversal and absolute paths before joining.
    if filename.split(['/', '\\']).any(|p| p == ".." || p == ".")
        || filename.starts_with('/')
        || filename.starts_with('\\')
        || filename.contains('\0')
    {
        return Err(format!("unsafe filename rejected: {filename}"));
    }
    let dest = dir.join(filename);
    // Lexical containment check (fast pre-filter).
    if !dest.starts_with(dir) {
        return Err(format!("filename escapes output directory: {filename}"));
    }

    // Ensure the output directory exists, then canonicalize it before any other I/O.
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("failed to create output directory: {e}"))?;
    let canonical_dir = std::fs::canonicalize(dir)
        .map_err(|e| format!("failed to resolve output directory: {e}"))?;

    // If `dest` already exists, check for symlinks before any further side-effects.
    // Use symlink_metadata (not exists/metadata) so dangling symlinks are also detected.
    if let Ok(meta) = dest.symlink_metadata() {
        if meta.file_type().is_symlink() {
            // Any symlink at the destination — dangling or not — is rejected.
            return Err(format!("filename escapes output directory via symlink: {filename}"));
        }
        // Regular file or directory: verify it's inside the canonical output dir.
        let canonical_dest = std::fs::canonicalize(&dest)
            .map_err(|e| format!("failed to resolve destination path: {e}"))?;
        if !canonical_dest.starts_with(&canonical_dir) {
            return Err(format!("filename escapes output directory: {filename}"));
        }
    }

    // Create parent directories only after symlink checks pass.
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create directory {}: {e}", parent.display()))?;
        // Re-verify after dir creation: intermediate symlinks in the path may resolve
        // to a location outside `dir`.
        let canonical_parent = std::fs::canonicalize(parent)
            .map_err(|e| format!("failed to resolve destination parent: {e}"))?;
        if !canonical_parent.starts_with(&canonical_dir) {
            return Err(format!("filename escapes output directory via symlink: {filename}"));
        }
    }

    std::fs::write(&dest, content)
        .map_err(|e| format!("failed to write {}: {e}", dest.display()))?;
    Ok(())
}

/// Print the save hint block — only called for single-URL saves, not batch/crawl.
fn print_save_hint(dest: &std::path::Path, content: &str) {
    let word_count = content.split_whitespace().count();
    let word_display = if word_count >= 1000 {
        format!("{:.1}k", word_count as f64 / 1000.0)
    } else {
        word_count.to_string()
    };
    let dir_part = dest.parent()
        .map(|p| format!("{}/", p.display()))
        .unwrap_or_default();
    let file_part = dest.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    eprintln!(
        "\n  {green}{bold}✓ saved{reset}  {dim}{dir_part}{reset}{bold}{cyan}{file_part}{reset}  {yellow}{bold}{word_display} words{reset}\n\
         \n\
         {dim}  grep{reset}     {cyan}noxa --grep {green}\"TERM\"{reset}\n\
         {dim}  context{reset}  {pink}noxa <url>{reset} {dim}(omit --output-dir to pipe straight to LLM){reset}\n",
    );
}


fn parse_http_url(url: &str) -> Result<url::Url, String> {
    if url.is_empty() {
        return Err("Invalid URL: must not be empty".into());
    }

    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        s => {
            return Err(format!(
                "Invalid URL: scheme '{s}' not allowed, must start with http:// or https://"
            ));
        }
    }
    parsed.host_str().ok_or("Invalid URL: no host")?;
    Ok(parsed)
}

/// Validate a URL provided by the operator (e.g. SEARXNG_URL). Only checks scheme and
/// host presence; does NOT reject private/loopback addresses (operator-trusted config).
fn validate_operator_url(url: &str) -> Result<(), String> {
    parse_http_url(url).map(|_| ())
}

/// Synchronous URL safety check (no DNS resolution) used for filtering search results.
/// Rejects private/loopback IP addresses but does not resolve hostnames.
/// NOTE: For stronger SSRF protection with DNS resolution, use the async `validate_url`.
fn validate_url_sync(url: &str) -> Result<(), String> {
    let parsed = parse_http_url(url)?;
    let host = parsed.host_str().ok_or("Invalid URL: no host")?;
    if matches!(host, "localhost" | "ip6-localhost" | "ip6-loopback")
        || host.ends_with(".localhost")
    {
        return Err(format!(
            "Invalid URL: host '{host}' is not a routable public address"
        ));
    }

    let private = match parsed.host() {
        Some(url::Host::Ipv4(addr)) => {
            let ip = std::net::IpAddr::V4(addr);
            ip.is_loopback() || ip.is_unspecified() || is_private_ip(ip)
        }
        Some(url::Host::Ipv6(addr)) => {
            let ip = std::net::IpAddr::V6(addr);
            ip.is_loopback() || ip.is_unspecified() || is_private_ip(ip)
        }
        Some(url::Host::Domain(_)) => false, // DNS not checked in sync path; use async validate_url for full protection
        None => true,
    };
    if private {
        return Err(format!(
            "Invalid URL: host '{host}' is not a routable public address"
        ));
    }

    Ok(())
}

fn clamp_search_scrape_concurrency(concurrency: usize) -> usize {
    concurrency.clamp(1, 20)
}


/// Get raw HTML from an extraction result, falling back to markdown if unavailable.
fn raw_html_or_markdown(result: &ExtractionResult) -> &str {
    result
        .content
        .raw_html
        .as_deref()
        .unwrap_or(&result.content.markdown)
}

/// Format an `ExtractionResult` into a string for the given output format.
fn format_output(result: &ExtractionResult, format: &OutputFormat, show_metadata: bool) -> String {
    match format {
        OutputFormat::Markdown => {
            let mut out = String::new();
            if show_metadata {
                out.push_str(&format_frontmatter(&result.metadata));
            }
            out.push_str(&result.content.markdown);
            if !result.structured_data.is_empty() {
                out.push_str("\n\n## Structured Data\n\n```json\n");
                out.push_str(
                    &serde_json::to_string_pretty(&result.structured_data).unwrap_or_default(),
                );
                out.push_str("\n```");
            }
            out
        }
        OutputFormat::Json => serde_json::to_string_pretty(result).expect("serialization failed"),
        OutputFormat::Text => result.content.plain_text.clone(),
        OutputFormat::Llm => to_llm_text(result, result.metadata.url.as_deref()),
        OutputFormat::Html => raw_html_or_markdown(result).to_string(),
    }
}

fn file_extension_for_format(format: &OutputFormat) -> &'static str {
    match format {
        OutputFormat::Markdown | OutputFormat::Llm => "md",
        OutputFormat::Json => "json",
        OutputFormat::Text => "txt",
        OutputFormat::Html => "html",
    }
}

fn default_search_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".noxa")
        .join("search")
}

fn format_cloud_output(resp: &serde_json::Value, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(resp).expect("serialization failed"),
        OutputFormat::Markdown => resp
            .get("content")
            .and_then(|c| c.get("markdown"))
            .and_then(|m| m.as_str())
            .or_else(|| resp.get("markdown").and_then(|m| m.as_str()))
            .map(str::to_string)
            .unwrap_or_else(|| serde_json::to_string_pretty(resp).expect("serialization failed")),
        OutputFormat::Text => resp
            .get("content")
            .and_then(|c| c.get("plain_text"))
            .and_then(|t| t.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format_cloud_output(resp, &OutputFormat::Markdown)),
        OutputFormat::Llm => resp
            .get("content")
            .and_then(|c| c.get("llm_text"))
            .and_then(|t| t.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format_cloud_output(resp, &OutputFormat::Markdown)),
        OutputFormat::Html => resp
            .get("content")
            .and_then(|c| c.get("raw_html"))
            .and_then(|h| h.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format_cloud_output(resp, &OutputFormat::Markdown)),
    }
}

fn format_diff_output(diff: &ContentDiff, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(diff).expect("serialization failed"),
        _ => {
            let mut out = String::new();
            out.push_str(&format!("Status: {:?}\n", diff.status));
            out.push_str(&format!("Word count delta: {:+}\n", diff.word_count_delta));

            if !diff.metadata_changes.is_empty() {
                out.push_str("\nMetadata changes:\n");
                for change in &diff.metadata_changes {
                    out.push_str(&format!(
                        "  {}: {} -> {}\n",
                        change.field,
                        change.old.as_deref().unwrap_or("(none)"),
                        change.new.as_deref().unwrap_or("(none)"),
                    ));
                }
            }

            if !diff.links_added.is_empty() {
                out.push_str("\nLinks added:\n");
                for link in &diff.links_added {
                    out.push_str(&format!("  + {} ({})\n", link.href, link.text));
                }
            }

            if !diff.links_removed.is_empty() {
                out.push_str("\nLinks removed:\n");
                for link in &diff.links_removed {
                    out.push_str(&format!("  - {} ({})\n", link.href, link.text));
                }
            }

            if let Some(ref text_diff) = diff.text_diff {
                out.push_str(&format!("\n{text_diff}\n"));
            }

            out
        }
    }
}

fn format_map_output(entries: &[SitemapEntry], format: &OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(entries).expect("serialization failed"),
        _ => {
            let mut out = String::new();
            for entry in entries {
                out.push_str(&entry.url);
                out.push('\n');
            }
            out
        }
    }
}

/// Collect all URLs from positional args + --urls-file, normalizing bare domains.
///
/// Returns `(url, optional_custom_filename)` pairs. Custom filenames come from
/// CSV-style lines in `--urls-file`: `url,filename`. Plain lines (no comma) get
/// `None` so the caller auto-generates the filename from the URL.
fn collect_urls(cli: &Cli) -> Result<Vec<(String, Option<String>)>, String> {
    let mut entries: Vec<(String, Option<String>)> =
        cli.urls.iter().map(|u| (normalize_url(u), None)).collect();

    if let Some(ref path) = cli.urls_file {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((url_part, name_part)) = trimmed.split_once(',') {
                let name = name_part.trim();
                let custom = if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                };
                entries.push((normalize_url(url_part.trim()), custom));
            } else {
                entries.push((normalize_url(trimmed), None));
            }
        }
    }

    Ok(entries)
}

/// Result that can be either a local extraction or a cloud API JSON response.
enum FetchOutput {
    Local(Box<ExtractionResult>),
    Cloud(serde_json::Value),
}

impl FetchOutput {
    /// Get the local ExtractionResult, or try to parse it from the cloud response.
    fn into_extraction(self) -> Result<ExtractionResult, String> {
        match self {
            FetchOutput::Local(r) => Ok(*r),
            FetchOutput::Cloud(resp) => {
                // Cloud response has an "extraction" field with the full ExtractionResult
                resp.get("extraction")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .or_else(|| serde_json::from_value(resp.clone()).ok())
                    .ok_or_else(|| "could not parse extraction from cloud response".to_string())
            }
        }
    }
}

/// Fetch a URL and extract content, handling PDF detection automatically.
/// Falls back to cloud API when bot protection or JS rendering is detected.
async fn fetch_and_extract(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
) -> Result<FetchOutput, String> {
    // Local sources: read and extract as HTML
    if cli.stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        let options = build_extraction_options(resolved);
        return extract_with_options(&buf, None, &options)
            .map(|r| FetchOutput::Local(Box::new(r)))
            .map_err(|e| format!("extraction error: {e}"));
    }

    if let Some(ref path) = cli.file {
        let html =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
        let options = build_extraction_options(resolved);
        return extract_with_options(&html, None, &options)
            .map(|r| FetchOutput::Local(Box::new(r)))
            .map_err(|e| format!("extraction error: {e}"));
    }

    let raw_url = cli
        .urls
        .first()
        .ok_or("no input provided -- pass a URL, --file, or --stdin")?;
    let url = normalize_url(raw_url);
    let url = url.as_str();

    let cloud_client = cloud::CloudClient::new(cli.api_key.as_deref());

    // --cloud: skip local, go straight to cloud API
    if cli.cloud {
        let c = cloud_client.ok_or("--cloud requires NOXA_API_KEY (set via env or --api-key)")?;
        let options = build_extraction_options(resolved);
        let format_str = match resolved.format {
            OutputFormat::Markdown => "markdown",
            OutputFormat::Json => "json",
            OutputFormat::Text => "text",
            OutputFormat::Llm => "llm",
            OutputFormat::Html => "html",
        };
        let resp = c
            .scrape(
                url,
                &[format_str],
                &options.include_selectors,
                &options.exclude_selectors,
                options.only_main_content,
            )
            .await?;
        return Ok(FetchOutput::Cloud(resp));
    }

    // Normal path: try local first
    let client = FetchClient::new(build_fetch_config(cli, resolved))
        .map_err(|e| format!("client error: {e}"))?;
    let options = build_extraction_options(resolved);
    let result = client
        .fetch_and_extract_with_options(url, &options)
        .await
        .map_err(|e| format!("fetch error: {e}"))?;

    // Check if we should fall back to cloud
    let reason = detect_empty(&result);
    if !matches!(reason, EmptyReason::None) {
        if let Some(ref c) = cloud_client {
            eprintln!("{}", info("falling back to cloud API..."));
            let format_str = match resolved.format {
                OutputFormat::Markdown => "markdown",
                OutputFormat::Json => "json",
                OutputFormat::Text => "text",
                OutputFormat::Llm => "llm",
                OutputFormat::Html => "html",
            };
            match c
                .scrape(
                    url,
                    &[format_str],
                    &options.include_selectors,
                    &options.exclude_selectors,
                    options.only_main_content,
                )
                .await
            {
                Ok(resp) => return Ok(FetchOutput::Cloud(resp)),
                Err(e) => {
                    eprintln!("{}", warning(&format!("cloud fallback failed: {e}")));
                    // Fall through to return the local result with a warning
                }
            }
        }
        warn_empty(url, &reason);
    }

    Ok(FetchOutput::Local(Box::new(result)))
}

/// Fetch raw HTML from a URL (no extraction). Used for --raw-html and brand extraction.
async fn fetch_html(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<FetchResult, String> {
    if cli.stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        return Ok(FetchResult {
            html: buf,
            url: String::new(),
            status: 200,
            headers: Default::default(),
            elapsed: Default::default(),
        });
    }

    if let Some(ref path) = cli.file {
        let html =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
        return Ok(FetchResult {
            html,
            url: String::new(),
            status: 200,
            headers: Default::default(),
            elapsed: Default::default(),
        });
    }

    let raw_url = cli
        .urls
        .first()
        .ok_or("no input provided -- pass a URL, --file, or --stdin")?;
    let url = normalize_url(raw_url);

    let client = FetchClient::new(build_fetch_config(cli, resolved))
        .map_err(|e| format!("client error: {e}"))?;
    client
        .fetch(&url)
        .await
        .map_err(|e| format!("fetch error: {e}"))
}

/// Fetch external stylesheets referenced in HTML and inject them as `<style>` blocks.
/// This allows brand extraction to see colors/fonts from external CSS files.
async fn enrich_html_with_stylesheets(html: &str, base_url: &str) -> String {
    let base = match url::Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return html.to_string(),
    };

    // Extract stylesheet hrefs from <link rel="stylesheet" href="...">
    let re = regex::Regex::new(
        r#"<link[^>]+rel=["']stylesheet["'][^>]+href=["']([^"']+)["']|<link[^>]+href=["']([^"']+)["'][^>]+rel=["']stylesheet["']"#
    ).unwrap();

    let hrefs: Vec<String> = re
        .captures_iter(html)
        .filter_map(|cap| {
            let href = cap.get(1).or(cap.get(2))?;
            Some(
                base.join(href.as_str())
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| href.as_str().to_string()),
            )
        })
        .take(10)
        .collect();

    if hrefs.is_empty() {
        return html.to_string();
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let mut extra_css = String::new();
    for href in &hrefs {
        if let Ok(resp) = client.get(href).send().await
            && resp.status().is_success()
            && let Ok(body) = resp.text().await
            && !body.trim_start().starts_with("<!")
            && body.len() < 2_000_000
        {
            extra_css.push_str("\n<style>\n");
            extra_css.push_str(&body);
            extra_css.push_str("\n</style>\n");
        }
    }

    if extra_css.is_empty() {
        return html.to_string();
    }

    if let Some(pos) = html.to_lowercase().find("</head>") {
        let mut enriched = String::with_capacity(html.len() + extra_css.len());
        enriched.push_str(&html[..pos]);
        enriched.push_str(&extra_css);
        enriched.push_str(&html[pos..]);
        enriched
    } else {
        format!("{extra_css}{html}")
    }
}

fn format_frontmatter(meta: &Metadata) -> String {
    let mut lines = vec!["---".to_string()];

    if let Some(title) = &meta.title {
        lines.push(format!("title: \"{title}\""));
    }
    if let Some(author) = &meta.author {
        lines.push(format!("author: \"{author}\""));
    }
    if let Some(date) = &meta.published_date {
        lines.push(format!("date: \"{date}\""));
    }
    if let Some(url) = &meta.url {
        lines.push(format!("source: \"{url}\""));
    }
    if meta.word_count > 0 {
        lines.push(format!("word_count: {}", meta.word_count));
    }

    lines.push("---".to_string());
    lines.push(String::new()); // blank line after frontmatter
    lines.join("\n")
}

fn print_output(result: &ExtractionResult, format: &OutputFormat, show_metadata: bool) {
    match format {
        OutputFormat::Markdown => {
            if show_metadata {
                print!("{}", format_frontmatter(&result.metadata));
            }
            println!("{}", result.content.markdown);
            if !result.structured_data.is_empty() {
                println!(
                    "\n## Structured Data\n\n```json\n{}\n```",
                    serde_json::to_string_pretty(&result.structured_data).unwrap_or_default()
                );
            }
        }
        OutputFormat::Json => {
            // serde_json::to_string_pretty won't fail on our types
            println!(
                "{}",
                serde_json::to_string_pretty(result).expect("serialization failed")
            );
        }
        OutputFormat::Text => {
            println!("{}", result.content.plain_text);
        }
        OutputFormat::Llm => {
            println!("{}", to_llm_text(result, result.metadata.url.as_deref()));
        }
        OutputFormat::Html => {
            println!("{}", raw_html_or_markdown(result));
        }
    }
}

/// Print cloud API response in the requested format.
fn print_cloud_output(resp: &serde_json::Value, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(resp).expect("serialization failed")
            );
        }
        OutputFormat::Markdown => {
            // Cloud response has content.markdown
            if let Some(md) = resp
                .get("content")
                .and_then(|c| c.get("markdown"))
                .and_then(|m| m.as_str())
            {
                println!("{md}");
            } else if let Some(md) = resp.get("markdown").and_then(|m| m.as_str()) {
                println!("{md}");
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(resp).expect("serialization failed")
                );
            }
        }
        OutputFormat::Text => {
            if let Some(txt) = resp
                .get("content")
                .and_then(|c| c.get("plain_text"))
                .and_then(|t| t.as_str())
            {
                println!("{txt}");
            } else {
                // Fallback to markdown or raw JSON
                print_cloud_output(resp, &OutputFormat::Markdown);
            }
        }
        OutputFormat::Llm => {
            if let Some(llm) = resp
                .get("content")
                .and_then(|c| c.get("llm_text"))
                .and_then(|t| t.as_str())
            {
                println!("{llm}");
            } else {
                print_cloud_output(resp, &OutputFormat::Markdown);
            }
        }
        OutputFormat::Html => {
            if let Some(html) = resp
                .get("content")
                .and_then(|c| c.get("raw_html"))
                .and_then(|h| h.as_str())
            {
                println!("{html}");
            } else {
                print_cloud_output(resp, &OutputFormat::Markdown);
            }
        }
    }
}

fn print_diff_output(diff: &ContentDiff, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(diff).expect("serialization failed")
            );
        }
        // For markdown/text/llm, show a human-readable summary
        _ => {
            println!("Status: {:?}", diff.status);
            println!("Word count delta: {:+}", diff.word_count_delta);

            if !diff.metadata_changes.is_empty() {
                println!("\nMetadata changes:");
                for change in &diff.metadata_changes {
                    println!(
                        "  {}: {} -> {}",
                        change.field,
                        change.old.as_deref().unwrap_or("(none)"),
                        change.new.as_deref().unwrap_or("(none)"),
                    );
                }
            }

            if !diff.links_added.is_empty() {
                println!("\nLinks added:");
                for link in &diff.links_added {
                    println!("  + {} ({})", link.href, link.text);
                }
            }

            if !diff.links_removed.is_empty() {
                println!("\nLinks removed:");
                for link in &diff.links_removed {
                    println!("  - {} ({})", link.href, link.text);
                }
            }

            if let Some(ref text_diff) = diff.text_diff {
                println!("\n{text_diff}");
            }
        }
    }
}

fn print_crawl_output(result: &CrawlResult, format: &OutputFormat, show_metadata: bool) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(result).expect("serialization failed")
            );
        }
        OutputFormat::Markdown => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                println!("---");
                println!("# Page: {}\n", page.url);
                if show_metadata {
                    print!("{}", format_frontmatter(&extraction.metadata));
                }
                println!("{}", extraction.content.markdown);
                println!();
            }
        }
        OutputFormat::Text => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                println!("---");
                println!("# Page: {}\n", page.url);
                println!("{}", extraction.content.plain_text);
                println!();
            }
        }
        OutputFormat::Llm => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                println!("---");
                println!("{}", to_llm_text(extraction, Some(page.url.as_str())));
                println!();
            }
        }
        OutputFormat::Html => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                println!("---");
                println!("<!-- Page: {} -->\n", page.url);
                println!("{}", raw_html_or_markdown(extraction));
                println!();
            }
        }
    }
}

fn print_batch_output(results: &[BatchExtractResult], format: &OutputFormat, show_metadata: bool) {
    match format {
        OutputFormat::Json => {
            // Build a JSON array of {url, result?, error?} objects
            let entries: Vec<serde_json::Value> = results
                .iter()
                .map(|r| match &r.result {
                    Ok(extraction) => serde_json::json!({
                        "url": r.url,
                        "result": extraction,
                    }),
                    Err(e) => serde_json::json!({
                        "url": r.url,
                        "error": e.to_string(),
                    }),
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&entries).expect("serialization failed")
            );
        }
        OutputFormat::Markdown => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        println!("---");
                        println!("# {}\n", r.url);
                        if show_metadata {
                            print!("{}", format_frontmatter(&extraction.metadata));
                        }
                        println!("{}", extraction.content.markdown);
                        println!();
                    }
                    Err(e) => {
                        eprintln!("error: {} -- {}", r.url, e);
                    }
                }
            }
        }
        OutputFormat::Text => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        println!("---");
                        println!("# {}\n", r.url);
                        println!("{}", extraction.content.plain_text);
                        println!();
                    }
                    Err(e) => {
                        eprintln!("error: {} -- {}", r.url, e);
                    }
                }
            }
        }
        OutputFormat::Llm => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        println!("---");
                        println!("{}", to_llm_text(extraction, Some(r.url.as_str())));
                        println!();
                    }
                    Err(e) => {
                        eprintln!("error: {} -- {}", r.url, e);
                    }
                }
            }
        }
        OutputFormat::Html => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        println!("---");
                        println!("<!-- {} -->\n", r.url);
                        println!("{}", raw_html_or_markdown(extraction));
                        println!();
                    }
                    Err(e) => {
                        eprintln!("error: {} -- {}", r.url, e);
                    }
                }
            }
        }
    }
}

fn print_map_output(entries: &[SitemapEntry], format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(entries).expect("serialization failed")
            );
        }
        _ => {
            for entry in entries {
                println!("{}", entry.url);
            }
        }
    }
}

/// Format a streaming progress line for a completed page.
fn format_progress(page: &PageResult, index: usize, max_pages: usize) -> String {
    let status = if page.error.is_some() { "ERR" } else { "OK " };
    let timing = format!("{}ms", page.elapsed.as_millis());
    let detail = if let Some(ref extraction) = page.extraction {
        format!(", {} words", extraction.metadata.word_count)
    } else if let Some(ref err) = page.error {
        format!(" ({err})")
    } else {
        String::new()
    };
    format!(
        "[{index}/{max_pages}] {status} {} ({timing}{detail})",
        page.url
    )
}

fn crawl_status_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".noxa")
        .join("crawls")
}

fn crawl_status_path(url: &str) -> std::path::PathBuf {
    let key = url_to_store_path(url)
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    crawl_status_dir().join(format!("{key}.json"))
}

fn write_crawl_status(
    path: &std::path::Path,
    url: &str,
    pages_done: usize,
    pages_ok: usize,
    pages_errors: usize,
    max_pages: usize,
    last_url: Option<&str>,
    done: bool,
    elapsed_secs: f64,
    docs_dir: &str,
    excluded: usize,
    total_words: usize,
) {
    let pid = std::process::id();
    let payload = serde_json::json!({
        "url": url,
        "pid": pid,
        "pages_done": pages_done,
        "pages_ok": pages_ok,
        "pages_errors": pages_errors,
        "max_pages": max_pages,
        "last_url": last_url,
        "done": done,
        "elapsed_secs": (elapsed_secs * 10.0).round() / 10.0,
        "docs_dir": docs_dir,
        "excluded": excluded,
        "total_words": total_words,
    });
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec_pretty(&payload) {
        if std::fs::write(&tmp, &bytes).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

fn run_retrieve(query: &str, store_root: std::path::PathBuf) {

    if !store_root.exists() {
        eprintln!("{dim}no local docs — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --crawl <url>{reset}");
        return;
    }

    // Exact URL lookup
    let looks_like_url = query.starts_with("http://")
        || query.starts_with("https://")
        || (!query.contains(' ') && query.contains('.'));

    if looks_like_url {
        let url = if query.starts_with("http") {
            query.to_string()
        } else {
            format!("https://{query}")
        };
        let md_path = store_root
            .join(url_to_store_path(&url))
            .with_extension("md");
        if md_path.exists() {
            match std::fs::read_to_string(&md_path) {
                Ok(content) => {
                    eprintln!("{dim}retrieved{reset} {pink}{}{reset}\n", md_path.display());
                    print!("{content}");
                    return;
                }
                Err(e) => { eprintln!("error reading {}: {e}", md_path.display()); return; }
            }
        }
        eprintln!("{yellow}not cached:{reset} {bold}{url}{reset}");
        eprintln!("{dim}run:{reset} {cyan}noxa {url}{reset} {dim}to fetch and store it{reset}");
        return;
    }

    // Fuzzy query — score docs by how many query words appear in URL + title
    let terms: Vec<String> = query.split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();

    let mut scored: Vec<(usize, String, std::path::PathBuf)> = Vec::new();
    collect_docs(&store_root, &store_root, &mut {
        let mut docs = Vec::new();
        docs
    });

    // Walk and score inline to avoid a second pass
    let mut all_docs: Vec<(String, std::path::PathBuf)> = Vec::new();
    collect_docs(&store_root, &store_root, &mut all_docs);

    for (url, path) in all_docs {
        let url_lower = url.to_lowercase();
        // Also pull title from JSON sidecar if present.
        // Support both new Sidecar envelope and legacy raw ExtractionResult.
        let title_lower = path.with_extension("json")
            .pipe(|jp| std::fs::read_to_string(jp).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| {
                // New sidecar: nested under current.metadata.title.
                if let Some(t) = v["current"]["metadata"]["title"].as_str() {
                    return Some(t.to_lowercase());
                }
                // Legacy format: metadata.title at top level.
                v["metadata"]["title"].as_str().map(|t| t.to_lowercase())
            })
            .unwrap_or_default();
        let score = terms.iter().filter(|t| url_lower.contains(t.as_str()) || title_lower.contains(t.as_str())).count();
        if score > 0 {
            scored.push((score, url, path));
        }
    }

    if scored.is_empty() {
        eprintln!("{yellow}no cached docs match:{reset} {bold}\"{query}\"{reset}");
        eprintln!("{dim}try:{reset} {cyan}noxa --search \"{query}\"{reset} {dim}to find and cache them{reset}");
        return;
    }

    // Sort by score desc; on tie prefer shorter URL (more specific)
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())));

    if scored.len() > 1 {
        eprintln!("{dim}best match ({}/{} docs scored):{reset}\n", scored.len(), scored.len());
        for (score, url, _) in scored.iter().take(5) {
            eprintln!("  {dim}{score} match(es){reset}  {cyan}{url}{reset}");
        }
        eprintln!();
    }

    let (_, best_url, best_path) = &scored[0];
    match std::fs::read_to_string(best_path) {
        Ok(content) => {
            eprintln!("{dim}retrieved{reset} {pink}{best_url}{reset}\n");
            print!("{content}");
        }
        Err(e) => eprintln!("error reading {}: {e}", best_path.display()),
    }
}

// Helper to pipe a value through a function (avoids temp var for path transform)
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R where F: FnOnce(Self) -> R { f(self) }
}
impl<T> Pipe for T {}

fn run_status(domain: &str) {

    // Accept domain with or without scheme, strip www.
    let key_input = domain
        .strip_prefix("https://").or_else(|| domain.strip_prefix("http://"))
        .unwrap_or(domain)
        .strip_prefix("www.").unwrap_or(domain);
    let key: String = key_input.chars().map(|c| {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }
    }).collect();

    let status_path = crawl_status_dir().join(format!("{key}.json"));
    if !status_path.exists() {
        eprintln!("{dim}no crawl status found for{reset} {bold}{domain}{reset}");
        eprintln!("{dim}run:{reset} {cyan}noxa --crawl {domain}{reset}");
        return;
    }

    let content = match std::fs::read_to_string(&status_path) {
        Ok(s) => s,
        Err(e) => { eprintln!("error reading status: {e}"); return; }
    };
    let v: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => { eprintln!("error parsing status: {e}"); return; }
    };

    let url          = v["url"].as_str().unwrap_or(domain);
    let pages_done   = v["pages_done"].as_u64().unwrap_or(0);
    let pages_ok     = v["pages_ok"].as_u64().unwrap_or(0);
    let pages_errors = v["pages_errors"].as_u64().unwrap_or(0);
    let max_pages    = v["max_pages"].as_u64().unwrap_or(0);
    let last_url     = v["last_url"].as_str().unwrap_or("");
    let done_flag    = v["done"].as_bool().unwrap_or(false);
    let elapsed      = v["elapsed_secs"].as_f64().unwrap_or(0.0);
    let pid          = v["pid"].as_u64().unwrap_or(0);
    let docs_dir     = v["docs_dir"].as_str().unwrap_or("");
    let excluded     = v["excluded"].as_u64().unwrap_or(0);
    let total_words  = v["total_words"].as_u64().unwrap_or(0);

    // Check if process is still alive
    let running = !done_flag && pid > 0 && {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    };

    // Also treat as done if pages_done hit the limit and process is no longer running
    let done = done_flag || (!running && max_pages > 0 && pages_done >= max_pages);

    let state_label = if done {
        format!("{green}{bold}done{reset}")
    } else if running {
        format!("{yellow}{bold}running{reset}")
    } else {
        format!("{dim}stopped{reset}")
    };

    let pages_display = if done {
        format!("{bold}{pages_done}{reset}")
    } else {
        format!("{bold}{pages_done}{reset}{dim}/{max_pages}{reset}")
    };

    eprintln!("\n  {dim}crawl{reset}  {bold}{cyan}{url}{reset}  {state_label}\n");
    if !docs_dir.is_empty() {
        eprintln!("  {dim}docs{reset}     {pink}{docs_dir}{reset}");
    }
    let words_suffix = if done && total_words > 0 {
        let s = if total_words >= 1_000_000 {
            format!("  {dim}~{:.1}M words{reset}", total_words as f64 / 1_000_000.0)
        } else if total_words >= 1_000 {
            format!("  {dim}~{}k words{reset}", total_words / 1_000)
        } else {
            format!("  {dim}{total_words} words{reset}")
        };
        s
    } else { String::new() };
    let excl_suffix = if done && excluded > 0 {
        format!("  {dim}{excluded} excluded{reset}")
    } else { String::new() };
    eprintln!("  {dim}pages{reset}    {pages_display}  {green}{pages_ok} ok{reset}  {}{words_suffix}{excl_suffix}",
        if pages_errors > 0 { format!("{yellow}{pages_errors} errors{reset}") } else { format!("{dim}0 errors{reset}") }
    );
    eprintln!("  {dim}elapsed{reset}  {bold}{elapsed:.1}s{reset}");
    if !last_url.is_empty() && !done {
        eprintln!("  {dim}last{reset}     {pink}{last_url}{reset}");
    }
    if running {
        eprintln!("  {dim}pid{reset}      {dim}{pid}{reset}");
        eprintln!("\n  {dim}noxa --crawl {url} --wait{reset}  {dim}to stream live progress{reset}");
    }
    if done {
        // Strip scheme for cleaner display in hints
        let bare = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
        let bare = bare.strip_prefix("www.").unwrap_or(bare);
        let bare = bare.trim_end_matches('/');
        eprintln!("\n  {dim}noxa --list {bare}{reset}          {dim}browse cached pages{reset}");
        eprintln!("  {dim}noxa --retrieve <url>{reset}      {dim}read a specific page{reset}");
        eprintln!("  {dim}noxa --grep \"<term>\" {reset}       {dim}search across all pages{reset}");
    }
    eprintln!();
}

fn spawn_crawl_background(cli: &Cli, resolved: &config::ResolvedConfig) {

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => { eprintln!("error: cannot find self: {e}"); return; }
    };

    // Rebuild args from original argv, inject --wait
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    args.push("--wait".to_string());

    let url = cli.urls.first().map(|s| s.as_str()).unwrap_or("?");
    let status_path = crawl_status_path(&normalize_url(url));
    let log_path = status_path.with_extension("log");
    if let Some(p) = log_path.parent() { let _ = std::fs::create_dir_all(p); }

    let log_file = std::fs::OpenOptions::new()
        .create(true).append(true).open(&log_path)
        .unwrap_or_else(|_| std::fs::File::open("/dev/null").unwrap());
    let log_clone = log_file.try_clone().unwrap_or_else(|_| std::fs::File::open("/dev/null").unwrap());

    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&args)
       .stdin(std::process::Stdio::null())
       .stdout(std::process::Stdio::from(log_file))
       .stderr(std::process::Stdio::from(log_clone));

    // Detach from process group so it survives terminal close
    unsafe { cmd.pre_exec(|| { libc::setsid(); Ok(()) }); }

    let store_root = content_store_root(resolved.output_dir.as_deref());
    let domain_dir = url::Url::parse(&normalize_url(url))
        .ok()
        .and_then(|u| u.host_str().map(|h| h.strip_prefix("www.").unwrap_or(h).replace('.', "_")))
        .map(|d| store_root.join(d))
        .unwrap_or(store_root);

    match cmd.spawn() {
        Ok(child) => {
            eprintln!(
                "\n  {green}{bold}✓ crawl started{reset}  {bold}{cyan}{url}{reset}\n\
                 \n\
                 {dim}  docs{reset}    {pink}{}{reset}\n\
                 {dim}  config{reset}  depth {}  ·  up to {} pages  ·  {} concurrent{}\n\
                 {dim}  status{reset}  noxa --status {url}\n\
                 {dim}  log{reset}     {}{reset}\n\
                 {dim}  pid{reset}     {dim}{}{reset}\n",
                domain_dir.display(),
                resolved.depth, resolved.max_pages, resolved.concurrency,
                if resolved.use_sitemap { "  ·  sitemap" } else { "" },
                log_path.display(), child.id()
            );
        }
        Err(e) => eprintln!("error spawning background crawl: {e}"),
    }
}

async fn run_crawl(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<(), String> {
    let url = cli
        .urls
        .first()
        .ok_or("--crawl requires a URL argument")
        .map(|u| normalize_url(u))?;
    let url = url.as_str();

    if cli.file.is_some() || cli.stdin {
        return Err("--crawl cannot be used with --file or --stdin".into());
    }

    let include_patterns = resolved.include_paths.clone();
    let exclude_patterns = resolved.exclude_paths.clone();

    // Set up streaming progress channel
    let (progress_tx, mut progress_rx) = tokio::sync::broadcast::channel::<PageResult>(100);

    // Set up cancel flag for Ctrl+C handling
    let cancel_flag = Arc::new(AtomicBool::new(false));

    // Register Ctrl+C handler when --crawl-state is set
    let state_path = cli.crawl_state.clone();
    if state_path.is_some() {
        let flag = Arc::clone(&cancel_flag);
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            flag.store(true, Ordering::Relaxed);
            eprintln!("\nCtrl+C received, saving crawl state...");
        });
    }

    let config = CrawlConfig {
        fetch: build_fetch_config(cli, resolved),
        max_depth: resolved.depth,
        max_pages: resolved.max_pages,
        concurrency: resolved.concurrency.max(1),
        delay: std::time::Duration::from_millis(resolved.delay),
        path_prefix: resolved.path_prefix.clone(),
        use_sitemap: resolved.use_sitemap,
        include_patterns,
        exclude_patterns,
        progress_tx: Some(progress_tx),
        cancel_flag: Some(Arc::clone(&cancel_flag)),
        extraction_options: build_extraction_options(resolved),
    };

    // Load resume state if --crawl-state file exists
    let resume_state = state_path
        .as_ref()
        .and_then(|p| Crawler::load_state(p))
        .inspect(|s| {
            eprintln!(
                "Resuming crawl: {} pages already visited, {} URLs in frontier",
                s.visited.len(),
                s.frontier.len(),
            );
        });

    let max_pages = resolved.max_pages;
    let completed_offset = resume_state.as_ref().map_or(0, |s| s.completed_pages);

    // Compute docs dir once — used for status file and final summary
    let store_root = content_store_root(resolved.output_dir.as_deref());
    let domain_dir = url::Url::parse(&normalize_url(url))
        .ok()
        .and_then(|u| u.host_str().map(|h| h.strip_prefix("www.").unwrap_or(h).replace('.', "_")))
        .map(|d| store_root.join(d))
        .unwrap_or(store_root);
    let docs_dir_str = domain_dir.to_string_lossy().to_string();

    // Status file: ~/.noxa/crawls/<domain>.json — updated each page
    let status_path = crawl_status_path(url);
    let start_time = std::time::Instant::now();
    write_crawl_status(&status_path, url, 0, 0, 0, max_pages, None, false, 0.0, &docs_dir_str, 0, 0);

    let status_path_bg = status_path.clone();
    let url_for_status = url.to_string();
    let docs_dir_bg = docs_dir_str.clone();
    let print_progress = cli.wait;

    // Spawn background task to print streaming progress to stderr
    let progress_handle = tokio::spawn(async move {
        let mut count = completed_offset;
        let mut ok = 0usize;
        let mut errors = 0usize;
        while let Ok(page) = progress_rx.recv().await {
            count += 1;
            if page.error.is_some() { errors += 1; } else { ok += 1; }
            if print_progress {
                eprintln!("{}", format_progress(&page, count, max_pages));
            }
            write_crawl_status(
                &status_path_bg, &url_for_status,
                count, ok, errors, max_pages,
                Some(&page.url), false,
                start_time.elapsed().as_secs_f64(),
                &docs_dir_bg, 0, 0,
            );
        }
    });

    let crawler = Crawler::new(url, config).map_err(|e| format!("crawler error: {e}"))?;
    let result = crawler.crawl(url, resume_state).await;

    // Drop the crawler (and its progress_tx clone) so the progress task finishes
    drop(crawler);
    let _ = progress_handle.await;

    // Mark crawl done in status file
    let final_words: usize = result.pages.iter()
        .filter_map(|p| p.extraction.as_ref())
        .map(|e| e.metadata.word_count as usize)
        .sum();
    write_crawl_status(
        &status_path, url,
        result.total, result.ok, result.errors, max_pages,
        None, true, result.elapsed_secs, &docs_dir_str,
        result.excluded, final_words,
    );

    // If cancelled via Ctrl+C and --crawl-state is set, save state for resume
    let was_cancelled = cancel_flag.load(Ordering::Relaxed);
    if was_cancelled {
        if let Some(ref path) = state_path {
            Crawler::save_state(
                path,
                url,
                &result.visited,
                &result.remaining_frontier,
                completed_offset + result.pages.len(),
                resolved.max_pages,
                resolved.depth,
            )?;
            eprintln!(
                "Crawl state saved to {} ({} pages completed). Resume with --crawl-state {}",
                path.display(),
                completed_offset + result.pages.len(),
                path.display(),
            );
        }
    } else if let Some(ref path) = state_path {
        // Crawl completed normally — clean up state file
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }

    // Log per-page errors and extraction warnings to stderr
    for page in &result.pages {
        if let Some(ref err) = page.error {
            eprintln!("error: {} -- {}", page.url, err);
        } else if let Some(ref extraction) = page.extraction {
            let reason = detect_empty(extraction);
            if !matches!(reason, EmptyReason::None) {
                warn_empty(&page.url, &reason);
            }
        }
    }

    // ContentStore auto-persisted every page during the crawl.
    // Show where they landed; if no output_dir is configured, just print to stdout.
    if !cli.no_store {
        let saved = result.pages.iter().filter(|p| p.extraction.is_some()).count();
        let total_words: usize = result.pages.iter()
            .filter_map(|p| p.extraction.as_ref())
            .map(|e| e.metadata.word_count as usize)
            .sum();
        let words_str = if total_words >= 1_000_000 {
            format!("~{:.1}M words", total_words as f64 / 1_000_000.0)
        } else if total_words >= 1_000 {
            format!("~{}k words", total_words / 1_000)
        } else {
            format!("{total_words} words")
        };
        let pages_str = if saved >= max_pages {
            format!("{bold}{saved}{reset}{dim}/{max_pages} (capped){reset}")
        } else {
            format!("{bold}{saved}{reset}{dim}/{max_pages}{reset}")
        };
        let err_part = if result.errors > 0 {
            format!("  {yellow}{} errors{reset}", result.errors)
        } else { String::new() };
        let excl_part = if result.excluded > 0 {
            format!("  {dim}{} excluded{reset}", result.excluded)
        } else { String::new() };
        eprintln!(
            "\n  {green}{bold}✓{reset} {pages_str} pages  {dim}{words_str}{reset}  {pink}{}{reset}  {dim}{:.1}s{reset}{err_part}{excl_part}\n",
            domain_dir.display(), result.elapsed_secs,
        );
    } else {
        print_crawl_output(&result, &resolved.format, resolved.metadata);
    }

    // Fire webhook on crawl complete
    if let Some(ref webhook_url) = cli.webhook {
        let urls: Vec<&str> = result.pages.iter().map(|p| p.url.as_str()).collect();
        fire_webhook(
            webhook_url,
            &serde_json::json!({
                "event": "crawl_complete",
                "total": result.total,
                "ok": result.ok,
                "errors": result.errors,
                "elapsed_secs": result.elapsed_secs,
                "urls": urls,
            }),
        );
        // Brief pause so the async webhook has time to fire
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    if result.errors > 0 {
        Err(format!(
            "{} of {} pages failed",
            result.errors, result.total
        ))
    } else {
        Ok(())
    }
}

async fn run_map(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<(), String> {
    let url = cli
        .urls
        .first()
        .ok_or("--map requires a URL argument")
        .map(|u| normalize_url(u))?;
    let url = url.as_str();

    let client = FetchClient::new(build_fetch_config(cli, resolved))
        .map_err(|e| format!("client error: {e}"))?;

    // map_site() calls sitemap::discover() and appends an Op::Map entry to ops_log.
    let entries = client.map_site(url).await?;

    if entries.is_empty() {
        eprintln!("no sitemap URLs found for {url}");
    } else {
        eprintln!("discovered {} URLs", entries.len());
    }

    print_map_output(&entries, &resolved.format);
    Ok(())
}

async fn run_batch(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    entries: &[(String, Option<String>)],
) -> Result<(), String> {
    let client = Arc::new(
        FetchClient::new(build_fetch_config(cli, resolved))
            .map_err(|e| format!("client error: {e}"))?,
    );

    let urls: Vec<&str> = entries.iter().map(|(u, _)| u.as_str()).collect();
    let options = build_extraction_options(resolved);
    let results = client
        .fetch_and_extract_batch_with_options(&urls, resolved.concurrency, &options)
        .await;

    let ok = results.iter().filter(|r| r.result.is_ok()).count();
    let errors = results.len() - ok;

    // Log errors and extraction warnings to stderr
    for r in &results {
        if let Err(ref e) = r.result {
            eprintln!("error: {} -- {}", r.url, e);
        } else if let Ok(ref extraction) = r.result {
            let reason = detect_empty(extraction);
            if !matches!(reason, EmptyReason::None) {
                warn_empty(&r.url, &reason);
            }
        }
    }

    print_batch_output(&results, &resolved.format, resolved.metadata);
    if !cli.no_store {
        let store_root = content_store_root(resolved.output_dir.as_deref());
        let saved = results.iter().filter(|r| r.result.is_ok()).count();
        eprintln!("Saved {saved} files to {}", store_root.display());
    }

    eprintln!(
        "Fetched {} URLs ({} ok, {} errors)",
        results.len(),
        ok,
        errors
    );

    // Fire webhook on batch complete
    if let Some(ref webhook_url) = cli.webhook {
        let urls: Vec<&str> = results.iter().map(|r| r.url.as_str()).collect();
        fire_webhook(
            webhook_url,
            &serde_json::json!({
                "event": "batch_complete",
                "total": results.len(),
                "ok": ok,
                "errors": errors,
                "urls": urls,
            }),
        );
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    if errors > 0 {
        Err(format!("{errors} of {} URLs failed", results.len()))
    } else {
        Ok(())
    }
}

fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (now % 86400) / 3600;
    let minutes = (now % 3600) / 60;
    let seconds = now % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// Fire a webhook POST with a JSON payload. Non-blocking — errors logged to stderr.
/// Auto-detects Discord and Slack webhook URLs and wraps the payload accordingly.
fn fire_webhook(url: &str, payload: &serde_json::Value) {
    let url = url.to_string();
    let is_discord = url.contains("discord.com/api/webhooks");
    let is_slack = url.contains("hooks.slack.com");

    let body = if is_discord {
        let event = payload
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("notification");
        let details = serde_json::to_string_pretty(payload).unwrap_or_default();
        serde_json::json!({
            "embeds": [{
                "title": format!("noxa: {event}"),
                "description": format!("```json\n{details}\n```"),
                "color": 5814783
            }]
        })
        .to_string()
    } else if is_slack {
        let event = payload
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("notification");
        let details = serde_json::to_string_pretty(payload).unwrap_or_default();
        serde_json::json!({
            "text": format!("*noxa: {event}*\n```{details}```")
        })
        .to_string()
    } else {
        serde_json::to_string(payload).unwrap_or_default()
    };
    tokio::spawn(async move {
        match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => match c
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
            {
                Ok(resp) => {
                    eprintln!(
                        "[webhook] POST {} -> {}",
                        &url[..url.len().min(60)],
                        resp.status()
                    );
                }
                Err(e) => eprintln!("[webhook] POST failed: {e}"),
            },
            Err(e) => eprintln!("[webhook] client error: {e}"),
        }
    });
}

async fn run_watch(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    urls: &[String],
) -> Result<(), String> {
    if urls.is_empty() {
        return Err("--watch requires at least one URL".into());
    }

    let client = Arc::new(
        FetchClient::new(build_fetch_config(cli, resolved))
            .map_err(|e| format!("client error: {e}"))?,
    );
    let options = build_extraction_options(resolved);

    // Ctrl+C handler
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        flag.store(true, Ordering::Relaxed);
    });

    // Single-URL mode: preserve original behavior exactly
    if urls.len() == 1 {
        return run_watch_single(cli, resolved, &client, &options, &urls[0], &cancelled).await;
    }

    // Multi-URL mode: batch fetch, diff each, report aggregate
    run_watch_multi(cli, resolved, &client, &options, urls, &cancelled).await
}

/// Original single-URL watch loop -- backward compatible.
async fn run_watch_single(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    client: &Arc<FetchClient>,
    options: &ExtractionOptions,
    url: &str,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    // Watch restart continuity: try to restore the last stored snapshot as the
    // baseline instead of always doing a fresh fetch on startup.
    // Reuse the client's store instead of creating a redundant instance.
    let store = client.store();

    let (mut previous, mut is_initial_baseline) = if let Some(s) = store {
        match s.read(url).await {
            Ok(Some(stored)) => {
                eprintln!(
                    "[watch] Restored baseline from store: {url} ({} words)",
                    stored.metadata.word_count
                );
                (stored, false)
            }
            _ => {
                let fetched = client
                    .fetch_and_extract_with_options(url, options)
                    .await
                    .map_err(|e| format!("initial fetch failed: {e}"))?;
                eprintln!(
                    "[watch] Initial snapshot: {url} ({} words)",
                    fetched.metadata.word_count
                );
                (fetched, true)
            }
        }
    } else {
        let fetched = client
            .fetch_and_extract_with_options(url, options)
            .await
            .map_err(|e| format!("initial fetch failed: {e}"))?;
        eprintln!(
            "[watch] Initial snapshot: {url} ({} words)",
            fetched.metadata.word_count
        );
        (fetched, false)
    };

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(cli.watch_interval)).await;

        if cancelled.load(Ordering::Relaxed) {
            eprintln!("[watch] Stopped");
            break;
        }

        let current = match client.fetch_and_extract_with_options(url, options).await {
            Ok(result) => result,
            Err(e) => {
                eprintln!("[watch] Fetch error ({}): {e}", timestamp());
                continue;
            }
        };

        let diff = noxa_core::diff::diff(&previous, &current);

        if diff.status == ChangeStatus::Same {
            eprintln!("[watch] No changes ({})", timestamp());
            is_initial_baseline = false;
        } else {
            print_diff_output(&diff, &resolved.format);
            eprintln!("[watch] Changes detected! ({})", timestamp());

            // Append change to ops log.
            let watch_ops_log = client.ops_log().cloned();
            log_operation(
                &watch_ops_log,
                url,
                Op::Diff,
                || serde_json::json!({
                    "source": "watch",
                    "interval_secs": cli.watch_interval
                }),
                || serde_json::to_value(&diff).unwrap_or(serde_json::Value::Null),
            ).await;

            // is_initial_baseline suppresses --on-change on the first reconciliation
            // write when there was no stored snapshot (avoids spurious triggers on startup).
            if !is_initial_baseline {
                if let Some(ref cmd) = cli.on_change {
                    let diff_json = serde_json::to_string(&diff).unwrap_or_default();
                    eprintln!("[watch] Running: {cmd}");
                    match tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmd)
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                    {
                        Ok(mut child) => {
                            if let Some(mut stdin) = child.stdin.take() {
                                use tokio::io::AsyncWriteExt;
                                let _ = stdin.write_all(diff_json.as_bytes()).await;
                            }
                        }
                        Err(e) => eprintln!("[watch] Failed to run command: {e}"),
                    }
                }

                if let Some(ref webhook_url) = cli.webhook {
                    fire_webhook(
                        webhook_url,
                        &serde_json::json!({
                            "event": "watch_change",
                            "url": url,
                            "status": format!("{:?}", diff.status),
                            "word_count_delta": diff.word_count_delta,
                            "metadata_changes": diff.metadata_changes.len(),
                            "links_added": diff.links_added.len(),
                            "links_removed": diff.links_removed.len(),
                        }),
                    );
                }
            }

            is_initial_baseline = false;
            previous = current;
        }
    }

    Ok(())
}

/// Multi-URL watch loop -- batch fetch all URLs, diff each, report aggregate.
async fn run_watch_multi(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    client: &Arc<FetchClient>,
    options: &ExtractionOptions,
    urls: &[String],
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    let url_refs: Vec<&str> = urls.iter().map(|u| u.as_str()).collect();

    // Initial pass: fetch all URLs in parallel
    let initial_results = client
        .fetch_and_extract_batch_with_options(&url_refs, resolved.concurrency, options)
        .await;

    let mut snapshots = std::collections::HashMap::new();
    let mut ok_count = 0usize;
    let mut err_count = 0usize;

    for r in initial_results {
        match r.result {
            Ok(extraction) => {
                snapshots.insert(r.url, extraction);
                ok_count += 1;
            }
            Err(e) => {
                eprintln!("[watch] Initial fetch error: {} -- {e}", r.url);
                err_count += 1;
            }
        }
    }

    eprintln!(
        "[watch] Watching {} URLs (interval: {}s)",
        urls.len(),
        cli.watch_interval
    );
    eprintln!("[watch] Initial snapshots: {ok_count} ok, {err_count} errors");

    let mut check_number = 0u64;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(cli.watch_interval)).await;

        if cancelled.load(Ordering::Relaxed) {
            eprintln!("[watch] Stopped");
            break;
        }

        check_number += 1;

        let current_results = client
            .fetch_and_extract_batch_with_options(&url_refs, resolved.concurrency, options)
            .await;

        let mut changed: Vec<serde_json::Value> = Vec::new();
        let mut same_count = 0usize;
        let mut fetch_errors = 0usize;

        for r in current_results {
            match r.result {
                Ok(current) => {
                    if let Some(previous) = snapshots.get(&r.url) {
                        let diff = noxa_core::diff::diff(previous, &current);
                        if diff.status == ChangeStatus::Same {
                            same_count += 1;
                        } else {
                            changed.push(serde_json::json!({
                                "url": r.url,
                                "word_count_delta": diff.word_count_delta,
                            }));
                            snapshots.insert(r.url, current);
                        }
                    } else {
                        // URL failed initially, first successful fetch -- store as baseline
                        snapshots.insert(r.url, current);
                        same_count += 1;
                    }
                }
                Err(e) => {
                    eprintln!("[watch] Fetch error: {} -- {e}", r.url);
                    fetch_errors += 1;
                }
            }
        }

        let ts = timestamp();
        let err_suffix = if fetch_errors > 0 {
            format!(", {fetch_errors} errors")
        } else {
            String::new()
        };

        if changed.is_empty() {
            eprintln!(
                "[watch] Check {check_number} ({ts}): 0 changed, {same_count} same{err_suffix}"
            );
        } else {
            eprintln!(
                "[watch] Check {check_number} ({ts}): {} changed, {same_count} same{err_suffix}",
                changed.len(),
            );
            for entry in &changed {
                let url = entry["url"].as_str().unwrap_or("?");
                let delta = entry["word_count_delta"].as_i64().unwrap_or(0);
                eprintln!("  -> {url} (word delta: {delta:+})");
            }

            // Append each changed URL to ops log.
            let multi_ops_log = client.ops_log().cloned();
            for entry in &changed {
                if let Some(url) = entry["url"].as_str() {
                    log_operation(
                        &multi_ops_log,
                        url,
                        Op::Diff,
                        || serde_json::json!({
                            "source": "watch",
                            "interval_secs": cli.watch_interval,
                            "check_number": check_number
                        }),
                        || entry.clone(),
                    ).await;
                }
            }

            // Fire --on-change once with all changes
            if let Some(ref cmd) = cli.on_change {
                let payload = serde_json::json!({
                    "event": "watch_changes",
                    "check_number": check_number,
                    "total_urls": urls.len(),
                    "changed": changed.len(),
                    "same": same_count,
                    "changes": changed,
                });
                let payload_json = serde_json::to_string(&payload).unwrap_or_default();
                eprintln!("[watch] Running: {cmd}");
                match tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                {
                    Ok(mut child) => {
                        if let Some(mut stdin) = child.stdin.take() {
                            use tokio::io::AsyncWriteExt;
                            let _ = stdin.write_all(payload_json.as_bytes()).await;
                        }
                    }
                    Err(e) => eprintln!("[watch] Failed to run command: {e}"),
                }
            }

            // Fire webhook once with aggregate payload
            if let Some(ref webhook_url) = cli.webhook {
                fire_webhook(
                    webhook_url,
                    &serde_json::json!({
                        "event": "watch_changes",
                        "check_number": check_number,
                        "total_urls": urls.len(),
                        "changed": changed.len(),
                        "same": same_count,
                        "changes": changed,
                    }),
                );
            }
        }
    }

    Ok(())
}

async fn run_diff(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    snapshot_path: &str,
) -> Result<(), String> {
    // Load previous snapshot
    let snapshot_json = std::fs::read_to_string(snapshot_path)
        .map_err(|e| format!("failed to read snapshot {snapshot_path}: {e}"))?;
    let old: ExtractionResult = serde_json::from_str(&snapshot_json)
        .map_err(|e| format!("failed to parse snapshot JSON: {e}"))?;

    // Extract current version (handles PDF detection for URLs)
    let new_result = fetch_and_extract(cli, resolved).await?.into_extraction()?;

    let diff = noxa_core::diff::diff(&old, &new_result);

    // Append diff result to ops log.
    let ops_log = build_ops_log(cli, resolved);
    let url = cli.urls.first().map(|u| normalize_url(u)).unwrap_or_default();
    log_operation(
        &ops_log,
        &url,
        Op::Diff,
        || serde_json::json!({ "source": "file", "snapshot": snapshot_path }),
        || serde_json::to_value(&diff).unwrap_or(serde_json::Value::Null),
    ).await;

    print_diff_output(&diff, &resolved.format);
    Ok(())
}

async fn run_brand(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<(), String> {
    let result = fetch_html(cli, resolved).await?;
    let enriched = enrich_html_with_stylesheets(&result.html, &result.url).await;
    let brand = noxa_core::brand::extract_brand(
        &enriched,
        Some(result.url.as_str()).filter(|s| !s.is_empty()),
    );

    let ops_log = build_ops_log(cli, resolved);
    log_operation(
        &ops_log,
        &result.url,
        Op::Brand,
        || serde_json::json!({}),
        || serde_json::to_value(&brand).unwrap_or(serde_json::Value::Null),
    ).await;

    let output = serde_json::to_string_pretty(&brand).expect("serialization failed");
    println!("{output}");
    Ok(())
}

/// Build an LLM provider based on CLI flags, or fall back to the default chain.
async fn build_llm_provider(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
) -> Result<Box<dyn LlmProvider>, String> {
    if let Some(ref name) = resolved.llm_provider {
        match name.as_str() {
            "gemini" => {
                let provider = noxa_llm::providers::gemini_cli::GeminiCliProvider::new(
                    resolved.llm_model.clone(),
                );
                if !provider.is_available().await {
                    return Err(
                        "gemini CLI not found on PATH -- install it or omit --llm-provider".into(),
                    );
                }
                Ok(Box::new(provider))
            }
            "ollama" => {
                let provider = noxa_llm::providers::ollama::OllamaProvider::new(
                    cli.llm_base_url.clone(),
                    resolved.llm_model.clone(),
                );
                if !provider.is_available().await {
                    return Err("ollama is not running or unreachable".into());
                }
                Ok(Box::new(provider))
            }
            "openai" => {
                let provider = noxa_llm::providers::openai::OpenAiProvider::new(
                    None,
                    cli.llm_base_url.clone(),
                    resolved.llm_model.clone(),
                )
                .ok_or("OPENAI_API_KEY not set")?;
                Ok(Box::new(provider))
            }
            "anthropic" => {
                let provider = noxa_llm::providers::anthropic::AnthropicProvider::new(
                    None,
                    resolved.llm_model.clone(),
                )
                .ok_or("ANTHROPIC_API_KEY not set")?;
                Ok(Box::new(provider))
            }
            other => Err(format!(
                "unknown LLM provider: {other} (use gemini, ollama, openai, or anthropic)"
            )),
        }
    } else {
        let chain = noxa_llm::ProviderChain::default().await;
        if chain.is_empty() {
            return Err(
                "no LLM providers available (priority: Gemini CLI -> OpenAI -> Ollama -> Anthropic) -- install gemini on PATH, set OPENAI_API_KEY, OLLAMA_HOST / OLLAMA_MODEL, or ANTHROPIC_API_KEY"
                    .into(),
            );
        }
        Ok(Box::new(chain))
    }
}

async fn run_llm(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<(), String> {
    // Extract content from source first (handles PDF detection for URLs)
    let result = fetch_and_extract(cli, resolved).await?.into_extraction()?;

    let url = cli.urls.first().map(|u| normalize_url(u)).unwrap_or_default();
    let provider = build_llm_provider(cli, resolved).await?;
    let model = resolved.llm_model.as_deref();
    let ops_log = build_ops_log(cli, resolved);

    let output_str: Option<String> = if let Some(ref schema_input) = cli.extract_json {
        // Support @file syntax for loading schema from file
        let schema_str = if let Some(path) = schema_input.strip_prefix('@') {
            std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read schema file {path}: {e}"))?
        } else {
            schema_input.clone()
        };

        let schema: serde_json::Value =
            serde_json::from_str(&schema_str).map_err(|e| format!("invalid JSON schema: {e}"))?;

        let t = std::time::Instant::now();
        let extracted = noxa_llm::extract::extract_json(
            &result.content.plain_text,
            &schema,
            provider.as_ref(),
            model,
        )
        .await
        .map_err(|e| format!("LLM extraction failed: {e}"))?;
        eprintln!("LLM: {:.1}s", t.elapsed().as_secs_f64());

        log_operation(
            &ops_log,
            &url,
            Op::Extract,
            || serde_json::json!({
                "kind": "json",
                "schema": schema,
                "provider": provider.name(),
                "model": model
            }),
            || extracted.clone(),
        ).await;

        Some(serde_json::to_string_pretty(&extracted).expect("serialization failed"))
    } else if let Some(ref prompt) = cli.extract_prompt {
        let t = std::time::Instant::now();
        let extracted = noxa_llm::extract::extract_with_prompt(
            &result.content.plain_text,
            prompt,
            provider.as_ref(),
            model,
        )
        .await
        .map_err(|e| format!("LLM extraction failed: {e}"))?;
        eprintln!("LLM: {:.1}s", t.elapsed().as_secs_f64());

        log_operation(
            &ops_log,
            &url,
            Op::Extract,
            || serde_json::json!({
                "kind": "prompt",
                "prompt": prompt,
                "provider": provider.name(),
                "model": model
            }),
            || extracted.clone(),
        ).await;

        Some(serde_json::to_string_pretty(&extracted).expect("serialization failed"))
    } else if let Some(sentences) = cli.summarize {
        let t = std::time::Instant::now();
        let summary = noxa_llm::summarize::summarize(
            &result.content.plain_text,
            Some(sentences),
            provider.as_ref(),
            model,
        )
        .await
        .map_err(|e| format!("LLM summarization failed: {e}"))?;
        eprintln!("LLM: {:.1}s", t.elapsed().as_secs_f64());

        log_operation(
            &ops_log,
            &url,
            Op::Summarize,
            || serde_json::json!({
                "sentences": sentences,
                "provider": provider.name(),
                "model": model
            }),
            || serde_json::Value::String(summary.clone()),
        ).await;

        Some(summary)
    } else {
        None
    };

    if let Some(s) = output_str {
        println!("{s}");
    }

    Ok(())
}

/// Batch LLM extraction: fetch each URL, run LLM on extracted content, save/print results.
/// URLs are processed sequentially to respect LLM provider rate limits.
async fn run_batch_llm(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    entries: &[(String, Option<String>)],
) -> Result<(), String> {
    let client = FetchClient::new(build_fetch_config(cli, resolved))
        .map_err(|e| format!("client error: {e}"))?;
    let options = build_extraction_options(resolved);
    let provider = build_llm_provider(cli, resolved).await?;
    let model = resolved.llm_model.as_deref();
    let ops_log = client.ops_log().cloned();

    // Pre-parse schema once if --extract-json is used
    let schema = if let Some(ref schema_input) = cli.extract_json {
        let schema_str = if let Some(path) = schema_input.strip_prefix('@') {
            std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read schema file {path}: {e}"))?
        } else {
            schema_input.clone()
        };
        Some(
            serde_json::from_str::<serde_json::Value>(&schema_str)
                .map_err(|e| format!("invalid JSON schema: {e}"))?,
        )
    } else {
        None
    };

    let total = entries.len();
    let mut ok = 0usize;
    let mut errors = 0usize;
    let mut all_results: Vec<serde_json::Value> = Vec::with_capacity(total);

    for (i, (url, _)) in entries.iter().enumerate() {
        let idx = i + 1;
        eprint!("[{idx}/{total}] {url} ");

        // Fetch and extract page content
        let extraction = match client.fetch_and_extract_with_options(url, &options).await {
            Ok(r) => r,
            Err(e) => {
                errors += 1;
                let msg = format!("fetch failed: {e}");
                eprintln!("-> error: {msg}");
                all_results.push(serde_json::json!({ "url": url, "error": msg }));
                continue;
            }
        };

        let text = &extraction.content.plain_text;

        // Run the appropriate LLM operation
        let llm_start = std::time::Instant::now();
        let llm_result = if let Some(ref schema) = schema {
            noxa_llm::extract::extract_json(text, schema, provider.as_ref(), model)
                .await
                .map(LlmOutput::Json)
        } else if let Some(ref prompt) = cli.extract_prompt {
            noxa_llm::extract::extract_with_prompt(text, prompt, provider.as_ref(), model)
                .await
                .map(LlmOutput::Json)
        } else if let Some(sentences) = cli.summarize {
            noxa_llm::summarize::summarize(text, Some(sentences), provider.as_ref(), model)
                .await
                .map(LlmOutput::Text)
        } else {
            unreachable!("run_batch_llm called without LLM flags")
        };
        let llm_elapsed = llm_start.elapsed();

        match llm_result {
            Ok(output) => {
                ok += 1;

                let (output_str, result_json) = match &output {
                    LlmOutput::Json(v) => {
                        let s = serde_json::to_string_pretty(v).expect("serialization failed");
                        let j = serde_json::json!({ "url": url, "result": v });
                        (s, j)
                    }
                    LlmOutput::Text(s) => {
                        let j = serde_json::json!({ "url": url, "result": s });
                        (s.clone(), j)
                    }
                };

                // Count top-level fields/items for progress display
                let detail = match &output {
                    LlmOutput::Json(v) => match v {
                        serde_json::Value::Object(m) => format!("{} fields", m.len()),
                        serde_json::Value::Array(a) => format!("{} items", a.len()),
                        _ => "done".to_string(),
                    },
                    LlmOutput::Text(s) => {
                        let words = s.split_whitespace().count();
                        format!("{words} words")
                    }
                };
                eprintln!("-> extracted {detail} ({:.1}s)", llm_elapsed.as_secs_f64());

                // Append to ops log.
                {
                    let (op, log_input, log_output) = if let Some(ref schema) = schema {
                        (Op::Extract,
                         serde_json::json!({ "kind": "json", "schema": schema, "provider": provider.name(), "model": model }),
                         match &output { LlmOutput::Json(v) => v.clone(), LlmOutput::Text(s) => serde_json::Value::String(s.clone()) })
                    } else if let Some(ref prompt) = cli.extract_prompt {
                        (Op::Extract,
                         serde_json::json!({ "kind": "prompt", "prompt": prompt, "provider": provider.name(), "model": model }),
                         match &output { LlmOutput::Json(v) => v.clone(), LlmOutput::Text(s) => serde_json::Value::String(s.clone()) })
                    } else {
                        let sentences = cli.summarize.unwrap_or(3);
                        (Op::Summarize,
                         serde_json::json!({ "sentences": sentences, "provider": provider.name(), "model": model }),
                         match &output { LlmOutput::Text(s) => serde_json::Value::String(s.clone()), LlmOutput::Json(v) => v.clone() })
                    };
                    log_operation(&ops_log, url, op, || log_input, || log_output).await;
                }

                println!("--- {url}");
                println!("{output_str}");
                println!();

                all_results.push(result_json);
            }
            Err(e) => {
                errors += 1;
                let msg = format!("LLM extraction failed: {e}");
                eprintln!("-> error: {msg}");
                all_results.push(serde_json::json!({ "url": url, "error": msg }));
            }
        }
    }

    eprintln!("Processed {total} URLs ({ok} ok, {errors} errors)");

    if let Some(ref webhook_url) = cli.webhook {
        fire_webhook(
            webhook_url,
            &serde_json::json!({
                "event": "batch_llm_complete",
                "total": total,
                "ok": ok,
                "errors": errors,
            }),
        );
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    if errors > 0 {
        Err(format!("{errors} of {total} URLs failed"))
    } else {
        Ok(())
    }
}

/// Intermediate type to hold LLM output before formatting.
enum LlmOutput {
    Json(serde_json::Value),
    Text(String),
}

/// Returns true if any LLM flag is set.
fn has_llm_flags(cli: &Cli) -> bool {
    cli.extract_json.is_some() || cli.extract_prompt.is_some() || cli.summarize.is_some()
}

async fn run_research(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    query: &str,
) -> Result<(), String> {
    let api_key = cli
        .api_key
        .as_deref()
        .ok_or("--research requires NOXA_API_KEY (set via env or --api-key)")?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("http client error: {e}"))?;

    let mut body = serde_json::json!({ "query": query });
    if cli.deep {
        body["deep"] = serde_json::json!(true);
    }

    eprintln!("Starting research: {query}");
    if cli.deep {
        eprintln!("Deep mode enabled (longer, more thorough)");
    }

    // Start job
    let resp = client
        .post("https://api.noxa.io/v1/research")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API error: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("parse error: {e}"))?;

    let job_id = resp
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("API did not return a job ID")?
        .to_string();

    eprintln!("Job started: {job_id}");

    // Poll
    for poll in 0..200 {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let status_resp = client
            .get(format!("https://api.noxa.io/v1/research/{job_id}"))
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| format!("poll error: {e}"))?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("parse error: {e}"))?;

        let status = status_resp
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match status {
            "completed" => {
                let report = status_resp
                    .get("report")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Save full result to JSON file
                let slug: String = query
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == ' ' {
                            c
                        } else {
                            ' '
                        }
                    })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join("-")
                    .to_lowercase();
                let slug = if slug.len() > 50 { &slug[..50] } else { &slug };
                let filename = format!("research-{slug}.json");

                let json = serde_json::to_string_pretty(&status_resp).unwrap_or_default();
                if let Some(ref dir) = resolved.output_dir {
                    let output_dir = dir.clone();
                    write_to_file(&output_dir, &filename, &json)?;
                } else {
                    std::fs::write(&filename, &json)
                        .map_err(|e| format!("failed to write {filename}: {e}"))?;
                }

                let elapsed = status_resp
                    .get("elapsed_ms")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let sources = status_resp
                    .get("sources_count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let findings = status_resp
                    .get("findings_count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                eprintln!(
                    "Research complete: {sources} sources, {findings} findings, {:.1}s",
                    elapsed as f64 / 1000.0
                );
                eprintln!("Saved to: {filename}");

                // Print report to stdout
                if !report.is_empty() {
                    println!("{report}");
                }

                return Ok(());
            }
            "failed" => {
                let error = status_resp
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                return Err(format!("Research failed: {error}"));
            }
            _ => {
                if poll % 10 == 9 {
                    eprintln!("Still researching... ({:.0}s)", (poll + 1) as f64 * 3.0);
                }
            }
        }
    }

    Err(format!(
        "Research timed out after ~10 minutes. Check status: GET /v1/research/{job_id}"
    ))
}

fn run_list(filter: &str, store_root: std::path::PathBuf) {

    if !store_root.exists() {
        eprintln!("{dim}no local docs yet — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --search \"...\"{reset} {dim}to build your store{reset}");
        return;
    }

    if filter.is_empty() {
        // Top-level: list all domain directories with doc counts
        let mut domains: Vec<(String, usize)> = std::fs::read_dir(&store_root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let count = count_md_files(&e.path());
                (name, count)
            })
            .collect();
        domains.sort_by(|a, b| a.0.cmp(&b.0));

        if domains.is_empty() {
            eprintln!("{dim}no docs stored yet{reset}");
            return;
        }

        let total: usize = domains.iter().map(|(_, c)| c).sum();
        eprintln!("\n{bold}{cyan}stored docs{reset}  {dim}{total} total{reset}\n");
        for (domain, count) in &domains {
            eprintln!("  {bold}{domain}{reset}  {dim}({count}){reset}");
        }
        eprintln!("\n{dim}noxa --list <domain>{reset}  {dim}to see individual docs{reset}\n");
    } else {
        // Domain view: list all docs for matching domain dir, URL → path
        let domain_dir = store_root.join(filter.strip_prefix("www.").unwrap_or(filter));
        if !domain_dir.exists() {
            // Try sanitized form (dots → underscores)
            let sanitized: String = filter.chars().map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }
            }).collect();
            let alt = store_root.join(&sanitized);
            if alt.exists() {
                list_domain_docs(&alt, &store_root, filter);
            } else {
                eprintln!("{dim}no docs found for{reset} {bold}{filter}{reset}");
            }
            return;
        }
        list_domain_docs(&domain_dir, &store_root, filter);
    }
}

fn count_md_files(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries.flatten().map(|e| {
        let p = e.path();
        if p.is_dir() {
            count_md_files(&p)
        } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
            1
        } else {
            0
        }
    }).sum()
}

fn list_domain_docs(
    dir: &std::path::Path,
    store_root: &std::path::Path,
    filter: &str,
) {
    let mut docs: Vec<(String, std::path::PathBuf)> = Vec::new();
    collect_docs(dir, store_root, &mut docs);
    docs.sort_by(|a, b| a.1.cmp(&b.1));

    if docs.is_empty() {
        eprintln!("{dim}no docs found for {reset}{bold}{filter}{reset}");
        return;
    }

    // Measure URL column width for alignment
    let url_width = docs.iter().map(|(url, _)| url.len()).max().unwrap_or(0);

    eprintln!("\n{bold}{cyan}{filter}{reset}  {dim}({} docs){reset}\n", docs.len());
    for (url, path) in &docs {
        let rel = path.strip_prefix(store_root).unwrap_or(path);
        eprintln!("  {blue}{url:<url_width$}{reset}  {dim}{}{reset}", rel.display());
    }
    eprintln!();
}

fn collect_docs(
    dir: &std::path::Path,
    store_root: &std::path::Path,
    out: &mut Vec<(String, std::path::PathBuf)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_docs(&path, store_root, out);
        } else if path.extension().and_then(|x| x.to_str()) == Some("md") {
            // Read URL from JSON sidecar; fall back to reconstructing from path.
            // Support both the new Sidecar envelope (url at top-level, metadata
            // nested under "current") and the legacy raw ExtractionResult format
            // (metadata at top-level).
            let json_path = path.with_extension("json");
            let url = std::fs::read_to_string(&json_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| {
                    // New sidecar: top-level "url" field.
                    if let Some(u) = v["url"].as_str().filter(|s| !s.is_empty()) {
                        return Some(u.to_string());
                    }
                    // New sidecar: nested under current.metadata.url.
                    if let Some(u) = v["current"]["metadata"]["url"].as_str() {
                        return Some(u.to_string());
                    }
                    // Legacy format: metadata.url at top level.
                    v["metadata"]["url"].as_str().map(|u| u.to_string())
                });
            if let Some(url) = url {
                out.push((url, path));
            }
        }
    }
}

fn run_grep(pattern: &str, store_root: std::path::PathBuf) {

    if !store_root.exists() {
        eprintln!("{dim}no local docs yet — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --search \"...\"{reset} {dim}to build your store{reset}");
        return;
    }

    eprintln!("\n{bold}{cyan}grep{reset}  {bold}{pattern}{reset}  {dim}{}{reset}\n", store_root.display());

    // Try rg first — it's fast and produces great output natively
    let rg_status = std::process::Command::new("rg")
        .args(["--color=always", "--heading", "--line-number", "--smart-case", pattern])
        .arg(&store_root)
        .status();

    match rg_status {
        Ok(status) => {
            if status.code() == Some(1) {
                // rg exit 1 = no matches
                eprintln!("{dim}no matches for {reset}{bold}\"{pattern}\"{reset}");
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // rg not installed — fall back to a simple Rust walk
            eprintln!("{dim}rg not found, using built-in search{reset}\n");
            let pattern_lower = pattern.to_lowercase();
            let mut matched_files = 0usize;
            let mut matched_lines = 0usize;
            if let Ok(walker) = std::fs::read_dir(&store_root) {
                grep_dir(walker, &pattern_lower, &store_root, &mut matched_files, &mut matched_lines);
            }
            if matched_files == 0 {
                eprintln!("{dim}no matches for {reset}{bold}\"{pattern}\"{reset}");
            } else {
                eprintln!("\n{green}{bold}✓{reset} {bold}{matched_lines} match(es){reset} {dim}across {matched_files} file(s){reset}");
            }
        }
        Err(e) => eprintln!("error running rg: {e}"),
    }
}

fn grep_dir(
    entries: std::fs::ReadDir,
    pattern: &str,
    root: &std::path::Path,
    matched_files: &mut usize,
    matched_lines: &mut usize,
) {
    let mut paths: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if let Ok(sub) = std::fs::read_dir(&path) {
                grep_dir(sub, pattern, root, matched_files, matched_lines);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            let hits: Vec<(usize, &str)> = content
                .lines()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(pattern))
                .collect();
            if !hits.is_empty() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                eprintln!("{pink}{}{reset}", rel.display());
                for (lineno, line) in &hits {
                    let trimmed = line.trim();
                    let display = if trimmed.len() > 120 { &trimmed[..120] } else { trimmed };
                    eprintln!("  {dim}{:>4}{reset}  {bold}{display}{reset}", lineno + 1);
                    *matched_lines += 1;
                }
                eprintln!();
                *matched_files += 1;
            }
        }
    }
}

async fn run_search(
    cli: &Cli,
    fetch_client: &Arc<noxa_fetch::FetchClient>,
    resolved: &config::ResolvedConfig,
    query: &str,
) -> Result<(), String> {
    if query.trim().is_empty() {
        return Err("Search query must not be empty or whitespace-only.".into());
    }

    let num = cli.num_results.clamp(1, 50);
    let concurrency = clamp_search_scrape_concurrency(cli.num_scrape_concurrency);

    let mut search_backend = String::from("noxa cloud");
    let results: Vec<(String, String, String)> = {
        let searxng_url = std::env::var("SEARXNG_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(base_url) = searxng_url {
            validate_operator_url(&base_url).map_err(|e| format!("SEARXNG_URL is invalid: {e}"))?;
            // Strip scheme for display (searxng.example.com vs https://searxng.example.com)
            let display = base_url
                .strip_prefix("https://")
                .or_else(|| base_url.strip_prefix("http://"))
                .unwrap_or(&base_url);
            search_backend = format!("searxng ({display})");
            noxa_fetch::searxng_search(fetch_client, &base_url, query, num)
                .await
                .map_err(|e| format!("SearXNG search failed: {e}"))?
                .into_iter()
                .map(|r| (r.title, r.url, r.content))
                .collect()
        } else {
            let api_key = cli
                .api_key
                .clone()
                .filter(|s| !s.is_empty())
                .or_else(|| std::env::var("NOXA_API_KEY").ok().filter(|s| !s.is_empty()))
                .ok_or("--search requires SEARXNG_URL or NOXA_API_KEY")?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| format!("http client error: {e}"))?;
            let resp: serde_json::Value = client
                .post("https://api.noxa.io/v1/search")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&serde_json::json!({ "query": query, "num_results": num }))
                .send()
                .await
                .map_err(|e| format!("API error: {e}"))?
                .json()
                .await
                .map_err(|e| format!("parse error: {e}"))?;
            resp.get("results")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|r| {
                            (
                                r.get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                r.get("url")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                r.get("snippet")
                                    .or_else(|| r.get("content"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
    };


    if results.is_empty() {
        eprintln!("{yellow}no results found for: {query}{reset}");
        return Ok(());
    }

    eprintln!(
        "\n{bold}{cyan}search{reset}  {bold}{query}{reset}  {dim}{} result(s)  via {search_backend}{reset}\n",
        results.len()
    );

    if cli.no_scrape {
        for (i, (title, url, snip)) in results.iter().enumerate() {
            println!("{dim}{i:2}.{reset} {bold}{title}{reset}");
            println!("     {blue}{url}{reset}");
            if !snip.is_empty() {
                println!("     {dim}{snip}{reset}");
            }
            println!();
        }
        return Ok(());
    }

    let store_root = content_store_root(resolved.output_dir.as_deref());

    let valid: Vec<(usize, String, String, String)> = results
        .into_iter()
        .enumerate()
        .filter_map(|(i, (title, url, snip))| match validate_url_sync(&url) {
            Ok(()) => Some((i + 1, title, url, snip)),
            Err(e) => {
                eprintln!("{dim}   skip {url}: {e}{reset}");
                None
            }
        })
        .collect();

    let url_refs: Vec<&str> = valid.iter().map(|(_, _, u, _)| u.as_str()).collect();
    let options = build_extraction_options(resolved);
    let scraped = fetch_client
        .fetch_and_extract_batch_with_options(&url_refs, concurrency, &options)
        .await;

    for ((idx, title, url, snip), scrape) in valid.iter().zip(scraped.iter()) {
        let store_path = store_root.join(url_to_store_path(url)).with_extension("md");
        println!("{dim}{idx:2}.{reset} {bold}{title}{reset}");
        println!("     {blue}{url}{reset}");
        if !snip.is_empty() {
            println!("     {dim}{snip}{reset}");
        }
        match &scrape.result {
            Ok(_)  => println!("     {green}✓{reset} {pink}{}{reset}", store_path.display()),
            Err(e) => println!("     {yellow}✗ scrape failed:{reset} {dim}{e}{reset}"),
        }
        println!();
    }

    let saved = scraped.iter().filter(|s| s.result.is_ok()).count();
    eprintln!(
        "{green}{bold}✓{reset} {bold}{saved}/{} scraped{reset}  {pink}{}{reset}\n\
         {dim}  grep{reset}  {cyan}noxa --grep {green}\"TERM\"{reset}\n",
        valid.len(),
        store_root.display(),
    );

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    if matches!(std::env::args().nth(1).as_deref(), Some("setup")) {
        setup::run();
        return;
    }

    if matches!(std::env::args().nth(1).as_deref(), Some("mcp")) {
        init_mcp_logging();

        if let Err(e) = noxa_mcp::run().await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // Use low-level API to get both typed Cli and ArgMatches for ValueSource detection.
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    // Load config BEFORE init_logging so verbose from config takes effect.
    let cfg = config::NoxaConfig::load(cli.config.as_deref());
    let resolved = config::resolve(&cli, &matches, &cfg);

    init_logging(resolved.verbose);

    // Validate webhook URL early so any SSRF attempt is rejected before operations run.
    if let Some(ref webhook_url) = cli.webhook {
        if let Err(e) = validate_url(webhook_url).await {
            eprintln!("error: invalid webhook URL: {e}");
            process::exit(1);
        }
    }

    // --map: sitemap discovery mode
    if cli.map {
        if let Err(e) = run_map(&cli, &resolved).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    if let Some(ref domain) = cli.status {
        run_status(domain);
        return;
    }

    if let Some(ref query) = cli.retrieve {
        run_retrieve(query, content_store_root(resolved.output_dir.as_deref()));
        return;
    }

    // --crawl: recursive crawl mode
    if cli.crawl {
        if cli.wait {
            // Block and stream live progress
            if let Err(e) = run_crawl(&cli, &resolved).await {
                eprintln!("error: {e}");
                process::exit(1);
            }
        } else {
            // Background mode: re-exec self with --wait, detach
            spawn_crawl_background(&cli, &resolved);
        }
        return;
    }

    // --watch: poll URL(s) for changes
    if cli.watch {
        let watch_urls: Vec<String> = match collect_urls(&cli) {
            Ok(entries) => entries.into_iter().map(|(url, _)| url).collect(),
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        };
        if let Err(e) = run_watch(&cli, &resolved, &watch_urls).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // --diff-with: change tracking mode
    if let Some(ref snapshot_path) = cli.diff_with {
        if let Err(e) = run_diff(&cli, &resolved, snapshot_path).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // --brand: brand identity extraction mode
    if cli.brand {
        if let Err(e) = run_brand(&cli, &resolved).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // --research: deep research via cloud API
    if let Some(ref query) = cli.research {
        if let Err(e) = run_research(&cli, &resolved, query).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    if let Some(ref filter) = cli.list {
        run_list(filter, content_store_root(resolved.output_dir.as_deref()));
        return;
    }

    if let Some(ref pattern) = cli.grep {
        run_grep(pattern, content_store_root(resolved.output_dir.as_deref()));
        return;
    }

    if let Some(ref query) = cli.search {
        let fetch_client = Arc::new(
            noxa_fetch::FetchClient::new(build_fetch_config(&cli, &resolved)).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                process::exit(1);
            }),
        );
        if let Err(e) = run_search(&cli, &fetch_client, &resolved, query).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // Collect all URLs from args + --urls-file
    let entries = match collect_urls(&cli) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // LLM modes: --extract-json, --extract-prompt, --summarize
    // When multiple URLs are provided, run batch LLM extraction over all of them.
    if has_llm_flags(&cli) {
        if entries.len() > 1 {
            if let Err(e) = run_batch_llm(&cli, &resolved, &entries).await {
                eprintln!("error: {e}");
                process::exit(1);
            }
        } else if let Err(e) = run_llm(&cli, &resolved).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // Multi-URL batch mode
    if entries.len() > 1 {
        if let Err(e) = run_batch(&cli, &resolved, &entries).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // --raw-html: skip extraction, dump the fetched HTML
    if resolved.raw_html {
        match fetch_html(&cli, &resolved).await {
            Ok(r) => println!("{}", r.html),
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
        return;
    }

    // Single-page extraction (handles both HTML and PDF via content-type detection)
    match fetch_and_extract(&cli, &resolved).await {
        Ok(FetchOutput::Local(result)) => {
            print_output(&result, &resolved.format, resolved.metadata);
            if !cli.no_store {
                let url = cli
                    .urls
                    .first()
                    .map(|u| normalize_url(u))
                    .unwrap_or_default();
                let content = format_output(&result, &resolved.format, resolved.metadata);
                let store_root = content_store_root(resolved.output_dir.as_deref());
                let dest = store_root
                    .join(url_to_store_path(&url))
                    .with_extension("md");
                print_save_hint(&dest, &content);
            }
        }
        Ok(FetchOutput::Cloud(resp)) => {
            print_cloud_output(&resp, &resolved.format);
            if !cli.no_store {
                let url = cli
                    .urls
                    .first()
                    .map(|u| normalize_url(u))
                    .unwrap_or_default();
                let content = format_cloud_output(&resp, &resolved.format);
                let store_root = content_store_root(resolved.output_dir.as_deref());
                let dest = store_root
                    .join(url_to_store_path(&url))
                    .with_extension("md");
                print_save_hint(&dest, &content);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_to_filename_root() {
        assert_eq!(
            url_to_filename("https://example.com/", &OutputFormat::Markdown),
            "example_com/index.md"
        );
        assert_eq!(
            url_to_filename("https://example.com", &OutputFormat::Markdown),
            "example_com/index.md"
        );
    }

    #[test]
    fn url_to_filename_path() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Markdown),
            "example_com/docs/api.md"
        );
    }

    #[test]
    fn url_to_filename_trailing_slash() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api/", &OutputFormat::Markdown),
            "example_com/docs/api.md"
        );
    }

    #[test]
    fn url_to_filename_nested_path() {
        assert_eq!(
            url_to_filename("https://example.com/blog/my-post", &OutputFormat::Markdown),
            "example_com/blog/my-post.md"
        );
    }

    #[test]
    fn url_to_filename_query_params() {
        assert_eq!(
            url_to_filename("https://example.com/p?id=123", &OutputFormat::Markdown),
            "example_com/p_id_123.md"
        );
    }

    #[test]
    fn url_to_filename_json_format() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Json),
            "example_com/docs/api.json"
        );
    }

    #[test]
    fn url_to_filename_text_format() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Text),
            "example_com/docs/api.txt"
        );
    }

    #[test]
    fn url_to_filename_llm_format() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Llm),
            "example_com/docs/api.md"
        );
    }

    #[test]
    fn url_to_filename_html_format() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Html),
            "example_com/docs/api.html"
        );
    }

    #[test]
    fn url_to_filename_special_chars() {
        // Spaces and special chars get replaced with underscores
        assert_eq!(
            url_to_filename(
                "https://example.com/path%20with%20spaces",
                &OutputFormat::Markdown
            ),
            "example_com/path_20with_20spaces.md"
        );
    }

    #[test]
    fn write_to_file_creates_dirs() {
        let dir = std::env::temp_dir().join("noxa_test_output_dir");
        let _ = std::fs::remove_dir_all(&dir);
        write_to_file(&dir, "nested/deep/file.md", "hello").unwrap();
        let content = std::fs::read_to_string(dir.join("nested/deep/file.md")).unwrap();
        assert_eq!(content, "hello");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_to_file_rejects_traversal() {
        let dir = std::env::temp_dir().join("noxa_sec_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(write_to_file(&dir, "../escaped.md", "x").is_err());
        assert!(write_to_file(&dir, "/abs.md", "x").is_err());
        assert!(write_to_file(&dir, "..\\windows.md", "x").is_err());
        assert!(write_to_file(&dir, "null\0byte.md", "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_to_file_allows_nested() {
        let dir = std::env::temp_dir().join("noxa_sec_test2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(write_to_file(&dir, "sub/file.md", "hello").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_content_store_root_no_output_dir() {
        let root = content_store_root(None);
        assert!(root.ends_with(".noxa/content"));
    }

    #[test]
    fn test_content_store_root_with_output_dir() {
        let dir = std::path::PathBuf::from("/tmp/mybase");
        assert_eq!(content_store_root(Some(&dir)), dir.join(".noxa/content"));
    }

    #[test]
    fn test_default_search_dir_under_noxa() {
        let d = default_search_dir();
        assert!(d.to_string_lossy().contains(".noxa"));
        assert!(d.to_string_lossy().contains("search"));
    }

    #[test]
    fn test_url_to_filename_flat_for_search() {
        let raw = url_to_filename("https://example.com/blog/post", &OutputFormat::Markdown);
        let flat = raw.replace('/', "_");
        assert!(!flat.contains('/'));
        assert!(flat.ends_with(".md"));
    }

    #[tokio::test]
    async fn validate_public_url_rejects_ipv6_private_ranges() {
        assert!(validate_url("http://[fe80::1]/").await.is_err());
        assert!(validate_url("http://[fc00::1]/").await.is_err());
    }

    #[test]
    fn validate_operator_url_allows_localhost() {
        assert!(validate_operator_url("http://127.0.0.1:8080").is_ok());
        assert!(validate_operator_url("https://localhost/search").is_ok());
    }

    #[test]
    fn search_scrape_concurrency_is_clamped() {
        assert_eq!(clamp_search_scrape_concurrency(0), 1);
        assert_eq!(clamp_search_scrape_concurrency(50), 20);
        assert_eq!(clamp_search_scrape_concurrency(4), 4);
    }
}

#[cfg(test)]
mod enum_deserialize_tests {
    use super::*;

    #[test]
    fn test_output_format_deserialize() {
        let f: OutputFormat = serde_json::from_str("\"llm\"").unwrap();
        assert!(matches!(f, OutputFormat::Llm));
        let f: OutputFormat = serde_json::from_str("\"markdown\"").unwrap();
        assert!(matches!(f, OutputFormat::Markdown));
    }

    #[test]
    fn test_browser_deserialize() {
        let b: Browser = serde_json::from_str("\"firefox\"").unwrap();
        assert!(matches!(b, Browser::Firefox));
    }

    #[test]
    fn test_pdf_mode_deserialize() {
        let p: PdfModeArg = serde_json::from_str("\"fast\"").unwrap();
        assert!(matches!(p, PdfModeArg::Fast));
    }

    // --- validate_url tests ---

    #[tokio::test]
    async fn validate_rejects_loopback() {
        assert!(validate_url("http://127.0.0.1/secret").await.is_err());
        assert!(validate_url("http://127.0.0.1:8080/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_localhost() {
        assert!(validate_url("http://localhost/secret").await.is_err());
        assert!(validate_url("http://localhost:8080/secret").await.is_err());
        assert!(validate_url("http://foo.localhost/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_rfc1918() {
        assert!(validate_url("http://10.0.0.1/").await.is_err());
        assert!(validate_url("http://172.16.0.1/").await.is_err());
        assert!(validate_url("http://172.31.255.255/").await.is_err());
        assert!(validate_url("http://192.168.1.1/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_link_local() {
        assert!(validate_url("http://169.254.169.254/latest/meta-data/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_tailscale() {
        assert!(validate_url("http://100.100.1.1/").await.is_err());
        assert!(validate_url("http://100.127.255.255/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_loopback() {
        assert!(validate_url("http://[::1]/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_link_local() {
        assert!(validate_url("http://[fe80::1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_ula() {
        assert!(validate_url("http://[fd00::1]/").await.is_err());
        assert!(validate_url("http://[fc00::1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv4_mapped_ipv6() {
        assert!(validate_url("http://[::ffff:127.0.0.1]/").await.is_err());
        assert!(validate_url("http://[::ffff:169.254.169.254]/latest/meta-data/").await.is_err());
        assert!(validate_url("http://[::ffff:10.0.0.1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_accepts_public_ip() {
        assert!(validate_url("http://8.8.8.8/").await.is_ok());
        assert!(validate_url("http://1.1.1.1/").await.is_ok());
    }

    // Use validate_url_impl with a mock resolver to test the DNS path without
    // hitting the network. This keeps hostname validation covered in all CI environments.

    #[tokio::test]
    async fn validate_accepts_hostname_resolving_to_public() {
        let result = validate_url_impl("http://example.com/", |_| async {
            Ok(vec!["93.184.216.34:80".parse::<std::net::SocketAddr>().unwrap()])
        })
        .await;
        assert!(result.is_ok(), "hostname resolving to a public IP should be accepted");
    }

    #[tokio::test]
    async fn validate_rejects_hostname_resolving_to_private() {
        let result = validate_url_impl("http://attacker.example/", |_| async {
            Ok(vec!["192.168.1.1:80".parse::<std::net::SocketAddr>().unwrap()])
        })
        .await;
        assert!(result.is_err(), "hostname resolving to a private IP should be rejected");
    }

    #[tokio::test]
    async fn validate_rejects_hostname_dns_failure() {
        let result = validate_url_impl("http://nxdomain.example/", |_| async {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no such host"))
        })
        .await;
        assert!(result.is_err(), "DNS failure should be rejected (fail-closed)");
    }

    // --- write_to_file traversal tests ---

    #[test]
    fn test_write_to_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        assert!(write_to_file(path, "../escape.md", "x").is_err());
        assert!(write_to_file(path, "/etc/passwd", "x").is_err());
        assert!(write_to_file(path, "..\\windows\\evil", "x").is_err());
        assert!(write_to_file(path, "foo\0bar", "x").is_err());
    }

    #[test]
    fn test_write_to_file_allows_nested() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        assert!(write_to_file(path, "sub/file.md", "hello").is_ok());
        assert!(path.join("sub/file.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_write_to_file_rejects_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let outside = tempfile::tempdir().unwrap();
        // Create a symlink inside `dir` pointing outside.
        let link = path.join("link");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        // Attempting to write through the symlink should be rejected.
        assert!(write_to_file(path, "link/escape.md", "x").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_write_to_file_rejects_leaf_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("secret.txt");
        // Create a leaf symlink inside `dir` pointing to a file outside.
        let link = path.join("output.md");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        // Writing to the leaf symlink should be rejected.
        assert!(write_to_file(path, "output.md", "x").is_err());
    }

    #[test]
    fn content_store_root_uses_explicit_output_dir() {
        let path = std::path::PathBuf::from("custom-output");
        assert_eq!(
            content_store_root(Some(path.as_path())),
            path.join(".noxa/content")
        );
    }

    #[test]
    fn content_store_root_defaults_to_noxa_content() {
        let result = content_store_root(None);
        // The default path should end with .noxa/content regardless of home dir
        assert!(
            result.ends_with(".noxa/content"),
            "expected path ending with .noxa/content, got: {}",
            result.display()
        );
    }
}
