use super::*;

#[derive(Parser)]
#[command(name = "noxa", about = "Extract web content for LLMs", version)]
pub(crate) struct Cli {
    /// Path to config.json (default: ./config.json, override with NOXA_CONFIG env var)
    #[arg(long, global = true)]
    pub(crate) config: Option<String>,

    /// URLs to fetch (multiple allowed)
    #[arg()]
    pub(crate) urls: Vec<String>,

    /// File with URLs (one per line)
    #[arg(long)]
    pub(crate) urls_file: Option<String>,

    /// Output format (markdown, json, text, llm, html)
    #[arg(short, long, default_value = "markdown")]
    pub(crate) format: OutputFormat,

    /// Browser to impersonate
    #[arg(short, long, default_value = "chrome")]
    pub(crate) browser: Browser,

    /// Proxy URL (http://user:pass@host:port or socks5://host:port)
    #[arg(short, long, env = "NOXA_PROXY")]
    pub(crate) proxy: Option<String>,

    /// File with proxies (host:port:user:pass, one per line). Rotates per request.
    #[arg(long, env = "NOXA_PROXY_FILE")]
    pub(crate) proxy_file: Option<String>,

    /// Request timeout in seconds
    #[arg(short, long, default_value = "30")]
    pub(crate) timeout: u64,

    /// Extract from local HTML file instead of fetching
    #[arg(long)]
    pub(crate) file: Option<String>,

    /// Read HTML from stdin
    #[arg(long)]
    pub(crate) stdin: bool,

    /// Include metadata in output (always included in JSON)
    #[arg(long)]
    pub(crate) metadata: bool,

    /// Output raw fetched HTML instead of extracting
    #[arg(long)]
    pub(crate) raw_html: bool,

    /// CSS selectors to include (comma-separated, e.g. "article,.content")
    #[arg(long)]
    pub(crate) include: Option<String>,

    /// CSS selectors to exclude (comma-separated, e.g. "nav,.sidebar,footer")
    #[arg(long)]
    pub(crate) exclude: Option<String>,

    /// Only extract main content (article/main element)
    #[arg(long)]
    pub(crate) only_main_content: bool,

    /// Custom headers (repeatable, e.g. -H "Cookie: foo=bar")
    #[arg(short = 'H', long = "header")]
    pub(crate) headers: Vec<String>,

    /// Cookie string (shorthand for -H "Cookie: ...")
    #[arg(long)]
    pub(crate) cookie: Option<String>,

    /// JSON cookie file (Chrome extension format: [{name, value, domain, ...}])
    #[arg(long)]
    pub(crate) cookie_file: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub(crate) verbose: bool,

    /// Compare against a previous JSON snapshot
    #[arg(long)]
    pub(crate) diff_with: Option<String>,

    /// Watch a URL for changes. Checks at the specified interval and reports diffs.
    #[arg(long)]
    pub(crate) watch: bool,

    /// Watch interval in seconds [default: 300]
    #[arg(long, default_value = "300")]
    pub(crate) watch_interval: u64,

    /// Command to run when changes are detected (receives diff JSON on stdin)
    #[arg(long)]
    pub(crate) on_change: Option<String>,

    /// Webhook URL: POST a JSON payload when an operation completes.
    /// Works with crawl, batch, watch (on change), and single URL modes.
    #[arg(long, env = "NOXA_WEBHOOK_URL")]
    pub(crate) webhook: Option<String>,

    /// Extract brand identity (colors, fonts, logo)
    #[arg(long)]
    pub(crate) brand: bool,

    // -- PDF options --
    /// PDF extraction mode: auto (error on empty) or fast (return whatever text is found)
    #[arg(long, default_value = "auto")]
    pub(crate) pdf_mode: PdfModeArg,

    // -- Crawl options --
    /// Enable recursive crawling of same-domain links. Runs in background by default.
    /// Use --wait to block and stream live progress.
    #[arg(long)]
    pub(crate) crawl: bool,

    /// Block and stream live crawl progress instead of running in background.
    #[arg(long)]
    pub(crate) wait: bool,

    /// Max crawl depth [default: 1]
    #[arg(long, default_value = "1")]
    pub(crate) depth: usize,

    /// Max pages to crawl [default: 20]
    #[arg(long, default_value = "20")]
    pub(crate) max_pages: usize,

    /// Max concurrent requests [default: 5]
    #[arg(long, default_value = "5")]
    pub(crate) concurrency: usize,

    /// Delay between requests in ms [default: 100]
    #[arg(long, default_value = "100")]
    pub(crate) delay: u64,

    /// Only crawl URLs matching this path prefix
    #[arg(long)]
    pub(crate) path_prefix: Option<String>,

    /// Glob patterns for crawl URL paths to include (comma-separated, e.g. "/api/*,/guides/**")
    #[arg(long)]
    pub(crate) include_paths: Option<String>,

    /// Glob patterns for crawl URL paths to exclude (comma-separated, e.g. "/changelog/*,/blog/*")
    #[arg(long)]
    pub(crate) exclude_paths: Option<String>,

    /// Path to save/resume crawl state. On Ctrl+C: saves progress. On start: resumes if file exists.
    #[arg(long)]
    pub(crate) crawl_state: Option<PathBuf>,

    /// Seed crawl frontier from sitemap discovery (robots.txt + /sitemap.xml)
    #[arg(long)]
    pub(crate) sitemap: bool,

    /// Discover URLs from sitemap and print them (one per line; JSON array with --format json)
    #[arg(long)]
    pub(crate) map: bool,

    // -- LLM options --
    /// Extract structured JSON using LLM (pass a JSON schema string or @file)
    #[arg(long)]
    pub(crate) extract_json: Option<String>,

    /// Extract using natural language prompt
    #[arg(long)]
    pub(crate) extract_prompt: Option<String>,

    /// Summarize content using LLM (optional: number of sentences, default 3)
    #[arg(long, num_args = 0..=1, default_missing_value = "3")]
    pub(crate) summarize: Option<usize>,

    /// Force a specific LLM provider (gemini, ollama, openai, anthropic)
    #[arg(long)]
    pub(crate) llm_provider: Option<String>,

    /// Override the LLM model name
    #[arg(long)]
    pub(crate) llm_model: Option<String>,

    /// Override the LLM base URL (Ollama or OpenAI-compatible)
    #[arg(long, env = "NOXA_LLM_BASE_URL")]
    pub(crate) llm_base_url: Option<String>,

    // -- Cloud API options --
    /// Noxa Cloud API key for automatic fallback on bot-protected or JS-rendered sites
    #[arg(long, env = "NOXA_API_KEY")]
    pub(crate) api_key: Option<String>,

    /// Force all requests through the cloud API (skip local extraction)
    #[arg(long)]
    pub(crate) cloud: bool,

    /// Cloud provider to use (e.g. "gcp", "aws")
    #[arg(long, env = "NOXA_CLOUD_PROVIDER")]
    pub(crate) cloud_provider: Option<String>,

    /// Cloud project ID
    #[arg(long, env = "NOXA_CLOUD_PROJECT")]
    pub(crate) cloud_project: Option<String>,

    /// Cloud zone or region
    #[arg(long, env = "NOXA_CLOUD_ZONE")]
    pub(crate) cloud_zone: Option<String>,

    /// Cloud cluster name
    #[arg(long, env = "NOXA_CLOUD_CLUSTER")]
    pub(crate) cloud_cluster: Option<String>,

    /// Path to cloud service account key file
    #[arg(long, env = "NOXA_CLOUD_SERVICE_ACCOUNT_KEY")]
    pub(crate) cloud_service_account_key: Option<String>,

    /// Disable cloud features
    #[arg(long)]
    pub(crate) cloud_disabled: bool,

    /// Run deep research on a topic via the cloud API. Requires --api-key.
    /// Saves full result (report + sources + findings) to a JSON file.
    #[arg(long)]
    pub(crate) research: Option<String>,

    /// Enable deep research mode (longer, more thorough report). Used with --research.
    #[arg(long)]
    pub(crate) deep: bool,

    /// Search via SearXNG (SEARXNG_URL) or noxa cloud (NOXA_API_KEY).
    #[arg(long)]
    pub(crate) search: Option<String>,

    /// Number of search results (1-50, default: 10).
    #[arg(long, default_value = "10")]
    pub(crate) num_results: u32,

    /// Print snippets only; skip scraping result URLs.
    #[arg(long)]
    pub(crate) no_scrape: bool,

    /// Disable automatic content store persistence (~/.noxa/content/).
    /// Also respected via the NOXA_NO_STORE environment variable.
    #[arg(long, env = "NOXA_NO_STORE")]
    pub(crate) no_store: bool,

    /// Concurrency for scraping search result URLs (default: 3).
    #[arg(long, default_value = "3")]
    pub(crate) num_scrape_concurrency: usize,

    /// Search through locally stored docs (~/.noxa/content/) with ripgrep.
    /// Falls back to built-in grep if rg is not installed.
    /// Example: noxa --grep "authentication"
    #[arg(long)]
    pub(crate) grep: Option<String>,

    /// List locally stored docs. No value = all domains. Pass a domain to list
    /// its individual docs with URL → path mapping.
    /// Example: noxa --list   or   noxa --list code.claude.com
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    pub(crate) list: Option<String>,

    /// Show status of a background crawl. Pass the domain or URL.
    /// Example: noxa --status code.claude.com
    #[arg(long)]
    pub(crate) status: Option<String>,

    /// Re-fetch all cached docs for a stored domain.
    /// Example: noxa --refresh docs.rust-lang.org
    #[arg(long)]
    pub(crate) refresh: Option<String>,

    /// Return a cached doc by exact URL or fuzzy query.
    /// Example: noxa --retrieve https://code.claude.com/docs/en/setup
    /// Example: noxa --retrieve "claude code setup"
    #[arg(long)]
    pub(crate) retrieve: Option<String>,

    /// Output directory: save each page to a separate file instead of stdout.
    /// Works with --crawl, batch (multiple URLs), and single URL mode.
    /// Filenames are derived from URL paths (e.g. /docs/api -> docs/api.md).
    #[arg(long)]
    pub(crate) output_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OutputFormat {
    Markdown,
    Json,
    Text,
    Llm,
    Html,
}

#[derive(Clone, Debug, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Browser {
    Chrome,
    Firefox,
    Random,
}

#[derive(Clone, Debug, ValueEnum, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PdfModeArg {
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
