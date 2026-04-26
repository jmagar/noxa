use regex::Regex;
use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "linkedin_post",
    label: "LinkedIn Post",
    description: "Extract post metadata from LinkedIn.",
    url_patterns: &[
        "https://www.linkedin.com/posts/*",
        "https://www.linkedin.com/feed/update/*",
    ],
};

pub fn matches(url: &str) -> bool {
    extract_urn(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let urn = extract_urn(url).ok_or_else(|| {
        FetchError::Build(format!("linkedin_post: cannot extract URN from '{url}'"))
    })?;
    let embed_url = format!("https://www.linkedin.com/embed/feed/update/{urn}");
    let html = client.get_text(&embed_url).await?;
    let og = parse_og_tags(&html);

    Ok(json!({
        "url": url,
        "embed_url": embed_url,
        "urn": urn,
        "canonical_url": og.get("url").cloned().unwrap_or_else(|| json!(url)),
        "data_completeness": "embed",
        "title": og.get("title").cloned(),
        "body": parse_post_body(&html),
        "author_name": parse_author(&html),
        "image_url": og.get("image").cloned(),
        "site_name": og.get("site_name").cloned().unwrap_or_else(|| json!("LinkedIn")),
    }))
}

fn extract_urn(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "www.linkedin.com" && host != "linkedin.com" {
        return None;
    }
    if let Some(index) = url.find("urn:li:") {
        let tail = &url[index..];
        let end = tail.find(['/', '?', '#']).unwrap_or(tail.len());
        let urn = &tail[..end];
        let mut parts = urn.split(':');
        if parts.next() == Some("urn")
            && parts.next() == Some("li")
            && parts.next().is_some()
            && parts
                .next()
                .is_some_and(|part| part.chars().all(|c| c.is_ascii_digit()))
        {
            return Some(urn.to_string());
        }
    }
    let re = Regex::new(r"/posts/[^/]*?-(\d{15,})-[A-Za-z0-9]{2,}/?").ok()?;
    re.captures(url)
        .and_then(|captures| captures.get(1))
        .map(|id| format!("urn:li:activity:{}", id.as_str()))
}

fn parse_og_tags(html: &str) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    let Ok(re) = Regex::new(r#"(?i)<meta[^>]+property="og:([a-z_]+)"[^>]+content="([^"]+)""#)
    else {
        return out;
    };
    for captures in re.captures_iter(html) {
        if let (Some(key), Some(value)) = (captures.get(1), captures.get(2)) {
            out.entry(key.as_str().to_lowercase())
                .or_insert_with(|| json!(html_decode(value.as_str())));
        }
    }
    out
}

fn parse_post_body(html: &str) -> Option<String> {
    let re = Regex::new(
        r#"(?s)<p[^>]+class="[^"]*attributed-text-segment-list__content[^"]*"[^>]*>(.*?)</p>"#,
    )
    .ok()?;
    let inner = re.captures(html)?.get(1)?.as_str();
    Some(strip_tags(inner).trim().to_string())
}

fn parse_author(html: &str) -> Option<String> {
    let re = Regex::new(r"<title>([^<]+)</title>").ok()?;
    let title = re.captures(html)?.get(1)?.as_str();
    title
        .rsplit_once('|')
        .map(|(_, name)| html_decode(name.trim()))
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
