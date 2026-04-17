use noxa_store::parse_http_url;
use serde_json::json;
use tracing::warn;

use crate::cloud::SmartFetchResult;
use crate::server::{NO_LLM_PROVIDERS_MESSAGE, NoxaMcp, validate_url};
use crate::tools::{ExtractParams, SearchParams, SummarizeParams};

impl NoxaMcp {
    pub(super) async fn extract_impl(&self, params: ExtractParams) -> Result<String, String> {
        validate_url(&params.url).await?;

        if params.schema.is_none() && params.prompt.is_none() {
            return Err("Either 'schema' or 'prompt' is required for extraction.".into());
        }

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
        let llm_content = self.fetch_llm_content(&params.url).await?;

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

    pub(super) async fn summarize_impl(&self, params: SummarizeParams) -> Result<String, String> {
        validate_url(&params.url).await?;

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
        let llm_content = self.fetch_llm_content(&params.url).await?;

        noxa_llm::summarize::summarize(&llm_content, params.max_sentences, chain, None)
            .await
            .map_err(|e| format!("Summarization failed: {e}"))
    }

    pub(super) async fn search_impl(&self, params: SearchParams) -> Result<String, String> {
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

            let mut valid_results: Vec<&noxa_fetch::SearxngResult> = Vec::new();
            for result in &results {
                if let Err(error) = validate_url(&result.url).await {
                    warn!("skipping result URL {}: {error}", result.url);
                } else {
                    valid_results.push(result);
                }
            }

            let valid_urls: Vec<&str> = valid_results.iter().map(|r| r.url.as_str()).collect();
            let scraped = self
                .fetch_client
                .fetch_and_extract_batch(&valid_urls, 4)
                .await;

            let mut out = String::with_capacity(results.len() * 256);
            out.push_str(&format!("Found {} result(s):\n\n", valid_results.len()));
            for (idx, (result, scrape)) in valid_results.iter().zip(scraped.iter()).enumerate() {
                out.push_str(&format!(
                    "{}. {}\n   {}\n",
                    idx + 1,
                    result.title,
                    result.url
                ));
                if !result.content.is_empty() {
                    out.push_str(&format!("   {}\n", result.content));
                }
                if let Err(ref error) = scrape.result {
                    out.push_str(&format!("   Error: {error}\n"));
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
            for (idx, result) in results.iter().enumerate() {
                let title = result.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let url = result.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let snippet = result
                    .get("snippet")
                    .or_else(|| result.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                out.push_str(&format!("{}. {}\n   {}\n", idx + 1, title, url));
                if !snippet.is_empty() {
                    out.push_str(&format!("   {snippet}\n"));
                }
                out.push('\n');
            }
            Ok(out)
        } else {
            Ok(serde_json::to_string_pretty(&resp).unwrap_or_default())
        }
    }

    async fn fetch_llm_content(&self, url: &str) -> Result<String, String> {
        let content = match self.smart_fetch_llm(url).await? {
            SmartFetchResult::Local(extraction) => noxa_core::to_llm_text(&extraction, Some(url)),
            SmartFetchResult::Cloud(resp) => resp
                .get("llm")
                .or_else(|| resp.get("markdown"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };
        Ok(content)
    }
}
