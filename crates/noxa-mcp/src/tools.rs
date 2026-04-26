/// Tool parameter structs for MCP tool inputs.
/// Each struct derives JsonSchema for automatic schema generation,
/// and Deserialize for parsing from MCP tool call arguments.
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScrapeFormat {
    #[default]
    Markdown,
    Llm,
    Text,
    Json,
}

impl ScrapeFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Llm => "llm",
            Self::Text => "text",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentFormat {
    #[default]
    Markdown,
    Llm,
    Text,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BrowserParam {
    #[default]
    Chrome,
    Firefox,
    Random,
}

impl From<BrowserParam> for noxa_fetch::BrowserProfile {
    fn from(value: BrowserParam) -> Self {
        match value {
            BrowserParam::Chrome => Self::Chrome,
            BrowserParam::Firefox => Self::Firefox,
            BrowserParam::Random => Self::Random,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScrapeParams {
    /// URL to scrape
    pub url: String,
    /// Output format: "markdown" (default), "llm", "text", or "json"
    pub format: Option<ScrapeFormat>,
    /// CSS selectors to include (only extract matching elements)
    pub include_selectors: Option<Vec<String>>,
    /// CSS selectors to exclude from output
    pub exclude_selectors: Option<Vec<String>>,
    /// If true, extract only the main content (article/main element)
    pub only_main_content: Option<bool>,
    /// Browser profile: "chrome" (default), "firefox", or "random"
    pub browser: Option<BrowserParam>,
    /// Cookies to send with the request (e.g. ["name=value", "session=abc123"])
    pub cookies: Option<Vec<String>>,
    /// Optional vertical extractor name. Use the extractors tool to list valid values.
    pub extractor: Option<String>,
}

impl ScrapeParams {
    pub fn resolved_format(&self) -> ScrapeFormat {
        self.format.unwrap_or_default()
    }

    pub fn resolved_browser(&self) -> noxa_fetch::BrowserProfile {
        self.browser.unwrap_or_default().into()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CrawlParams {
    /// Seed URL to start crawling from
    pub url: String,
    /// Maximum link depth to follow (default: 2)
    pub depth: Option<u32>,
    /// Maximum number of pages to crawl (default: 50)
    pub max_pages: Option<usize>,
    /// Number of concurrent requests (default: 5)
    pub concurrency: Option<usize>,
    /// Seed the frontier from sitemap discovery before crawling
    pub use_sitemap: Option<bool>,
    /// Output format for each page: "markdown" (default), "llm", "text"
    pub format: Option<ContentFormat>,
}

impl CrawlParams {
    pub fn resolved_format(&self) -> ContentFormat {
        self.format.unwrap_or_default()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MapParams {
    /// Base URL to discover sitemaps from (e.g. `<https://example.com>`)
    pub url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchParams {
    /// List of URLs to extract content from
    pub urls: Vec<String>,
    /// Output format: "markdown" (default), "llm", "text"
    pub format: Option<ContentFormat>,
    /// Number of concurrent requests (default: 5)
    pub concurrency: Option<usize>,
}

impl BatchParams {
    pub fn resolved_format(&self) -> ContentFormat {
        self.format.unwrap_or_default()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtractParams {
    /// URL to fetch and extract structured data from
    pub url: String,
    /// Natural language prompt describing what to extract
    pub prompt: Option<String>,
    /// JSON schema describing the structure to extract
    pub schema: Option<serde_json::Value>,
}

impl ExtractParams {
    pub fn validate(&self) -> Result<(), String> {
        match (self.schema.as_ref(), self.prompt.as_deref()) {
            (Some(_), Some(_)) => {
                Err("Provide exactly one of 'schema' or 'prompt', not both.".into())
            }
            (None, None) => Err("Either 'schema' or 'prompt' is required for extraction.".into()),
            (_, Some(prompt)) if prompt.trim().is_empty() => {
                Err("'prompt' must not be blank.".into())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SummarizeParams {
    /// URL to fetch and summarize
    pub url: String,
    /// Number of sentences in the summary (default: 3)
    pub max_sentences: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiffParams {
    /// URL to fetch current content from
    pub url: String,
    /// Optional. If provided, must be a JSON-serialized ExtractionResult from a
    /// previous scrape call. If omitted, the previous snapshot is loaded from
    /// the local ContentStore (~/.noxa/content/). Requires the URL to have been
    /// scraped at least once. Returns an error if no stored snapshot exists, but
    /// also fetches and stores the current content as the baseline for future diffs.
    ///
    /// NOTE: this field changed from required to optional — existing MCP clients
    /// that pass previous_snapshot JSON continue to work unchanged (backward-compatible).
    pub previous_snapshot: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrandParams {
    /// URL to extract brand identity from
    pub url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResearchParams {
    /// Research query or question to investigate
    pub query: String,
    /// Enable deep research mode for more thorough investigation (default: false)
    pub deep: Option<bool>,
    /// Topic hint to guide research focus (e.g. "technology", "finance", "science")
    pub topic: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    /// Search query
    pub query: String,
    /// Number of results to return (default: 10)
    pub num_results: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scrape_rejects_unknown_format() {
        let err = serde_json::from_value::<ScrapeParams>(json!({
            "url": "https://example.com",
            "format": "html"
        }))
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown variant"));
    }

    #[test]
    fn scrape_accepts_explicit_extractor() {
        let params = serde_json::from_value::<ScrapeParams>(json!({
            "url": "https://github.com/jmagar/noxa",
            "extractor": "github_repo"
        }))
        .unwrap();

        assert_eq!(params.extractor.as_deref(), Some("github_repo"));
    }

    #[test]
    fn batch_rejects_json_format() {
        let err = serde_json::from_value::<BatchParams>(json!({
            "urls": ["https://example.com"],
            "format": "json"
        }))
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown variant"));
    }

    #[test]
    fn extract_requires_exactly_one_of_schema_or_prompt() {
        let missing = serde_json::from_value::<ExtractParams>(json!({
            "url": "https://example.com"
        }))
        .unwrap();
        assert!(missing.validate().is_err());

        let both = serde_json::from_value::<ExtractParams>(json!({
            "url": "https://example.com",
            "prompt": "extract fields",
            "schema": { "type": "object" }
        }))
        .unwrap();
        assert!(both.validate().is_err());

        let prompt_only = serde_json::from_value::<ExtractParams>(json!({
            "url": "https://example.com",
            "prompt": "extract fields"
        }))
        .unwrap();
        assert!(prompt_only.validate().is_ok());
    }

    #[test]
    fn extract_rejects_blank_prompt() {
        let params = serde_json::from_value::<ExtractParams>(json!({
            "url": "https://example.com",
            "prompt": "   "
        }))
        .unwrap();

        assert!(params.validate().is_err());
    }
}
