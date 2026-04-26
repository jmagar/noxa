use regex::Regex;
use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "substack_post",
    label: "Substack Post",
    description: "Extract post metadata from Substack publications.",
    url_patterns: &["https://*.substack.com/p/*", "*/p/*"],
};

pub fn matches(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            let host = parsed.host_str()?.to_ascii_lowercase();
            let has_post_path = parsed.path_segments().is_some_and(|mut segments| {
                segments.next() == Some("p") && segments.next().is_some()
            });
            Some(has_post_path && (host.ends_with(".substack.com") || host != "substack.com"))
        })
        .unwrap_or(false)
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let html = client.get_text(url).await?;
    let article = article_json_ld(&html).unwrap_or_else(|| json!({}));
    let body = article_body(&html);

    Ok(json!({
        "url": url,
        "canonical_url": meta(&html, "property", "og:url").unwrap_or_else(|| url.to_string()),
        "title": string_field(&article, "headline")
            .or_else(|| string_field(&article, "name"))
            .or_else(|| meta(&html, "property", "og:title"))
            .or_else(|| title_tag(&html)),
        "description": string_field(&article, "description")
            .or_else(|| meta(&html, "property", "og:description")),
        "author": author(&article).or_else(|| meta(&html, "name", "author")),
        "published_at": string_field(&article, "datePublished")
            .or_else(|| meta(&html, "property", "article:published_time")),
        "modified_at": string_field(&article, "dateModified")
            .or_else(|| meta(&html, "property", "article:modified_time")),
        "image": article.get("image").cloned()
            .or_else(|| meta(&html, "property", "og:image").map(Value::String)),
        "body": body,
        "data_source": "html",
    }))
}

fn article_json_ld(html: &str) -> Option<Value> {
    let re =
        Regex::new(r#"(?is)<script[^>]+type=["']application/ld\+json["'][^>]*>(.*?)</script>"#)
            .ok()?;
    re.captures_iter(html)
        .filter_map(|captures| captures.get(1))
        .filter_map(|body| serde_json::from_str::<Value>(body.as_str().trim()).ok())
        .flat_map(flatten_graph)
        .find(is_article)
}

fn flatten_graph(value: Value) -> Vec<Value> {
    if let Some(values) = value.as_array() {
        return values.clone();
    }
    if let Some(values) = value.get("@graph").and_then(Value::as_array) {
        return values.clone();
    }
    vec![value]
}

fn is_article(value: &Value) -> bool {
    match value.get("@type") {
        Some(Value::String(kind)) => ARTICLE_TYPES.contains(&kind.as_str()),
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .any(|kind| ARTICLE_TYPES.contains(&kind)),
        _ => false,
    }
}

const ARTICLE_TYPES: &[&str] = &["Article", "BlogPosting", "NewsArticle"];

fn author(article: &Value) -> Option<String> {
    let author = article.get("author")?;
    if let Some(name) = string_field(author, "name") {
        return Some(name);
    }
    author
        .as_array()
        .and_then(|authors| authors.first())
        .and_then(|author| {
            string_field(author, "name").or_else(|| author.as_str().map(str::to_string))
        })
        .or_else(|| author.as_str().map(str::to_string))
}

fn article_body(html: &str) -> Option<String> {
    let re = Regex::new(r"(?is)<article[^>]*>(.*?)</article>").ok()?;
    let inner = re.captures(html)?.get(1)?.as_str();
    let text = strip_tags(inner);
    (!text.is_empty()).then_some(text)
}

fn title_tag(html: &str) -> Option<String> {
    let re = Regex::new(r"(?is)<title[^>]*>(.*?)</title>").ok()?;
    re.captures(html)
        .and_then(|captures| captures.get(1))
        .map(|value| html_decode(value.as_str()).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn meta(html: &str, attr: &str, key: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<meta[^>]+{}=["']{}["'][^>]+content=["']([^"']+)["']"#,
        regex::escape(attr),
        regex::escape(key)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|value| html_decode(value.as_str()))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn strip_tags(html: &str) -> String {
    let Ok(re) = Regex::new(r"<[^>]+>") else {
        return html_decode(html);
    };
    html_decode(&re.replace_all(html, " "))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn html_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}
