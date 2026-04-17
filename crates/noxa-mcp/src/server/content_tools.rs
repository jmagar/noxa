use serde_json::json;
use tracing::info;

use crate::cloud::{self, SmartFetchResult};
use crate::server::{LOCAL_FETCH_TIMEOUT, NoxaMcp, parse_browser, validate_url};
use crate::tools::{BatchParams, BrandParams, CrawlParams, DiffParams, MapParams, ScrapeParams};

impl NoxaMcp {
    pub(super) async fn scrape_impl(&self, params: ScrapeParams) -> Result<String, String> {
        validate_url(&params.url).await?;
        let format = params.format.as_deref().unwrap_or("markdown");
        let browser = parse_browser(params.browser.as_deref());
        let include = params.include_selectors.unwrap_or_default();
        let exclude = params.exclude_selectors.unwrap_or_default();
        let main_only = params.only_main_content.unwrap_or(false);

        let cookie_header = params
            .cookies
            .as_ref()
            .filter(|c| !c.is_empty())
            .map(|c| c.join("; "));

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
            SmartFetchResult::Local(extraction) => Ok(match format {
                "llm" => noxa_core::to_llm_text(&extraction, Some(&params.url)),
                "text" => extraction.content.plain_text,
                "json" => serde_json::to_string_pretty(&extraction).unwrap_or_default(),
                _ => extraction.content.markdown,
            }),
            SmartFetchResult::Cloud(resp) => {
                let content = resp
                    .get(format)
                    .or_else(|| resp.get("markdown"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if content.is_empty() {
                    Ok(serde_json::to_string_pretty(&resp).unwrap_or_default())
                } else {
                    Ok(content.to_string())
                }
            }
        }
    }

    pub(super) async fn crawl_impl(&self, params: CrawlParams) -> Result<String, String> {
        validate_url(&params.url).await?;

        if let Some(max) = params.max_pages
            && max > 500
        {
            return Err("max_pages cannot exceed 500".into());
        }

        let format = params.format.as_deref().unwrap_or("markdown");
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

    pub(super) async fn map_impl(&self, params: MapParams) -> Result<String, String> {
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

    pub(super) async fn batch_impl(&self, params: BatchParams) -> Result<String, String> {
        if params.urls.is_empty() {
            return Err("urls must not be empty".into());
        }
        if params.urls.len() > 100 {
            return Err("batch is limited to 100 URLs per request".into());
        }
        for url in &params.urls {
            validate_url(url).await?;
        }

        let format = params.format.as_deref().unwrap_or("markdown");
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
        for result in &results {
            output.push_str(&format!("--- {} ---\n", result.url));
            match &result.result {
                Ok(extraction) => {
                    let content = match format {
                        "llm" => noxa_core::to_llm_text(extraction, Some(&result.url)),
                        "text" => extraction.content.plain_text.clone(),
                        _ => extraction.content.markdown.clone(),
                    };
                    output.push_str(&content);
                }
                Err(error) => output.push_str(&format!("Error: {error}")),
            }
            output.push_str("\n\n");
        }

        Ok(output)
    }

    pub(super) async fn diff_impl(&self, params: DiffParams) -> Result<String, String> {
        validate_url(&params.url).await?;
        let previous = self.load_previous_snapshot(&params).await?;
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
                let current = self.cloud_markdown_to_extraction(&params.url, &resp)?;
                let content_diff = noxa_core::diff::diff(&previous, &current);
                Ok(serde_json::to_string_pretty(&content_diff).unwrap_or_default())
            }
        }
    }

    pub(super) async fn brand_impl(&self, params: BrandParams) -> Result<String, String> {
        validate_url(&params.url).await?;
        let fetch_result =
            tokio::time::timeout(LOCAL_FETCH_TIMEOUT, self.fetch_client.fetch(&params.url))
                .await
                .map_err(|_| format!("Fetch timed out after 30s for {}", params.url))?
                .map_err(|e| format!("Fetch failed: {e}"))?;

        if cloud::is_bot_protected(&fetch_result.html, &fetch_result.headers) {
            if let Some(ref cloud) = self.cloud {
                let resp = cloud.post("brand", json!({"url": params.url})).await?;
                return Ok(serde_json::to_string_pretty(&resp).unwrap_or_default());
            }
            return Err(format!(
                "Bot protection detected on {}. Set NOXA_API_KEY for automatic cloud bypass. \
                 Get a key at https://noxa.io",
                params.url
            ));
        }

        let identity = noxa_core::brand::extract_brand(&fetch_result.html, Some(&fetch_result.url));
        Ok(serde_json::to_string_pretty(&identity).unwrap_or_default())
    }

    async fn load_previous_snapshot(
        &self,
        params: &DiffParams,
    ) -> Result<noxa_core::ExtractionResult, String> {
        let previous: Option<noxa_core::ExtractionResult> = match params.previous_snapshot {
            Some(ref json) => Some(
                serde_json::from_str(json)
                    .map_err(|e| format!("Failed to parse previous_snapshot JSON: {e}"))?,
            ),
            None => self.store.read(&params.url).await.ok().flatten(),
        };

        match previous {
            Some(previous) => Ok(previous),
            None => self.store_diff_baseline(&params.url).await,
        }
    }

    async fn store_diff_baseline(&self, url: &str) -> Result<noxa_core::ExtractionResult, String> {
        info!(url = %url, "diff: no previous snapshot — fetching baseline");
        let fetch_result = cloud::smart_fetch(
            &self.fetch_client,
            self.cloud.as_ref(),
            url,
            &[],
            &[],
            false,
            &["markdown"],
        )
        .await;

        match fetch_result {
            Err(error) => Err(format!(
                "No previous snapshot stored for {url}. Failed to fetch baseline: {error}",
            )),
            Ok(SmartFetchResult::Local(extraction)) => {
                match self.store.write(url, &extraction).await {
                    Ok(_) => Err(format!(
                        "No previous snapshot stored for {url}. The page has been fetched and stored \
                     as the baseline — run diff again to compare against this snapshot.",
                    )),
                    Err(error) => Err(format!(
                        "No previous snapshot stored for {url}. Fetched the page but failed to store \
                     baseline: {error}. Ensure the content store is writable and retry.",
                    )),
                }
            }
            Ok(SmartFetchResult::Cloud(_)) => Err(format!(
                "No previous snapshot stored for {url}. The page required cloud fetching (bot \
                 protection) and cannot be auto-stored as a baseline. Provide a previous_snapshot \
                 parameter explicitly.",
            )),
        }
    }

    fn cloud_markdown_to_extraction(
        &self,
        url: &str,
        resp: &serde_json::Value,
    ) -> Result<noxa_core::ExtractionResult, String> {
        let markdown = resp.get("markdown").and_then(|v| v.as_str()).unwrap_or("");
        if markdown.is_empty() {
            return Err(
                "Cloud API fallback returned no markdown content; cannot compute diff.".into(),
            );
        }

        Ok(noxa_core::ExtractionResult {
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
                url: Some(url.to_string()),
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
            structured_data: Vec::new(),
        })
    }
}
