//! Shared SearXNG JSON search support.
use std::time::Duration;

use serde::Deserialize;

use crate::error::FetchError;

#[derive(Debug, Clone, Deserialize)]
pub struct SearxngResult {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub content: String,
    #[serde(rename = "publishedDate")]
    pub published_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearxngResponse {
    pub results: Vec<SearxngResult>,
}

pub async fn searxng_search(
    client: &wreq::Client,
    base_url: &str,
    query: &str,
    num_results: u32,
) -> Result<Vec<SearxngResult>, FetchError> {
    let encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let search_url = format!(
        "{}/search?q={encoded}&format=json&pageno=1",
        base_url.trim_end_matches('/')
    );

    let resp = client
        .get(&search_url)
        .timeout(Duration::from_secs(15))
        .send()
        .await?;

    let status = resp.status().as_u16();
    if status == 403 {
        return Err(FetchError::Build(
            "SearXNG returned 403 — add 'json' to formats in settings.yml".into(),
        ));
    }
    if !(200..300).contains(&status) {
        return Err(FetchError::Build(format!("SearXNG returned HTTP {status}")));
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.contains("json") {
        return Err(FetchError::Build(format!(
            "SearXNG returned non-JSON (Content-Type: {content_type}) — is JSON format enabled in settings.yml?"
        )));
    }

    let body = resp
        .bytes()
        .await
        .map_err(|e| FetchError::BodyDecode(e.to_string()))?;
    let parsed: SearxngResponse = serde_json::from_slice(&body)
        .map_err(|e| FetchError::Build(format!("SearXNG parse error: {e}")))?;

    Ok(parsed
        .results
        .into_iter()
        .take(num_results as usize)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full() {
        let json = r#"{"results":[{"title":"Rust","url":"https://rust-lang.org","content":"A language.","publishedDate":"2024-01-01"}]}"#;
        let resp: SearxngResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.results[0].title, "Rust");
        assert_eq!(resp.results[0].published_date.as_deref(), Some("2024-01-01"));
    }

    #[test]
    fn test_deserialize_missing_content_defaults_empty() {
        let json = r#"{"results":[{"title":"X","url":"https://x.com"}]}"#;
        let resp: SearxngResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.results[0].content, "");
    }

    #[test]
    fn test_empty_results() {
        let json = r#"{"results":[]}"#;
        let resp: SearxngResponse = serde_json::from_str(json).unwrap();
        assert!(resp.results.is_empty());
    }
}
