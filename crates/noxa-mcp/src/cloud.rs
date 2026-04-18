/// Cloud API fallback for protected sites.
///
/// When local fetch returns a challenge page, this module retries
/// via api.noxa.io. Requires NOXA_API_KEY to be set.
use std::time::Duration;

use serde_json::{Value, json};
use tracing::info;

use crate::error::NoxaMcpError;

const API_BASE: &str = "https://api.noxa.io/v1";

/// Lightweight client for the noxa cloud API.
pub struct CloudClient {
    api_key: String,
    http: reqwest::Client,
}

impl CloudClient {
    pub fn new(api_key: String) -> Result<Self, NoxaMcpError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(NoxaMcpError::CloudClientInit)?;
        Ok(Self { api_key, http })
    }

    /// Scrape a URL via the cloud API. Returns the response JSON.
    pub async fn scrape(
        &self,
        url: &str,
        formats: &[&str],
        include_selectors: &[String],
        exclude_selectors: &[String],
        only_main_content: bool,
    ) -> Result<Value, NoxaMcpError> {
        let mut body = json!({
            "url": url,
            "formats": formats,
        });

        if only_main_content {
            body["only_main_content"] = json!(true);
        }
        if !include_selectors.is_empty() {
            body["include_selectors"] = json!(include_selectors);
        }
        if !exclude_selectors.is_empty() {
            body["exclude_selectors"] = json!(exclude_selectors);
        }

        self.post("scrape", body).await
    }

    /// Generic POST to the cloud API.
    pub async fn post(&self, endpoint: &str, body: Value) -> Result<Value, NoxaMcpError> {
        let resp = self
            .http
            .post(format!("{API_BASE}/{endpoint}"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| NoxaMcpError::cloud(format!("request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            let truncated = truncate_error(&text);
            return Err(NoxaMcpError::cloud(format!("error {status}: {truncated}")));
        }

        resp.json::<Value>()
            .await
            .map_err(|e| NoxaMcpError::cloud(format!("response parse failed: {e}")))
    }

    /// Generic GET from the cloud API.
    pub async fn get(&self, endpoint: &str) -> Result<Value, NoxaMcpError> {
        let resp = self
            .http
            .get(format!("{API_BASE}/{endpoint}"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| NoxaMcpError::cloud(format!("request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            let truncated = truncate_error(&text);
            return Err(NoxaMcpError::cloud(format!("error {status}: {truncated}")));
        }

        resp.json::<Value>()
            .await
            .map_err(|e| NoxaMcpError::cloud(format!("response parse failed: {e}")))
    }
}

/// Truncate error body to avoid flooding logs with huge HTML responses.
fn truncate_error(text: &str) -> &str {
    const MAX_LEN: usize = 500;
    match text.char_indices().nth(MAX_LEN) {
        Some((byte_pos, _)) => &text[..byte_pos],
        None => text,
    }
}

/// Check if fetched HTML looks like a bot protection challenge page.
/// Detects common bot protection challenge pages.
pub fn is_bot_protected(html: &str, headers: &noxa_fetch::HeaderMap) -> bool {
    let html_lower = html.to_lowercase();

    // Cloudflare challenge page
    if html_lower.contains("_cf_chl_opt") || html_lower.contains("challenge-platform") {
        return true;
    }

    // Cloudflare "checking your browser" spinner
    if (html_lower.contains("just a moment") || html_lower.contains("checking your browser"))
        && html_lower.contains("cf-spinner")
    {
        return true;
    }

    // Cloudflare Turnstile (only on short pages = challenge, not embedded on real content)
    if (html_lower.contains("cf-turnstile")
        || html_lower.contains("challenges.cloudflare.com/turnstile"))
        && html.len() < 100_000
    {
        return true;
    }

    // DataDome
    if html_lower.contains("geo.captcha-delivery.com")
        || html_lower.contains("captcha-delivery.com/captcha")
    {
        return true;
    }

    // AWS WAF
    if html_lower.contains("awswaf-captcha") || html_lower.contains("aws-waf-client-browser") {
        return true;
    }

    // hCaptcha blocking page
    if html_lower.contains("hcaptcha.com")
        && html_lower.contains("h-captcha")
        && html.len() < 50_000
    {
        return true;
    }

    // Cloudflare via headers + challenge body
    let has_cf_headers = headers.get("cf-ray").is_some() || headers.get("cf-mitigated").is_some();
    if has_cf_headers
        && (html_lower.contains("just a moment") || html_lower.contains("checking your browser"))
    {
        return true;
    }

    false
}

/// Check if a page likely needs JS rendering (SPA with almost no text content).
pub fn needs_js_rendering(word_count: usize, html: &str) -> bool {
    let has_scripts = html.contains("<script");

    // Tier 1: almost no extractable text from a large page
    if word_count < 50 && html.len() > 5_000 && has_scripts {
        return true;
    }

    // Tier 2: SPA framework detected with suspiciously low content-to-HTML ratio
    if word_count < 800 && html.len() > 50_000 && has_scripts {
        let html_lower = html.to_lowercase();
        let has_spa_marker = html_lower.contains("react-app")
            || html_lower.contains("id=\"__next\"")
            || html_lower.contains("id=\"root\"")
            || html_lower.contains("id=\"app\"")
            || html_lower.contains("__next_data__")
            || html_lower.contains("nuxt")
            || html_lower.contains("ng-app");

        if has_spa_marker {
            return true;
        }
    }

    false
}

/// Result of a smart fetch: either local extraction or cloud API response.
pub enum SmartFetchResult {
    /// Successfully extracted locally.
    Local(Box<noxa_core::ExtractionResult>),
    /// Fell back to cloud API. Contains the API response JSON.
    Cloud(Value),
}

/// Try local fetch first, fall back to cloud API if bot-protected or JS-rendered.
///
/// Returns the extraction result (local) or the cloud API response JSON.
/// If no API key is configured and local fetch is blocked, returns an error
/// with a helpful message.
pub async fn smart_fetch(
    client: &noxa_fetch::FetchClient,
    cloud: Option<&CloudClient>,
    url: &str,
    include_selectors: &[String],
    exclude_selectors: &[String],
    only_main_content: bool,
    formats: &[&str],
) -> Result<SmartFetchResult, NoxaMcpError> {
    // Step 1: Try local fetch (with timeout to avoid hanging on slow servers)
    let fetch_result = tokio::time::timeout(Duration::from_secs(30), client.fetch(url))
        .await
        .map_err(|_| NoxaMcpError::message(format!("Fetch timed out after 30s for {url}")))?
        .map_err(NoxaMcpError::Fetch)?;

    // Step 2: Check for bot protection
    if is_bot_protected(&fetch_result.html, &fetch_result.headers) {
        info!(url, "bot protection detected, falling back to cloud API");
        return cloud_fallback(
            cloud,
            url,
            include_selectors,
            exclude_selectors,
            only_main_content,
            formats,
        )
        .await;
    }

    // Step 3: Extract locally
    let options = noxa_core::ExtractionOptions {
        include_selectors: include_selectors.to_vec(),
        exclude_selectors: exclude_selectors.to_vec(),
        only_main_content,
        include_raw_html: false,
    };

    let extraction =
        noxa_core::extract_with_options(&fetch_result.html, Some(&fetch_result.url), &options)
            .map_err(NoxaMcpError::Extract)?;

    // Step 4: Check for JS-rendered pages (low content from large HTML)
    if needs_js_rendering(extraction.metadata.word_count, &fetch_result.html) {
        info!(
            url,
            word_count = extraction.metadata.word_count,
            html_len = fetch_result.html.len(),
            "JS-rendered page detected, falling back to cloud API"
        );
        return cloud_fallback(
            cloud,
            url,
            include_selectors,
            exclude_selectors,
            only_main_content,
            formats,
        )
        .await;
    }

    Ok(SmartFetchResult::Local(Box::new(extraction)))
}

async fn cloud_fallback(
    cloud: Option<&CloudClient>,
    url: &str,
    include_selectors: &[String],
    exclude_selectors: &[String],
    only_main_content: bool,
    formats: &[&str],
) -> Result<SmartFetchResult, NoxaMcpError> {
    match cloud {
        Some(c) => {
            let resp = c
                .scrape(
                    url,
                    formats,
                    include_selectors,
                    exclude_selectors,
                    only_main_content,
                )
                .await?;
            info!(url, "cloud API fallback successful");
            Ok(SmartFetchResult::Cloud(resp))
        }
        None => Err(NoxaMcpError::cloud(format!(
            "Bot protection detected on {url}. Set NOXA_API_KEY for automatic cloud bypass. \
             Get a key at https://noxa.io"
        ))),
    }
}
