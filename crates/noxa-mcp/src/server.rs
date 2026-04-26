/// MCP server implementation for noxa.
/// Exposes web extraction capabilities as tools for AI agents.
///
/// Uses a local-first architecture: fetches pages directly, then falls back
/// to the noxa cloud API (api.noxa.io) when bot protection or
/// JS rendering is detected. Set NOXA_API_KEY for automatic fallback.
use std::sync::Arc;
use std::time::Duration;

use noxa_llm::LlmProvider;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde_json::json;
use tracing::{info, warn};

use crate::cloud::{self, CloudClient, SmartFetchResult};
use crate::config::NoxaMcpConfig;
use crate::error::NoxaMcpError;
use crate::research::{
    ResearchRequest, build_research_response, load_cached_research, save_research,
};
use crate::serialization::to_pretty_json;
use crate::tools::*;
use crate::validation::{collect_valid_urls, validate_fetch_url, validate_fetch_urls};

const NO_LLM_PROVIDERS_MESSAGE: &str = "No LLM providers available (priority: Gemini CLI -> OpenAI -> Ollama -> Anthropic). Install gemini on PATH, set OPENAI_API_KEY, OLLAMA_HOST / OLLAMA_MODEL, or ANTHROPIC_API_KEY, or set NOXA_API_KEY for cloud fallback.";
type ToolResult = Result<String, String>;

async fn build_llm_chain(
    config: &NoxaMcpConfig,
) -> Result<Option<noxa_llm::ProviderChain>, NoxaMcpError> {
    let chain = if let Some(ref name) = config.llm_provider {
        let provider: Box<dyn noxa_llm::LlmProvider> = match name.as_str() {
            "gemini" => {
                let provider = noxa_llm::providers::gemini_cli::GeminiCliProvider::new(
                    config.llm_model.clone(),
                );
                if !provider.is_available().await {
                    return Err(NoxaMcpError::llm(
                        "gemini CLI not found on PATH -- install it or omit NOXA_LLM_PROVIDER",
                    ));
                }
                Box::new(provider)
            }
            "ollama" => {
                let provider = noxa_llm::providers::ollama::OllamaProvider::new(
                    config.llm_base_url.clone(),
                    config.llm_model.clone(),
                );
                if !provider.is_available().await {
                    return Err(NoxaMcpError::llm("ollama is not running or unreachable"));
                }
                Box::new(provider)
            }
            "openai" => Box::new(
                noxa_llm::providers::openai::OpenAiProvider::new(
                    None,
                    config.llm_base_url.clone(),
                    config.llm_model.clone(),
                )
                .ok_or_else(|| NoxaMcpError::llm("OPENAI_API_KEY not set"))?,
            ),
            "anthropic" => Box::new(
                noxa_llm::providers::anthropic::AnthropicProvider::new(
                    None,
                    config.llm_model.clone(),
                )
                .ok_or_else(|| NoxaMcpError::llm("ANTHROPIC_API_KEY not set"))?,
            ),
            other => {
                return Err(NoxaMcpError::invalid_parameter(format!(
                    "unknown LLM provider: {other} (use gemini, ollama, openai, or anthropic)"
                )));
            }
        };
        noxa_llm::ProviderChain::single(provider)
    } else {
        noxa_llm::ProviderChain::default().await
    };

    if chain.is_empty() {
        warn!("{NO_LLM_PROVIDERS_MESSAGE} -- extract/summarize tools will fail");
        Ok(None)
    } else {
        info!(providers = chain.len(), "LLM provider chain ready");
        Ok(Some(chain))
    }
}

pub struct NoxaMcp {
    tool_router: ToolRouter<Self>,
    config: Arc<NoxaMcpConfig>,
    fetch_client: Arc<noxa_fetch::FetchClient>,
    llm_chain: Option<noxa_llm::ProviderChain>,
    cloud: Option<CloudClient>,
    store: noxa_store::FilesystemContentStore,
}

/// Timeout for local fetch calls (prevents hanging on tarpitting servers).
const LOCAL_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum poll iterations for research jobs (~10 minutes at 3s intervals).
const RESEARCH_MAX_POLLS: u32 = 200;
#[cfg(not(test))]
const RESEARCH_POLL_INTERVAL: Duration = Duration::from_secs(3);
#[cfg(test)]
const RESEARCH_POLL_INTERVAL: Duration = Duration::from_millis(5);

#[tool_router]
impl NoxaMcp {
    pub async fn new() -> Result<Self, NoxaMcpError> {
        let config = Arc::new(NoxaMcpConfig::from_env()?);
        let store = config.store.clone();
        let fetch_client = noxa_fetch::FetchClient::new(config.fetch.clone())
            .map_err(NoxaMcpError::FetchClientInit)?;

        let llm_chain = build_llm_chain(config.as_ref()).await?;

        let cloud = config
            .cloud_api_key
            .clone()
            .map(CloudClient::new)
            .transpose()?;
        if cloud.is_some() {
            info!("cloud API fallback enabled (NOXA_API_KEY set)");
        } else {
            warn!(
                "NOXA_API_KEY not set -- bot-protected sites will return challenge pages. \
                 Get a key at https://noxa.io"
            );
        }

        Ok(Self {
            tool_router: Self::tool_router(),
            config,
            fetch_client: Arc::new(fetch_client),
            llm_chain,
            cloud,
            store,
        })
    }

    /// Helper: smart fetch with LLM format for extract/summarize tools.
    async fn smart_fetch_llm(&self, url: &str) -> Result<SmartFetchResult, NoxaMcpError> {
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

    fn custom_fetch_config(
        &self,
        browser: noxa_fetch::BrowserProfile,
        cookie_header: Option<String>,
    ) -> Option<noxa_fetch::FetchConfig> {
        let is_default_browser = matches!(browser, noxa_fetch::BrowserProfile::Chrome);
        if is_default_browser && cookie_header.is_none() {
            return None;
        }

        let mut config = self.config.fetch.clone();
        config.browser = browser;
        config
            .headers
            .insert("Accept-Language".to_string(), "en-US,en;q=0.9".to_string());
        if let Some(cookies) = cookie_header {
            config.headers.insert("Cookie".to_string(), cookies);
        }

        Some(config)
    }

    fn build_fetch_client(
        &self,
        config: Option<noxa_fetch::FetchConfig>,
    ) -> Result<Option<noxa_fetch::FetchClient>, NoxaMcpError> {
        config
            .map(noxa_fetch::FetchClient::new)
            .transpose()
            .map_err(NoxaMcpError::FetchClientInit)
    }

    fn map_tool_error(error: NoxaMcpError) -> String {
        error.to_string()
    }

    async fn persist_local_extraction(
        &self,
        url: &str,
        extraction: &noxa_core::ExtractionResult,
    ) -> Result<(), NoxaMcpError> {
        self.store.write(url, extraction).await?;
        Ok(())
    }

    async fn scrape_after_validation(&self, params: ScrapeParams) -> ToolResult {
        let format = params.resolved_format();
        let browser = params.resolved_browser();
        let include = params.include_selectors.unwrap_or_default();
        let exclude = params.exclude_selectors.unwrap_or_default();
        let main_only = params.only_main_content.unwrap_or(false);
        let cookie_header = params
            .cookies
            .as_ref()
            .filter(|c| !c.is_empty())
            .map(|c| c.join("; "));
        let custom_config = self.custom_fetch_config(browser, cookie_header);
        let custom_client = self
            .build_fetch_client(custom_config)
            .map_err(Self::map_tool_error)?;
        let client = custom_client
            .as_ref()
            .unwrap_or_else(|| self.fetch_client.as_ref());

        let formats = [format.as_str()];
        let result = cloud::smart_fetch(
            client,
            self.cloud.as_ref(),
            &params.url,
            &include,
            &exclude,
            main_only,
            &formats,
        )
        .await
        .map_err(Self::map_tool_error)?;

        match result {
            SmartFetchResult::Local(extraction) => {
                self.persist_local_extraction(&params.url, &extraction)
                    .await
                    .map_err(Self::map_tool_error)?;
                let output = match format {
                    ScrapeFormat::Llm => noxa_core::to_llm_text(&extraction, Some(&params.url)),
                    ScrapeFormat::Text => extraction.content.plain_text,
                    ScrapeFormat::Json => to_pretty_json(&extraction, "scrape local extraction")
                        .map_err(Self::map_tool_error)?,
                    ScrapeFormat::Markdown => extraction.content.markdown,
                };
                Ok(output)
            }
            SmartFetchResult::Cloud(resp) => {
                let content = resp
                    .get(format.as_str())
                    .or_else(|| resp.get("markdown"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if content.is_empty() {
                    to_pretty_json(&resp, "scrape cloud response").map_err(Self::map_tool_error)
                } else {
                    Ok(content.to_string())
                }
            }
        }
    }

    async fn extract_after_validation(&self, params: ExtractParams) -> ToolResult {
        if self.llm_chain.is_none() {
            let cloud = self.cloud.as_ref().ok_or(NO_LLM_PROVIDERS_MESSAGE)?;
            let mut body = json!({"url": params.url});
            if let Some(ref schema) = params.schema {
                body["schema"] = json!(schema);
            }
            if let Some(ref prompt) = params.prompt {
                body["prompt"] = json!(prompt);
            }
            let resp = cloud
                .post("extract", body)
                .await
                .map_err(Self::map_tool_error)?;
            return to_pretty_json(&resp, "extract cloud response").map_err(Self::map_tool_error);
        }

        let chain = self.llm_chain.as_ref().unwrap();
        let llm_content = match self
            .smart_fetch_llm(&params.url)
            .await
            .map_err(Self::map_tool_error)?
        {
            SmartFetchResult::Local(extraction) => {
                self.persist_local_extraction(&params.url, &extraction)
                    .await
                    .map_err(Self::map_tool_error)?;
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
                .map_err(|e| Self::map_tool_error(NoxaMcpError::llm(e.to_string())))?
        } else {
            let prompt = params.prompt.as_deref().unwrap();
            noxa_llm::extract::extract_with_prompt(&llm_content, prompt, chain, None)
                .await
                .map_err(|e| Self::map_tool_error(NoxaMcpError::llm(e.to_string())))?
        };

        to_pretty_json(&data, "extract result").map_err(Self::map_tool_error)
    }

    async fn summarize_after_validation(&self, params: SummarizeParams) -> ToolResult {
        if self.llm_chain.is_none() {
            let cloud = self.cloud.as_ref().ok_or(NO_LLM_PROVIDERS_MESSAGE)?;
            let mut body = json!({"url": params.url});
            if let Some(sentences) = params.max_sentences {
                body["max_sentences"] = json!(sentences);
            }
            let resp = cloud
                .post("summarize", body)
                .await
                .map_err(Self::map_tool_error)?;
            let summary = resp.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            if summary.is_empty() {
                return to_pretty_json(&resp, "summarize cloud response")
                    .map_err(Self::map_tool_error);
            }
            return Ok(summary.to_string());
        }

        let chain = self.llm_chain.as_ref().unwrap();
        let llm_content = match self
            .smart_fetch_llm(&params.url)
            .await
            .map_err(Self::map_tool_error)?
        {
            SmartFetchResult::Local(extraction) => {
                self.persist_local_extraction(&params.url, &extraction)
                    .await
                    .map_err(Self::map_tool_error)?;
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
            .map_err(|e| Self::map_tool_error(NoxaMcpError::llm(e.to_string())))
    }

    async fn diff_after_validation(&self, params: DiffParams) -> ToolResult {
        let previous: Option<noxa_core::ExtractionResult> = match params.previous_snapshot {
            Some(ref json) => Some(
                serde_json::from_str(json)
                    .map_err(|e| format!("Failed to parse previous_snapshot JSON: {e}"))?,
            ),
            None => self.store.read(&params.url).await.ok().flatten(),
        };

        let previous = match previous {
            Some(previous) => previous,
            None => {
                info!(url = %params.url, "diff: no previous snapshot — fetching baseline");
                match cloud::smart_fetch(
                    &self.fetch_client,
                    self.cloud.as_ref(),
                    &params.url,
                    &[],
                    &[],
                    false,
                    &["markdown"],
                )
                .await
                {
                    Err(error) => {
                        return Err(format!(
                            "No previous snapshot stored for {url}. Failed to fetch baseline: {error}",
                            url = params.url
                        ));
                    }
                    Ok(SmartFetchResult::Local(_)) => {
                        let stored = self.store.read(&params.url).await.map_err(|error| {
                            format!(
                                "No previous snapshot stored for {url}. Fetched the page but failed to verify the stored baseline: {error}",
                                url = params.url
                            )
                        })?;
                        if stored.is_some() {
                            return Err(format!(
                                "No previous snapshot stored for {url}. The page has been fetched and stored as the baseline — run diff again to compare against this snapshot.",
                                url = params.url
                            ));
                        }
                        return Err(format!(
                            "No previous snapshot stored for {url}. Fetched the page but failed to store baseline. Ensure the content store is writable and retry.",
                            url = params.url
                        ));
                    }
                    Ok(SmartFetchResult::Cloud(_)) => {
                        return Err(format!(
                            "No previous snapshot stored for {url}. The page required cloud fetching (bot protection) and cannot be auto-stored as a baseline. Provide a previous_snapshot parameter explicitly.",
                            url = params.url
                        ));
                    }
                }
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
        .await
        .map_err(Self::map_tool_error)?;

        match result {
            SmartFetchResult::Local(current) => {
                let content_diff = noxa_core::diff::diff(&previous, &current);
                to_pretty_json(&content_diff, "diff result").map_err(Self::map_tool_error)
            }
            SmartFetchResult::Cloud(resp) => {
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
                    domain_data: None,
                    vertical_data: None,
                    structured_data: Vec::new(),
                };

                let content_diff = noxa_core::diff::diff(&previous, &current);
                to_pretty_json(&content_diff, "diff result").map_err(Self::map_tool_error)
            }
        }
    }

    fn research_request(params: &ResearchParams) -> ResearchRequest {
        ResearchRequest {
            query: params.query.clone(),
            deep: params.deep.unwrap_or(false),
            topic: params.topic.clone(),
        }
    }

    async fn research_after_validation(&self, params: ResearchParams) -> ToolResult {
        let cloud = self
            .cloud
            .as_ref()
            .ok_or("Research requires NOXA_API_KEY. Get a key at https://noxa.io")?;
        let research_dir = &self.config.research_dir;
        let request = Self::research_request(&params);

        if let Some((cached, artifacts)) =
            load_cached_research(research_dir, &request).map_err(Self::map_tool_error)?
        {
            info!(query = %params.query, "returning cached research");
            let response = build_research_response(&params.query, &cached, &artifacts, true);
            return to_pretty_json(&response, "cached research response")
                .map_err(Self::map_tool_error);
        }

        let mut body = json!({ "query": params.query });
        if let Some(deep) = params.deep {
            body["deep"] = json!(deep);
        }
        if let Some(ref topic) = params.topic {
            body["topic"] = json!(topic);
        }

        let start_resp = cloud
            .post("research", body)
            .await
            .map_err(Self::map_tool_error)?;
        let job_id = start_resp
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Research API did not return a job ID")?
            .to_string();

        info!(job_id = %job_id, "research job started, polling for completion");
        for poll in 0..RESEARCH_MAX_POLLS {
            tokio::time::sleep(RESEARCH_POLL_INTERVAL).await;

            let status_resp = cloud
                .get(&format!("research/{job_id}"))
                .await
                .map_err(Self::map_tool_error)?;
            let status = status_resp
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            match status {
                "completed" => {
                    let artifacts = save_research(research_dir, &request, &status_resp)
                        .map_err(Self::map_tool_error)?;
                    let response =
                        build_research_response(&params.query, &status_resp, &artifacts, false);
                    return to_pretty_json(&response, "research response")
                        .map_err(Self::map_tool_error);
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

    async fn search_after_validation(&self, params: SearchParams) -> ToolResult {
        let num = params.num_results.unwrap_or(10).clamp(1, 50);

        if let Some(base_url) = self.config.searxng_url.as_ref() {
            let results =
                noxa_fetch::searxng_search(&self.fetch_client, base_url, &params.query, num)
                    .await
                    .map_err(|e| format!("SearXNG search failed: {e}"))?;

            if results.is_empty() {
                return Ok(format!("No results found for: {}", params.query));
            }

            let candidate_urls: Vec<String> =
                results.iter().map(|result| result.url.clone()).collect();
            let validations = collect_valid_urls(&candidate_urls).await;
            let mut validity = std::collections::HashMap::with_capacity(validations.len());
            for (url, result) in validations {
                validity.insert(url, result);
            }

            let mut valid_results = Vec::new();
            for result in results {
                match validity.remove(&result.url).unwrap_or_else(|| {
                    Err(NoxaMcpError::message(format!(
                        "missing validation result for {}",
                        result.url
                    )))
                }) {
                    Ok(()) => valid_results.push(result),
                    Err(error) => warn!("skipping result URL {}: {}", result.url, error),
                }
            }

            let mut out = String::with_capacity(valid_results.len() * 256);
            out.push_str(&format!("Found {} result(s):\n\n", valid_results.len()));
            for (idx, result) in valid_results.iter().enumerate() {
                out.push_str(&format!(
                    "{}. {}\n   {}\n",
                    idx + 1,
                    result.title,
                    result.url
                ));
                if !result.content.is_empty() {
                    out.push_str(&format!("   {}\n", result.content));
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
        let resp = cloud
            .post("search", body)
            .await
            .map_err(Self::map_tool_error)?;

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
            to_pretty_json(&resp, "search cloud response").map_err(Self::map_tool_error)
        }
    }

    /// Scrape a single URL and extract its content as markdown, LLM-optimized text, plain text, or full JSON.
    /// Automatically falls back to the noxa cloud API when bot protection or JS rendering is detected.
    #[tool]
    async fn scrape(&self, Parameters(params): Parameters<ScrapeParams>) -> ToolResult {
        let cookie_header = params
            .cookies
            .as_ref()
            .filter(|c| !c.is_empty())
            .map(|c| c.join("; "));
        let custom_config = self.custom_fetch_config(params.resolved_browser(), cookie_header);
        validate_fetch_url(
            custom_config.as_ref().unwrap_or(&self.config.fetch),
            &params.url,
        )
        .await
        .map_err(Self::map_tool_error)?;
        self.scrape_after_validation(params).await
    }

    /// Crawl a website starting from a seed URL, following links breadth-first up to a configurable depth and page limit.
    #[tool]
    async fn crawl(&self, Parameters(params): Parameters<CrawlParams>) -> ToolResult {
        validate_fetch_url(&self.config.fetch, &params.url)
            .await
            .map_err(Self::map_tool_error)?;

        if let Some(max) = params.max_pages
            && max > 500
        {
            return Err("max_pages cannot exceed 500".into());
        }

        let format = params.resolved_format();

        let concurrency = params.concurrency.unwrap_or(5);
        if concurrency == 0 || concurrency > 20 {
            return Err(format!(
                "concurrency must be between 1 and 20 (got {concurrency})"
            ));
        }

        let config = noxa_fetch::CrawlConfig {
            max_depth: params.depth.unwrap_or(2) as usize,
            max_pages: params.max_pages.unwrap_or(50),
            concurrency,
            use_sitemap: params.use_sitemap.unwrap_or(false),
            fetch: noxa_fetch::FetchConfig {
                store: Some(self.store.clone()),
                ..self.config.fetch.clone()
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
                    ContentFormat::Llm => noxa_core::to_llm_text(extraction, Some(&page.url)),
                    ContentFormat::Text => extraction.content.plain_text.clone(),
                    ContentFormat::Markdown => extraction.content.markdown.clone(),
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
    async fn map(&self, Parameters(params): Parameters<MapParams>) -> ToolResult {
        validate_fetch_url(&self.config.fetch, &params.url)
            .await
            .map_err(Self::map_tool_error)?;
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
    async fn batch(&self, Parameters(params): Parameters<BatchParams>) -> ToolResult {
        if params.urls.is_empty() {
            return Err("urls must not be empty".into());
        }
        if params.urls.len() > 100 {
            return Err("batch is limited to 100 URLs per request".into());
        }
        validate_fetch_urls(&self.config.fetch, &params.urls)
            .await
            .map_err(Self::map_tool_error)?;

        let format = params.resolved_format();
        let concurrency = params.concurrency.unwrap_or(5);
        if concurrency == 0 || concurrency > 20 {
            return Err(format!(
                "concurrency must be between 1 and 20 (got {concurrency})"
            ));
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
                        ContentFormat::Llm => noxa_core::to_llm_text(extraction, Some(&r.url)),
                        ContentFormat::Text => extraction.content.plain_text.clone(),
                        ContentFormat::Markdown => extraction.content.markdown.clone(),
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
    async fn extract(&self, Parameters(params): Parameters<ExtractParams>) -> ToolResult {
        validate_fetch_url(&self.config.fetch, &params.url)
            .await
            .map_err(Self::map_tool_error)?;
        params
            .validate()
            .map_err(|e| Self::map_tool_error(NoxaMcpError::invalid_parameter(e)))?;
        self.extract_after_validation(params).await
    }

    /// Summarize the content of a web page using an LLM.
    /// Falls back to the noxa cloud API when no local LLM is available or bot protection is detected.
    #[tool]
    async fn summarize(&self, Parameters(params): Parameters<SummarizeParams>) -> ToolResult {
        validate_fetch_url(&self.config.fetch, &params.url)
            .await
            .map_err(Self::map_tool_error)?;
        self.summarize_after_validation(params).await
    }

    /// Compare the current content of a URL against a previous extraction snapshot, showing what changed.
    /// Automatically falls back to the noxa cloud API when bot protection is detected.
    #[tool]
    async fn diff(&self, Parameters(params): Parameters<DiffParams>) -> ToolResult {
        validate_fetch_url(&self.config.fetch, &params.url)
            .await
            .map_err(Self::map_tool_error)?;
        self.diff_after_validation(params).await
    }

    /// Extract brand identity (colors, fonts, logo, favicon) from a website's HTML and CSS.
    /// Automatically falls back to the noxa cloud API when bot protection is detected.
    #[tool]
    async fn brand(&self, Parameters(params): Parameters<BrandParams>) -> ToolResult {
        validate_fetch_url(&self.config.fetch, &params.url)
            .await
            .map_err(Self::map_tool_error)?;
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
                    .await
                    .map_err(Self::map_tool_error)?;
                return to_pretty_json(&resp, "brand cloud response").map_err(Self::map_tool_error);
            } else {
                return Err(format!(
                    "Bot protection detected on {}. Set NOXA_API_KEY for automatic cloud bypass. \
                     Get a key at https://noxa.io",
                    params.url
                ));
            }
        }

        let identity = noxa_core::brand::extract_brand(&fetch_result.html, Some(&fetch_result.url));

        to_pretty_json(&identity, "brand result").map_err(Self::map_tool_error)
    }

    /// Run a deep research investigation on a topic or question. Requires NOXA_API_KEY.
    /// Saves full result to ~/.noxa/research/ and returns the file path + key findings.
    /// Checks cache first — same query returns the cached result without spending credits.
    #[tool]
    async fn research(&self, Parameters(params): Parameters<ResearchParams>) -> ToolResult {
        self.research_after_validation(params).await
    }

    /// Search using SearXNG (`SEARXNG_URL`) or cloud (`NOXA_API_KEY`).
    #[tool]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> ToolResult {
        if params.query.trim().is_empty() {
            return Err("query must not be empty".into());
        }
        self.search_after_validation(params).await
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tempfile::tempdir;

    use super::*;
    use crate::test_support::{TestHttpServer, TestResponse};

    fn test_app(
        root: &std::path::Path,
        searxng_url: Option<String>,
        research_dir: Option<PathBuf>,
        cloud_base: Option<String>,
        fetch: Option<noxa_fetch::FetchConfig>,
    ) -> NoxaMcp {
        let store_root = root.join("content");
        let research_dir = research_dir.unwrap_or_else(|| root.join("research"));
        std::fs::create_dir_all(&store_root).unwrap();
        if !research_dir.exists() {
            std::fs::create_dir_all(&research_dir).unwrap();
        }

        let store = noxa_store::FilesystemContentStore::new(&store_root);
        let mut fetch = fetch.unwrap_or_default();
        fetch.store = Some(store.clone());
        let config = Arc::new(NoxaMcpConfig {
            fetch: fetch.clone(),
            store: store.clone(),
            research_dir,
            searxng_url,
            cloud_api_key: cloud_base.as_ref().map(|_| "test-api-key".to_string()),
            llm_provider: None,
            llm_model: None,
            llm_base_url: None,
        });

        NoxaMcp {
            tool_router: NoxaMcp::tool_router(),
            fetch_client: Arc::new(noxa_fetch::FetchClient::new(fetch).unwrap()),
            llm_chain: None,
            cloud: cloud_base
                .map(|base| CloudClient::new_with_base("test-api-key".into(), base).unwrap()),
            store,
            config,
        }
    }

    #[tokio::test]
    async fn scrape_after_validation_persists_local_results() {
        let page = TestHttpServer::spawn(|_| {
            TestResponse::html(
                "<html><head><title>Hello</title></head><body><p>persist me</p></body></html>",
            )
        })
        .await;
        let home = tempdir().unwrap();
        let app = test_app(home.path(), None, None, None, None);
        let url = page.url("/article");

        let output = app
            .scrape_after_validation(ScrapeParams {
                url: url.clone(),
                format: Some(ScrapeFormat::Markdown),
                browser: None,
                cookies: None,
                include_selectors: None,
                exclude_selectors: None,
                only_main_content: None,
            })
            .await
            .unwrap();

        assert!(output.contains("persist me"));
        let stored = app.store.read(&url).await.unwrap();
        assert!(stored.is_some(), "scrape should persist a diff baseline");
    }

    #[tokio::test]
    async fn search_does_not_fetch_result_pages() {
        let search_server = TestHttpServer::spawn(|request| {
            if request.path.starts_with("/search") {
                TestResponse::json(
                    r#"{"results":[{"title":"One","url":"https://example.com/one","content":"snippet one"},{"title":"Two","url":"https://example.com/two","content":"snippet two"}]}"#,
                )
            } else {
                TestResponse::html("unexpected fetch")
            }
        })
        .await;

        let home = tempdir().unwrap();
        let app = test_app(home.path(), Some(search_server.url("")), None, None, None);

        let output = app
            .search(Parameters(SearchParams {
                query: "rust async".into(),
                num_results: Some(2),
            }))
            .await
            .unwrap();

        assert!(output.contains("https://example.com/one"));
        let requests = search_server.requests();
        assert_eq!(
            requests.len(),
            1,
            "search should only call the SearXNG endpoint"
        );
        assert!(
            requests[0].path.starts_with("/search?"),
            "expected a single SearXNG search request, got {:?}",
            requests[0].path
        );
    }

    #[test]
    fn custom_clients_preserve_store_and_proxy_pool() {
        let home = tempdir().unwrap();
        let app = test_app(
            home.path(),
            None,
            None,
            None,
            Some(noxa_fetch::FetchConfig {
                proxy_pool: vec![
                    "http://proxy-1.example:8080".into(),
                    "http://proxy-2.example:8080".into(),
                ],
                ..Default::default()
            }),
        );

        let config = app.custom_fetch_config(
            noxa_fetch::BrowserProfile::Firefox,
            Some("session=abc".into()),
        );
        let client = app.build_fetch_client(config).unwrap().unwrap();

        assert_eq!(client.proxy_pool_size(), 2);
        assert!(
            client.store().is_some(),
            "custom client should preserve store"
        );
    }

    #[tokio::test]
    async fn explicit_ollama_config_builds_non_empty_chain() {
        let home = tempdir().unwrap();
        let store_root = home.path().join("content");
        std::fs::create_dir_all(&store_root).unwrap();
        let store = noxa_store::FilesystemContentStore::new(&store_root);
        let config = NoxaMcpConfig {
            fetch: noxa_fetch::FetchConfig {
                store: Some(store.clone()),
                ..Default::default()
            },
            store,
            research_dir: home.path().join("research"),
            searxng_url: None,
            cloud_api_key: None,
            llm_provider: Some("ollama".into()),
            llm_model: Some("qwen3.5:9b".into()),
            llm_base_url: Some("http://127.0.0.1:11434".into()),
        };

        let chain = build_llm_chain(&config).await.unwrap();

        assert!(chain.is_some(), "explicit ollama config should be honored");
        assert_eq!(chain.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn diff_after_validation_uses_scrape_persisted_baseline() {
        let current = Arc::new(std::sync::Mutex::new("first version".to_string()));
        let page_state = Arc::clone(&current);
        let page = TestHttpServer::spawn(move |_| {
            let body = page_state.lock().unwrap().clone();
            TestResponse::html(format!("<html><body><main>{body}</main></body></html>"))
        })
        .await;

        let home = tempdir().unwrap();
        let app = test_app(home.path(), None, None, None, None);
        let url = page.url("/article");

        app.scrape_after_validation(ScrapeParams {
            url: url.clone(),
            format: Some(ScrapeFormat::Markdown),
            browser: None,
            cookies: None,
            include_selectors: None,
            exclude_selectors: None,
            only_main_content: None,
        })
        .await
        .unwrap();

        *current.lock().unwrap() = "second version".to_string();

        let diff = app
            .diff_after_validation(DiffParams {
                url: url.clone(),
                previous_snapshot: None,
            })
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&diff).unwrap();
        assert_ne!(parsed["status"], "same");
        assert!(
            parsed["text_diff"].as_str().is_some(),
            "expected rendered text diff in response: {parsed}"
        );
    }

    #[tokio::test]
    async fn research_uses_cache_key_and_distinguishes_colliding_queries() {
        let request_count = Arc::new(AtomicUsize::new(0));
        let status_count = Arc::new(AtomicUsize::new(0));
        let request_counter = Arc::clone(&request_count);
        let status_counter = Arc::clone(&status_count);
        let cloud = TestHttpServer::spawn(move |request| {
            if request.path == "/v1/research" {
                request_counter.fetch_add(1, Ordering::SeqCst);
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                let query = body["query"].as_str().unwrap();
                let job_id = if query == "Hello!" { "job-a" } else { "job-b" };
                return TestResponse::json(format!(r#"{{"id":"{job_id}"}}"#));
            }

            if request.path == "/v1/research/job-a" {
                status_counter.fetch_add(1, Ordering::SeqCst);
                return TestResponse::json(
                    r##"{"status":"completed","report":"# alpha","findings":["alpha"],"sources_count":1,"findings_count":1}"##,
                );
            }

            if request.path == "/v1/research/job-b" {
                status_counter.fetch_add(1, Ordering::SeqCst);
                return TestResponse::json(
                    r##"{"status":"completed","report":"# beta","findings":["beta"],"sources_count":1,"findings_count":1}"##,
                );
            }

            TestResponse::text(404, "missing", "text/plain")
        })
        .await;

        let home = tempdir().unwrap();
        let app = test_app(home.path(), None, None, Some(cloud.url("/v1")), None);

        let first = app
            .research_after_validation(ResearchParams {
                query: "Hello!".into(),
                deep: None,
                topic: None,
            })
            .await
            .unwrap();
        let cached = app
            .research_after_validation(ResearchParams {
                query: "Hello!".into(),
                deep: None,
                topic: None,
            })
            .await
            .unwrap();
        let second = app
            .research_after_validation(ResearchParams {
                query: "Hello?".into(),
                deep: None,
                topic: None,
            })
            .await
            .unwrap();

        let first_json: serde_json::Value = serde_json::from_str(&first).unwrap();
        let cached_json: serde_json::Value = serde_json::from_str(&cached).unwrap();
        let second_json: serde_json::Value = serde_json::from_str(&second).unwrap();

        assert_eq!(request_count.load(Ordering::SeqCst), 2);
        assert_eq!(status_count.load(Ordering::SeqCst), 2);
        assert_eq!(cached_json["cached"], true);
        assert_ne!(first_json["json_file"], second_json["json_file"]);
        assert_eq!(first_json["findings"][0], "alpha");
        assert_eq!(second_json["findings"][0], "beta");
        assert!(PathBuf::from(first_json["report_file"].as_str().unwrap()).exists());
    }

    #[tokio::test]
    async fn research_surfaces_artifact_write_failures() {
        let cloud = TestHttpServer::spawn(|request| {
            if request.path == "/v1/research" {
                return TestResponse::json(r#"{"id":"job-write-fail"}"#);
            }
            if request.path == "/v1/research/job-write-fail" {
                return TestResponse::json(
                    r##"{"status":"completed","report":"# blocked","findings":["x"],"sources_count":1,"findings_count":1}"##,
                );
            }
            TestResponse::text(404, "missing", "text/plain")
        })
        .await;

        let home = tempdir().unwrap();
        let invalid_research_dir = home.path().join("not-a-directory");
        std::fs::write(&invalid_research_dir, "x").unwrap();
        let app = test_app(
            home.path(),
            None,
            Some(invalid_research_dir),
            Some(cloud.url("/v1")),
            None,
        );

        let err = app
            .research_after_validation(ResearchParams {
                query: "blocked".into(),
                deep: None,
                topic: None,
            })
            .await
            .unwrap_err();

        assert!(err.contains("failed to write"));
    }
}
