use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "instagram_post",
    label: "Instagram Post",
    description: "Extract post metadata from Instagram.",
    url_patterns: &[
        "https://www.instagram.com/p/*",
        "https://www.instagram.com/reel/*",
    ],
};

pub fn matches(url: &str) -> bool {
    parse_shortcode(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (kind, shortcode) = parse_shortcode(url).ok_or_else(|| {
        FetchError::Build(format!(
            "instagram_post: cannot parse shortcode from '{url}'"
        ))
    })?;
    let embed_url = format!("https://www.instagram.com/p/{shortcode}/embed/captioned/");
    let html = client.get_text(&embed_url).await?;

    Ok(json!({
        "url": url,
        "embed_url": embed_url,
        "shortcode": shortcode,
        "kind": kind,
        "data_completeness": "embed",
        "author_username": parse_username(&html),
        "caption": parse_caption(&html),
        "thumbnail_url": parse_thumbnail(&html),
        "canonical_url": format!("https://www.instagram.com/{}/{shortcode}/", path_segment_for(kind)),
    }))
}

fn parse_shortcode(url: &str) -> Option<(&'static str, String)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "www.instagram.com" && host != "instagram.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let kind = match segs.first().copied()? {
        "p" => "post",
        "reel" | "reels" => "reel",
        "tv" => "tv",
        _ => return None,
    };
    Some((kind, segs.get(1)?.to_string()))
}

fn path_segment_for(kind: &str) -> &'static str {
    match kind {
        "reel" => "reel",
        "tv" => "tv",
        _ => "p",
    }
}

fn parse_username(html: &str) -> Option<String> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)class="CaptionUsername"[^>]*>([^<]+)<"#).unwrap());
    RE.captures(html)
        .and_then(|captures| captures.get(1))
        .map(|value| html_decode(value.as_str().trim()))
}

fn parse_caption(html: &str) -> Option<String> {
    static OUTER_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)<div\s+class="Caption"[^>]*>(.*?)</div>"#).unwrap());
    static USER_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)<a[^>]*class="CaptionUsername"[^>]*>.*?</a>"#).unwrap());
    static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

    let block = OUTER_RE.captures(html)?.get(1)?.as_str();
    let stripped = USER_RE.replace_all(block, "");
    let text = TAG_RE.replace_all(&stripped, " ");
    let decoded = html_decode(text.trim());
    let cleaned = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    (!cleaned.is_empty()).then_some(cleaned)
}

fn parse_thumbnail(html: &str) -> Option<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)<img[^>]+class="[^"]*EmbeddedMediaImage[^"]*"[^>]+src="([^"]+)""#)
            .unwrap()
    });
    RE.captures(html)
        .and_then(|captures| captures.get(1))
        .map(|value| html_decode(value.as_str()))
}

fn html_decode(value: &str) -> String {
    decode_html_entities(value)
}

fn decode_html_entities(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let entity = &rest[1..end];
        if let Some(decoded) = decode_entity(entity) {
            out.push(decoded);
        } else if let Some(decoded) = decode_named_entity(entity) {
            out.push_str(decoded);
        } else {
            out.push_str(&rest[..=end]);
        }
        rest = &rest[end + 1..];
    }

    out.push_str(rest);
    out
}

fn decode_entity(entity: &str) -> Option<char> {
    let codepoint = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .or_else(|| {
            entity
                .strip_prefix('#')
                .and_then(|decimal| decimal.parse().ok())
        })?;
    char::from_u32(codepoint)
}

fn decode_named_entity(entity: &str) -> Option<&'static str> {
    match entity {
        "amp" => Some("&"),
        "lt" => Some("<"),
        "gt" => Some(">"),
        "quot" => Some("\""),
        "apos" | "#39" => Some("'"),
        "nbsp" => Some(" "),
        "hellip" => Some("..."),
        _ => None,
    }
}
