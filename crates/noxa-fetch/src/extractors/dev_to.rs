use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "dev_to",
    label: "dev.to Article",
    description: "Extract article metadata and content from dev.to.",
    url_patterns: &["https://dev.to/*/*"],
};

pub fn matches(url: &str) -> bool {
    parse_username_slug(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (username, slug) = parse_username_slug(url).ok_or_else(|| {
        FetchError::Build(format!("dev_to: cannot parse username/slug from '{url}'"))
    })?;
    let api_url = format!("https://dev.to/api/articles/{username}/{slug}");
    let article = client.get_json(&api_url).await?;

    Ok(json!({
        "url": url,
        "id": article.get("id").cloned(),
        "title": article.get("title").cloned(),
        "description": article.get("description").cloned(),
        "body_markdown": article.get("body_markdown").cloned(),
        "url_canonical": article.get("canonical_url").cloned(),
        "published_at": article.get("published_at").cloned(),
        "edited_at": article.get("edited_at").cloned(),
        "reading_time_min": article.get("reading_time_minutes").cloned(),
        "tags": article.get("tag_list").cloned(),
        "positive_reactions": article.get("positive_reactions_count").cloned(),
        "public_reactions": article.get("public_reactions_count").cloned(),
        "comments_count": article.get("comments_count").cloned(),
        "page_views_count": article.get("page_views_count").cloned(),
        "cover_image": article.get("cover_image").cloned(),
        "author": {
            "username": article.pointer("/user/username").cloned(),
            "name": article.pointer("/user/name").cloned(),
            "twitter": article.pointer("/user/twitter_username").cloned(),
            "github": article.pointer("/user/github_username").cloned(),
            "website": article.pointer("/user/website_url").cloned(),
        },
    }))
}

fn parse_username_slug(url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "dev.to" && host != "www.dev.to" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() != 2 || RESERVED_FIRST_SEGS.contains(&segs[0]) {
        return None;
    }
    Some((segs[0].to_string(), segs[1].to_string()))
}

const RESERVED_FIRST_SEGS: &[&str] = &[
    "api", "tags", "search", "settings", "enter", "signup", "about", "privacy", "terms",
    "contact", "sponsorships", "sponsors", "shop", "videos", "listings", "podcasts", "p", "t",
];
