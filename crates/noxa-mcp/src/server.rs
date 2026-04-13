/// MCP server implementation for noxa.
/// Exposes web extraction capabilities as tools for AI agents.
///
/// Uses a local-first architecture: fetches pages directly, then falls back
/// to the noxa cloud API (api.noxa.io) when bot protection or
/// JS rendering is detected. Set NOXA_API_KEY for automatic fallback.
use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde_json::json;
use tracing::{error, info, warn};
use url::Url;

use crate::cloud::{self, CloudClient, SmartFetchResult};
use crate::tools::*;

const NO_LLM_PROVIDERS_MESSAGE: &str = "No LLM providers available (priority: Gemini CLI -> OpenAI -> Ollama -> Anthropic). Install gemini on PATH, set OPENAI_API_KEY, OLLAMA_HOST / OLLAMA_MODEL, or ANTHROPIC_API_KEY, or set NOXA_API_KEY for cloud fallback.";

pub struct NoxaMcp {
    tool_router: ToolRouter<Self>,
    fetch_client: Arc<noxa_fetch::FetchClient>,
    llm_chain: Option<noxa_llm::ProviderChain>,
    cloud: Option<CloudClient>,
    store: noxa_fetch::ContentStore,
}

/// Parse a browser string into a BrowserProfile.
fn parse_browser(browser: Option<&str>) -> noxa_fetch::BrowserProfile {
    match browser {
        Some("firefox") => noxa_fetch::BrowserProfile::Firefox,
        Some("random") => noxa_fetch::BrowserProfile::Random,
        _ => noxa_fetch::BrowserProfile::Chrome,
    }
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
    let parsed = Url::parse(url).map_err(|e| format!("Invalid URL: {e}. Must start with http:// or https://"))?;
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

/// Timeout for local fetch calls (prevents hanging on tarpitting servers).
const LOCAL_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum poll iterations for research jobs (~10 minutes at 3s intervals).
const RESEARCH_MAX_POLLS: u32 = 200;

#[tool_router]
impl NoxaMcp {
    pub async fn new() -> Self {
        let mut config = noxa_fetch::FetchConfig::default();

        // Load proxy config from env vars or local file
        if let Ok(proxy) = std::env::var("NOXA_PROXY") {
            info!("using single proxy from NOXA_PROXY");
            config.proxy = Some(proxy);
        }

        let proxy_file = std::env::var("NOXA_PROXY_FILE")
            .ok()
            .unwrap_or_else(|| "proxies.txt".to_string());
        if std::path::Path::new(&proxy_file).exists()
            && let Ok(pool) = noxa_fetch::parse_proxy_file(&proxy_file)
            && !pool.is_empty()
        {
            info!(count = pool.len(), file = %proxy_file, "loaded proxy pool");
            config.proxy_pool = pool;
        }

        // Create the content store first so we can clone it into FetchConfig.
        let store = noxa_fetch::ContentStore::open();
        info!("content store ready");

        // Inject store into FetchConfig so FetchClient auto-persists all extractions.
        config.store = Some(store.clone());

        let fetch_client = match noxa_fetch::FetchClient::new(config) {
            Ok(client) => client,
            Err(e) => {
                error!("failed to build FetchClient: {e}");
                std::process::exit(1);
            }
        };

        let chain = noxa_llm::ProviderChain::default().await;
        let llm_chain = if chain.is_empty() {
            warn!("{NO_LLM_PROVIDERS_MESSAGE} -- extract/summarize tools will fail");
            None
        } else {
            info!(providers = chain.len(), "LLM provider chain ready");
            Some(chain)
        };

        let cloud = CloudClient::from_env();
        if cloud.is_some() {
            info!("cloud API fallback enabled (NOXA_API_KEY set)");
        } else {
            warn!(
                "NOXA_API_KEY not set -- bot-protected sites will return challenge pages. \
                 Get a key at https://noxa.io"
            );
        }

        Self {
            tool_router: Self::tool_router(),
            fetch_client: Arc::new(fetch_client),
            llm_chain,
            cloud,
            store,
        }
    }

    /// Helper: smart fetch with LLM format for extract/summarize tools.
    async fn smart_fetch_llm(&self, url: &str) -> Result<SmartFetchResult, String> {
        cloud::smart_fetch(
            &self.fetch_client,
            self.cloud.as_ref(),
            url,
            &[],
            &[],
            false,
            &["llm", "markdown"],
        )
        .await
    }

    /// Scrape a single URL and extract its content as markdown, LLM-optimized text, plain text, or full JSON.
    /// Automatically falls back to the noxa cloud API when bot protection or JS rendering is detected.
    #[tool]
    async fn scrape(&self, Parameters(params): Parameters<ScrapeParams>) -> Result<String, String> {
        validate_url(&params.url).await?;
        let format = params.format.as_deref().unwrap_or("markdown");
        let browser = parse_browser(params.browser.as_deref());
        let include = params.include_selectors.unwrap_or_default();
        let exclude = params.exclude_selectors.unwrap_or_default();
        let main_only = params.only_main_content.unwrap_or(false);

        // Build cookie header from params
        let cookie_header = params
            .cookies
            .as_ref()
            .filter(|c| !c.is_empty())
            .map(|c| c.join("; "));

        // Use a custom client if non-default browser or cookies are provided
        let is_default_browser = matches!(browser, noxa_fetch::BrowserProfile::Chrome);
        let needs_custom = !is_default_browser || cookie_header.is_some();
        let custom_client;
        let client: &noxa_fetch::FetchClient = if needs_custom {
            let mut headers = std::collections::HashMap::new();
            headers.insert("Accept-Language".to_string(), "en-US,en;q=0.9".to_string());
            if let Some(ref cookies) = cookie_header {
                headers.insert("Cookie".to_string(), cookies.clone());
            }
            let config = noxa_fetch::FetchConfig {
                browser,
                headers,
                store: Some(self.store.clone()),
                ..Default::default()
            };
            custom_client = noxa_fetch::FetchClient::new(config)
                .map_err(|e| format!("Failed to build client: {e}"))?;
            &custom_client
        } else {
            &self.fetch_client
        };

        let formats = [format];
        let result = cloud::smart_fetch(
            client,
            self.cloud.as_ref(),
            &params.url,
            &include,
            &exclude,
            main_only,
            &formats,
        )
        .await?;

        match result {
            SmartFetchResult::Local(extraction) => {
                let output = match format {
                    "llm" => noxa_core::to_llm_text(&extraction, Some(&params.url)),
                    "text" => extraction.content.plain_text,
                    "json" => serde_json::to_string_pretty(&extraction).unwrap_or_default(),
                    _ => extraction.content.markdown,
                };
                Ok(output)
            }
            SmartFetchResult::Cloud(resp) => {
                // Extract the requested format from the API response
                let content = resp
                    .get(format)
                    .or_else(|| resp.get("markdown"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if content.is_empty() {
                    // Return full JSON if no content in the expected format
                    Ok(serde_json::to_string_pretty(&resp).unwrap_or_default())
                } else {
                    Ok(content.to_string())
                }
            }
        }
    }

    /// Crawl a website starting from a seed URL, following links breadth-first up to a configurable depth and page limit.
    #[tool]
    async fn crawl(&self, Parameters(params): Parameters<CrawlParams>) -> Result<String, String> {
        validate_url(&params.url).await?;

        if let Some(max) = params.max_pages
            && max > 500
        {
            return Err("max_pages cannot exceed 500".into());
        }

        let format = params.format.as_deref().unwrap_or("markdown");

        let concurrency = params.concurrency.unwrap_or(5);
        if concurrency == 0 || concurrency > 20 {
            return Err(format!("concurrency must be between 1 and 20 (got {concurrency})"));
        }

        let config = noxa_fetch::CrawlConfig {
            max_depth: params.depth.unwrap_or(2) as usize,
            max_pages: params.max_pages.unwrap_or(50),
            concurrency,
            use_sitemap: params.use_sitemap.unwrap_or(false),
            fetch: noxa_fetch::FetchConfig {
                store: Some(self.store.clone()),
                ..Default::default()
            },
            ..Default::default()
        };

        let crawler = noxa_fetch::Crawler::new(&params.url, config)
            .map_err(|e| format!("Crawler init failed: {e}"))?;

        let result = crawler.crawl(&params.url, None).await;

        let mut output = format!(
            "Crawled {} pages ({} ok, {} errors) in {:.1}s\n\n",
            result.total, result.ok, result.errors, result.elapsed_secs
        );

        for page in &result.pages {
            output.push_str(&format!("--- {} (depth {}) ---\n", page.url, page.depth));
            if let Some(ref extraction) = page.extraction {
                let content = match format {
                    "llm" => noxa_core::to_llm_text(extraction, Some(&page.url)),
                    "text" => extraction.content.plain_text.clone(),
                    _ => extraction.content.markdown.clone(),
                };
                output.push_str(&content);
            } else if let Some(ref err) = page.error {
                output.push_str(&format!("Error: {err}"));
            }
            output.push_str("\n\n");
        }

        Ok(output)
    }

    /// Discover URLs from a website's sitemaps (robots.txt + sitemap.xml).
    #[tool]
    async fn map(&self, Parameters(params): Parameters<MapParams>) -> Result<String, String> {
        validate_url(&params.url).await?;
        let entries = noxa_fetch::sitemap::discover(&self.fetch_client, &params.url)
            .await
            .map_err(|e| format!("Sitemap discovery failed: {e}"))?;

        let urls: Vec<&str> = entries.iter().map(|e| e.url.as_str()).collect();
        Ok(format!(
            "Discovered {} URLs:\n\n{}",
            urls.len(),
            urls.join("\n")
        ))
    }

    /// Extract content from multiple URLs concurrently.
    #[tool]
    async fn batch(&self, Parameters(params): Parameters<BatchParams>) -> Result<String, String> {
        if params.urls.is_empty() {
            return Err("urls must not be empty".into());
        }
        if params.urls.len() > 100 {
            return Err("batch is limited to 100 URLs per request".into());
        }
        for u in &params.urls {
            validate_url(u).await?;
        }

        let format = params.format.as_deref().unwrap_or("markdown");
        let concurrency = params.concurrency.unwrap_or(5);
        if concurrency == 0 || concurrency > 20 {
            return Err(format!("concurrency must be between 1 and 20 (got {concurrency})"));
        }
        let url_refs: Vec<&str> = params.urls.iter().map(String::as_str).collect();

        let results = self
            .fetch_client
            .fetch_and_extract_batch(&url_refs, concurrency)
            .await;

        let mut output = format!("Extracted {} URLs:\n\n", results.len());

        for r in &results {
            output.push_str(&format!("--- {} ---\n", r.url));
            match &r.result {
                Ok(extraction) => {
                    let content = match format {
                        "llm" => noxa_core::to_llm_text(extraction, Some(&r.url)),
                        "text" => extraction.content.plain_text.clone(),
                        _ => extraction.content.markdown.clone(),
                    };
                    output.push_str(&content);
                }
                Err(e) => {
                    output.push_str(&format!("Error: {e}"));
                }
            }
            output.push_str("\n\n");
        }

        Ok(output)
    }

    /// Extract structured data from a web page using an LLM. Provide either a JSON schema or a natural language prompt.
    /// Falls back to the noxa cloud API when no local LLM is available or bot protection is detected.
    #[tool]
    async fn extract(
        &self,
        Parameters(params): Parameters<ExtractParams>,
    ) -> Result<String, String> {
        validate_url(&params.url).await?;

        if params.schema.is_none() && params.prompt.is_none() {
            return Err("Either 'schema' or 'prompt' is required for extraction.".into());
        }

        // No local LLM — fall back to cloud API directly
        if self.llm_chain.is_none() {
            let cloud = self.cloud.as_ref().ok_or(NO_LLM_PROVIDERS_MESSAGE)?;
            let mut body = json!({"url": params.url});
            if let Some(ref schema) = params.schema {
                body["schema"] = json!(schema);
            }
            if let Some(ref prompt) = params.prompt {
                body["prompt"] = json!(prompt);
            }
            let resp = cloud.post("extract", body).await?;
            return Ok(serde_json::to_string_pretty(&resp).unwrap_or_default());
        }

        let chain = self.llm_chain.as_ref().unwrap();

        let llm_content = match self.smart_fetch_llm(&params.url).await? {
            SmartFetchResult::Local(extraction) => {
                noxa_core::to_llm_text(&extraction, Some(&params.url))
            }
            SmartFetchResult::Cloud(resp) => resp
                .get("llm")
                .or_else(|| resp.get("markdown"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };

        let data = if let Some(ref schema) = params.schema {
            noxa_llm::extract::extract_json(&llm_content, schema, chain, None)
                .await
                .map_err(|e| format!("LLM extraction failed: {e}"))?
        } else {
            let prompt = params.prompt.as_deref().unwrap();
            noxa_llm::extract::extract_with_prompt(&llm_content, prompt, chain, None)
                .await
                .map_err(|e| format!("LLM extraction failed: {e}"))?
        };

        Ok(serde_json::to_string_pretty(&data).unwrap_or_default())
    }

    /// Summarize the content of a web page using an LLM.
    /// Falls back to the noxa cloud API when no local LLM is available or bot protection is detected.
    #[tool]
    async fn summarize(
        &self,
        Parameters(params): Parameters<SummarizeParams>,
    ) -> Result<String, String> {
        validate_url(&params.url).await?;

        // No local LLM — fall back to cloud API directly
        if self.llm_chain.is_none() {
            let cloud = self.cloud.as_ref().ok_or(NO_LLM_PROVIDERS_MESSAGE)?;
            let mut body = json!({"url": params.url});
            if let Some(sentences) = params.max_sentences {
                body["max_sentences"] = json!(sentences);
            }
            let resp = cloud.post("summarize", body).await?;
            let summary = resp.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            if summary.is_empty() {
                return Ok(serde_json::to_string_pretty(&resp).unwrap_or_default());
            }
            return Ok(summary.to_string());
        }

        let chain = self.llm_chain.as_ref().unwrap();

        let llm_content = match self.smart_fetch_llm(&params.url).await? {
            SmartFetchResult::Local(extraction) => {
                noxa_core::to_llm_text(&extraction, Some(&params.url))
            }
            SmartFetchResult::Cloud(resp) => resp
                .get("llm")
                .or_else(|| resp.get("markdown"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };

        noxa_llm::summarize::summarize(&llm_content, params.max_sentences, chain, None)
            .await
            .map_err(|e| format!("Summarization failed: {e}"))
    }

    /// Compare the current content of a URL against a previous extraction snapshot, showing what changed.
    /// Automatically falls back to the noxa cloud API when bot protection is detected.
    #[tool]
    async fn diff(&self, Parameters(params): Parameters<DiffParams>) -> Result<String, String> {
        validate_url(&params.url).await?;

        // Load the previous snapshot. IMPORTANT: this read must complete and bind
        // to `previous` before any fetch for the same URL — otherwise the fetch
        // auto-write would overwrite the snapshot we're about to read.
        let previous: Option<noxa_core::ExtractionResult> = match params.previous_snapshot {
            Some(ref json) => Some(
                serde_json::from_str(json)
                    .map_err(|e| format!("Failed to parse previous_snapshot JSON: {e}"))?,
            ),
            // Err from store.read (e.g. corrupt JSON) is treated as None — proceed
            // to the first-fetch path rather than blocking the user.
            None => self.store.read(&params.url).await.ok().flatten(),
        };

        let previous = match previous {
            Some(p) => p,
            None => {
                // No stored snapshot: fetch-and-store the current page as the
                // baseline, then return an informative error.
                info!(url = %params.url, "diff: no previous snapshot — fetching baseline");
                let _ = cloud::smart_fetch(
                    &self.fetch_client,
                    self.cloud.as_ref(),
                    &params.url,
                    &[],
                    &[],
                    false,
                    &["markdown"],
                )
                .await;
                return Err(format!(
                    "No previous snapshot stored for {url}. The page has been fetched and \
                     stored as the baseline — run diff again to compare against this snapshot.",
                    url = params.url
                ));
            }
        };

        let result = cloud::smart_fetch(
            &self.fetch_client,
            self.cloud.as_ref(),
            &params.url,
            &[],
            &[],
            false,
            &["markdown"],
        )
        .await?;

        match result {
            SmartFetchResult::Local(current) => {
                let content_diff = noxa_core::diff::diff(&previous, &current);
                Ok(serde_json::to_string_pretty(&content_diff).unwrap_or_default())
            }
            SmartFetchResult::Cloud(resp) => {
                // Extract markdown from the cloud response and build a minimal
                // ExtractionResult so we can compute the diff locally.
                let markdown = resp.get("markdown").and_then(|v| v.as_str()).unwrap_or("");

                if markdown.is_empty() {
                    return Err(
                        "Cloud API fallback returned no markdown content; cannot compute diff."
                            .into(),
                    );
                }

                let current = noxa_core::ExtractionResult {
                    content: noxa_core::Content {
                        markdown: markdown.to_string(),
                        plain_text: markdown.to_string(),
                        links: Vec::new(),
                        images: Vec::new(),
                        code_blocks: Vec::new(),
                        raw_html: None,
                    },
                    metadata: noxa_core::Metadata {
                        title: None,
                        description: None,
                        author: None,
                        published_date: None,
                        language: None,
                        url: Some(params.url.clone()),
                        site_name: None,
                        image: None,
                        favicon: None,
                        word_count: markdown.split_whitespace().count(),
                    },
                    domain_data: None,
                    structured_data: Vec::new(),
                };

                let content_diff = noxa_core::diff::diff(&previous, &current);
                Ok(serde_json::to_string_pretty(&content_diff).unwrap_or_default())
            }
        }
    }

    /// Extract brand identity (colors, fonts, logo, favicon) from a website's HTML and CSS.
    /// Automatically falls back to the noxa cloud API when bot protection is detected.
    #[tool]
    async fn brand(&self, Parameters(params): Parameters<BrandParams>) -> Result<String, String> {
        validate_url(&params.url).await?;
        let fetch_result =
            tokio::time::timeout(LOCAL_FETCH_TIMEOUT, self.fetch_client.fetch(&params.url))
                .await
                .map_err(|_| format!("Fetch timed out after 30s for {}", params.url))?
                .map_err(|e| format!("Fetch failed: {e}"))?;

        // Check for bot protection before extracting brand
        if cloud::is_bot_protected(&fetch_result.html, &fetch_result.headers) {
            if let Some(ref c) = self.cloud {
                let resp = c
                    .post("brand", serde_json::json!({"url": params.url}))
                    .await?;
                return Ok(serde_json::to_string_pretty(&resp).unwrap_or_default());
            } else {
                return Err(format!(
                    "Bot protection detected on {}. Set NOXA_API_KEY for automatic cloud bypass. \
                     Get a key at https://noxa.io",
                    params.url
                ));
            }
        }

        let identity = noxa_core::brand::extract_brand(&fetch_result.html, Some(&fetch_result.url));

        Ok(serde_json::to_string_pretty(&identity).unwrap_or_default())
    }

    /// Run a deep research investigation on a topic or question. Requires NOXA_API_KEY.
    /// Saves full result to ~/.noxa/research/ and returns the file path + key findings.
    /// Checks cache first — same query returns the cached result without spending credits.
    #[tool]
    async fn research(
        &self,
        Parameters(params): Parameters<ResearchParams>,
    ) -> Result<String, String> {
        let cloud = self
            .cloud
            .as_ref()
            .ok_or("Research requires NOXA_API_KEY. Get a key at https://noxa.io")?;

        let research_dir = research_dir();
        let slug = slugify(&params.query);

        // Check cache first
        if let Some(cached) = load_cached_research(&research_dir, &slug) {
            info!(query = %params.query, "returning cached research");
            return Ok(cached);
        }

        let mut body = json!({ "query": params.query });
        if let Some(deep) = params.deep {
            body["deep"] = json!(deep);
        }
        if let Some(ref topic) = params.topic {
            body["topic"] = json!(topic);
        }

        // Start the research job
        let start_resp = cloud.post("research", body).await?;
        let job_id = start_resp
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Research API did not return a job ID")?
            .to_string();

        info!(job_id = %job_id, "research job started, polling for completion");

        // Poll until completed or failed
        for poll in 0..RESEARCH_MAX_POLLS {
            tokio::time::sleep(Duration::from_secs(3)).await;

            let status_resp = cloud.get(&format!("research/{job_id}")).await?;
            let status = status_resp
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            match status {
                "completed" => {
                    // Save full result to file
                    let (report_path, json_path) =
                        save_research(&research_dir, &slug, &status_resp);

                    // Build compact response: file paths + findings (no full report)
                    let sources_count = status_resp
                        .get("sources_count")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let findings_count = status_resp
                        .get("findings_count")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    let mut response = json!({
                        "status": "completed",
                        "query": params.query,
                        "report_file": report_path,
                        "json_file": json_path,
                        "sources_count": sources_count,
                        "findings_count": findings_count,
                    });

                    if let Some(findings) = status_resp.get("findings") {
                        response["findings"] = findings.clone();
                    }
                    if let Some(sources) = status_resp.get("sources") {
                        response["sources"] = sources.clone();
                    }

                    return Ok(serde_json::to_string_pretty(&response).unwrap_or_default());
                }
                "failed" => {
                    let error = status_resp
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown error");
                    return Err(format!("Research job failed: {error}"));
                }
                _ => {
                    if poll % 20 == 19 {
                        info!(job_id = %job_id, poll, "research still in progress...");
                    }
                }
            }
        }

        Err(format!(
            "Research job {job_id} timed out after ~10 minutes of polling. \
             Check status manually via the noxa API: GET /v1/research/{job_id}"
        ))
    }

    /// Search using SearXNG (`SEARXNG_URL`) or cloud (`NOXA_API_KEY`).
    #[tool]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> Result<String, String> {
        if params.query.trim().is_empty() {
            return Err("query must not be empty".into());
        }
        let num = params.num_results.unwrap_or(10).clamp(1, 50);

        let searxng_url = std::env::var("SEARXNG_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(base_url) = searxng_url {
            parse_http_url(&base_url)?;

            let results =
                noxa_fetch::searxng_search(&self.fetch_client, &base_url, &params.query, num)
                    .await
                    .map_err(|e| format!("SearXNG search failed: {e}"))?;

            if results.is_empty() {
                return Ok(format!("No results found for: {}", params.query));
            }

            let valid_results: Vec<&noxa_fetch::SearxngResult> = results
                .iter()
                .filter(|r| {
                    if let Err(e) = validate_url(&r.url) {
                        warn!("skipping result URL {}: {e}", r.url);
                        false
                    } else {
                        true
                    }
                })
                .collect();
            let valid_urls: Vec<&str> = valid_results.iter().map(|r| r.url.as_str()).collect();
            let scraped = self
                .fetch_client
                .fetch_and_extract_batch(&valid_urls, 4)
                .await;

            let mut out = String::with_capacity(results.len() * 256);
            out.push_str(&format!("Found {} result(s):\n\n", valid_results.len()));

            // Note: store writes are handled automatically by FetchClient.fetch_and_extract
            // inside fetch_and_extract_batch above. Explicit writes here were removed to
            // prevent double-writes. The "saved/updated/unchanged" label is intentionally
            // absent — FetchClient writes are fire-and-forget.
            for (idx, (r, scrape)) in valid_results.iter().zip(scraped.iter()).enumerate() {
                out.push_str(&format!("{}. {}\n   {}\n", idx + 1, r.title, r.url));
                if !r.content.is_empty() {
                    out.push_str(&format!("   {}\n", r.content));
                }
                if let Err(ref e) = scrape.result {
                    out.push_str(&format!("   Error: {e}\n"));
                }
                out.push('\n');
            }

            return Ok(out);
        }

        let cloud = self.cloud.as_ref().ok_or(
            "Search requires SEARXNG_URL (self-hosted SearXNG) or NOXA_API_KEY (cloud). \
             Set SEARXNG_URL to your SearXNG instance URL.",
        )?;
        let body = json!({ "query": params.query, "num_results": num });
        let resp = cloud.post("search", body).await?;

        if let Some(results) = resp.get("results").and_then(|v| v.as_array()) {
            let mut out = String::with_capacity(results.len() * 256);
            out.push_str(&format!("Found {} result(s):\n\n", results.len()));
            for (i, r) in results.iter().enumerate() {
                let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let url = r.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let snip = r
                    .get("snippet")
                    .or_else(|| r.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                out.push_str(&format!("{}. {}\n   {}\n", i + 1, title, url));
                if !snip.is_empty() {
                    out.push_str(&format!("   {snip}\n"));
                }
                out.push('\n');
            }
            Ok(out)
        } else {
            Ok(serde_json::to_string_pretty(&resp).unwrap_or_default())
        }
    }
}

#[tool_handler]
impl ServerHandler for NoxaMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("noxa-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(String::from(
                "Noxa MCP server -- web content extraction for AI agents. \
                 Tools: scrape, crawl, map, batch, extract, summarize, diff, brand, research, search.",
            ))
    }
}

// ---------------------------------------------------------------------------
// Research file helpers
// ---------------------------------------------------------------------------

fn research_dir() -> std::path::PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".noxa")
        .join("research");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn slugify(query: &str) -> String {
    let s: String = query
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
    if s.len() > 60 { s[..60].to_string() } else { s }
}

/// Check for a cached research result. Returns the compact response if found.
fn load_cached_research(dir: &std::path::Path, slug: &str) -> Option<String> {
    let json_path = dir.join(format!("{slug}.json"));
    let report_path = dir.join(format!("{slug}.md"));

    if !json_path.exists() || !report_path.exists() {
        return None;
    }

    let json_str = std::fs::read_to_string(&json_path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    // Build compact response from cache
    let mut response = json!({
        "status": "completed",
        "cached": true,
        "query": data.get("query").cloned().unwrap_or(json!("")),
        "report_file": report_path.to_string_lossy(),
        "json_file": json_path.to_string_lossy(),
        "sources_count": data.get("sources_count").cloned().unwrap_or(json!(0)),
        "findings_count": data.get("findings_count").cloned().unwrap_or(json!(0)),
    });

    if let Some(findings) = data.get("findings") {
        response["findings"] = findings.clone();
    }
    if let Some(sources) = data.get("sources") {
        response["sources"] = sources.clone();
    }

    Some(serde_json::to_string_pretty(&response).unwrap_or_default())
}

/// Save research result to disk. Returns (report_path, json_path) as strings.
fn save_research(dir: &std::path::Path, slug: &str, data: &serde_json::Value) -> (String, String) {
    let json_path = dir.join(format!("{slug}.json"));
    let report_path = dir.join(format!("{slug}.md"));

    // Save full JSON
    if let Ok(json_str) = serde_json::to_string_pretty(data) {
        std::fs::write(&json_path, json_str).ok();
    }

    // Save report as markdown
    if let Some(report) = data.get("report").and_then(|v| v.as_str()) {
        std::fs::write(&report_path, report).ok();
    }

    (
        report_path.to_string_lossy().to_string(),
        json_path.to_string_lossy().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Uses a literal IP — no DNS needed, fast.
        assert!(validate_url("http://8.8.8.8/").await.is_ok());
        assert!(validate_url("http://1.1.1.1/").await.is_ok());
    }

    // Use validate_url_impl with a mock resolver to test the DNS path without
    // hitting the network. This keeps hostname validation covered in all CI environments.

    #[tokio::test]
    async fn validate_accepts_hostname_resolving_to_public() {
        let result = validate_url_impl("http://example.com/", |_| async {
            // Simulate example.com → 93.184.216.34 (IANA-assigned, public)
            Ok(vec!["93.184.216.34:80".parse::<std::net::SocketAddr>().unwrap()])
        })
        .await;
        assert!(result.is_ok(), "hostname resolving to a public IP should be accepted");
    }

    #[tokio::test]
    async fn validate_rejects_hostname_resolving_to_private() {
        let result = validate_url_impl("http://attacker.example/", |_| async {
            // Simulate DNS rebinding: attacker.example → 192.168.1.1 (private)
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

    #[tokio::test]
    async fn validate_rejects_ipv6_link_local_and_ula() {
        assert!(validate_url("http://[fe80::1]/").is_err());
        assert!(validate_url("http://[fc00::1]/").is_err());
    }

    #[test]
    fn test_num_results_clamp() {
        assert_eq!(0_u32.clamp(1, 50), 1);
        assert_eq!(100_u32.clamp(1, 50), 50);
        assert_eq!(10_u32.clamp(1, 50), 10);
    }
}
