/// MCP server implementation for noxa.
/// Exposes web extraction capabilities as tools for AI agents.
///
/// Uses a local-first architecture: fetches pages directly, then falls back
/// to the noxa cloud API (api.noxa.io) when bot protection or
/// JS rendering is detected. Set NOXA_API_KEY for automatic fallback.
mod bootstrap;
mod content_tools;
mod intelligence_tools;
mod research;

use std::sync::Arc;
use std::time::{Duration, Instant};

use noxa_store::{parse_http_url, validate_public_http_url};
use rmcp::ErrorData;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, tool, tool_router};

use crate::cloud::CloudClient;
use crate::tools::{
    BatchParams, BrandParams, CrawlParams, DiffParams, ExtractParams, MapParams, ResearchParams,
    ScrapeParams, SearchParams, SummarizeParams,
};

const NO_LLM_PROVIDERS_MESSAGE: &str = "No LLM providers available (priority: Gemini CLI -> OpenAI -> Ollama -> Anthropic). Install gemini on PATH, set OPENAI_API_KEY, OLLAMA_HOST / OLLAMA_MODEL, or ANTHROPIC_API_KEY, or set NOXA_API_KEY for cloud fallback.";
const LOCAL_FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const RESEARCH_MAX_POLLS: u32 = 200;

pub struct NoxaMcp {
    tool_router: ToolRouter<Self>,
    fetch_client: Arc<noxa_fetch::FetchClient>,
    llm_chain: Option<noxa_llm::ProviderChain>,
    cloud: Option<CloudClient>,
    store: noxa_store::FilesystemContentStore,
}

fn parse_browser(browser: Option<&str>) -> noxa_fetch::BrowserProfile {
    match browser {
        Some("firefox") => noxa_fetch::BrowserProfile::Firefox,
        Some("random") => noxa_fetch::BrowserProfile::Random,
        _ => noxa_fetch::BrowserProfile::Chrome,
    }
}

async fn validate_url(url: &str) -> Result<(), String> {
    validate_public_http_url(url).await
}

#[tool_router]
impl NoxaMcp {
    #[tool]
    async fn scrape(&self, Parameters(params): Parameters<ScrapeParams>) -> Result<String, String> {
        self.scrape_impl(params).await
    }

    #[tool]
    async fn crawl(&self, Parameters(params): Parameters<CrawlParams>) -> Result<String, String> {
        self.crawl_impl(params).await
    }

    #[tool]
    async fn map(&self, Parameters(params): Parameters<MapParams>) -> Result<String, String> {
        self.map_impl(params).await
    }

    #[tool]
    async fn batch(&self, Parameters(params): Parameters<BatchParams>) -> Result<String, String> {
        self.batch_impl(params).await
    }

    #[tool]
    async fn extract(
        &self,
        Parameters(params): Parameters<ExtractParams>,
    ) -> Result<String, String> {
        self.extract_impl(params).await
    }

    #[tool]
    async fn summarize(
        &self,
        Parameters(params): Parameters<SummarizeParams>,
    ) -> Result<String, String> {
        self.summarize_impl(params).await
    }

    #[tool]
    async fn diff(&self, Parameters(params): Parameters<DiffParams>) -> Result<String, String> {
        self.diff_impl(params).await
    }

    #[tool]
    async fn brand(&self, Parameters(params): Parameters<BrandParams>) -> Result<String, String> {
        self.brand_impl(params).await
    }

    #[tool]
    async fn research(
        &self,
        Parameters(params): Parameters<ResearchParams>,
    ) -> Result<String, String> {
        self.research_impl(params).await
    }

    #[tool]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> Result<String, String> {
        self.search_impl(params).await
    }
}

impl ServerHandler for NoxaMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("noxa-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(String::from(
                "Noxa MCP server -- web content extraction for AI agents. \
                 Tools: scrape, crawl, map, batch, extract, summarize, diff, brand, research, search.",
            ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let tool = request.name.to_string();
        let started = Instant::now();
        let tcc = ToolCallContext::new(self, request, context);
        let result = self.tool_router.call(tcc).await;
        let duration_ms = started.elapsed().as_millis() as u64;
        match &result {
            Ok(_) => tracing::info!(tool = %tool, duration_ms, "tool ok"),
            Err(e) => tracing::warn!(tool = %tool, duration_ms, error = %e, "tool err"),
        }
        result
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: self.tool_router.list_all(),
            meta: None,
            next_cursor: None,
        })
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned()
    }
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
        assert!(
            validate_url("http://169.254.169.254/latest/meta-data/")
                .await
                .is_err()
        );
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
        assert!(
            validate_url("http://[::ffff:169.254.169.254]/latest/meta-data/")
                .await
                .is_err()
        );
        assert!(validate_url("http://[::ffff:10.0.0.1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_accepts_public_ip() {
        assert!(validate_url("http://8.8.8.8/").await.is_ok());
        assert!(validate_url("http://1.1.1.1/").await.is_ok());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_link_local_and_ula() {
        assert!(validate_url("http://[fe80::1]/").await.is_err());
        assert!(validate_url("http://[fc00::1]/").await.is_err());
    }

    #[test]
    fn test_num_results_clamp() {
        assert_eq!(0_u32.clamp(1, 50), 1);
        assert_eq!(100_u32.clamp(1, 50), 50);
        assert_eq!(10_u32.clamp(1, 50), 10);
    }
}
