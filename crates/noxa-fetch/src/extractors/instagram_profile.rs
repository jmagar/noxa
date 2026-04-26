use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "instagram_profile",
    label: "Instagram Profile",
    description: "Extract profile metadata from Instagram.",
    url_patterns: &["https://www.instagram.com/*"],
};

pub fn matches(url: &str) -> bool {
    parse_username(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let username = parse_username(url).ok_or_else(|| {
        FetchError::Build(format!("instagram_profile: cannot parse username from '{url}'"))
    })?;
    let api_url =
        format!("https://www.instagram.com/api/v1/users/web_profile_info/?username={username}");
    let body = client.get_json(&api_url).await?;
    let user = body
        .pointer("/data/user")
        .ok_or_else(|| FetchError::BodyDecode("instagram profile missing data.user".into()))?;
    let recent_posts: Vec<_> = user
        .pointer("/edge_owner_to_timeline_media/edges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|edge| edge.get("node"))
        .map(post_summary)
        .collect();

    Ok(json!({
        "url": url,
        "canonical_url": format!("https://www.instagram.com/{username}/"),
        "username": user.get("username").cloned().unwrap_or_else(|| json!(username)),
        "data_completeness": "api",
        "user_id": user.get("id").cloned(),
        "full_name": user.get("full_name").cloned(),
        "biography": user.get("biography").cloned(),
        "biography_links": user.get("bio_links").cloned(),
        "external_url": user.get("external_url").cloned(),
        "category": user.get("category_name").cloned(),
        "follower_count": user.pointer("/edge_followed_by/count").cloned(),
        "following_count": user.pointer("/edge_follow/count").cloned(),
        "post_count": user.pointer("/edge_owner_to_timeline_media/count").cloned(),
        "is_verified": user.get("is_verified").cloned(),
        "is_private": user.get("is_private").cloned(),
        "is_business": user.get("is_business_account").cloned(),
        "is_professional": user.get("is_professional_account").cloned(),
        "profile_pic_url": user.get("profile_pic_url_hd").or_else(|| user.get("profile_pic_url")).cloned(),
        "recent_posts": recent_posts,
    }))
}

fn parse_username(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "www.instagram.com" && host != "instagram.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() != 1 || RESERVED.contains(&segs[0]) {
        return None;
    }
    Some(segs[0].to_string())
}

const RESERVED: &[&str] = &[
    "p", "reel", "reels", "tv", "explore", "stories", "directory", "accounts", "about",
    "developer", "press", "api", "ads", "blog", "fragments", "terms", "privacy", "session",
    "login", "signup",
];

fn post_summary(node: &Value) -> Value {
    let shortcode = node.get("shortcode").and_then(Value::as_str).unwrap_or("");
    let kind = classify(node);
    let path = if kind == "reel" { "reel" } else { "p" };
    json!({
        "shortcode": node.get("shortcode").cloned(),
        "url": format!("https://www.instagram.com/{path}/{shortcode}/"),
        "kind": kind,
        "is_video": node.get("is_video").cloned(),
        "video_views": node.get("video_view_count").cloned(),
        "thumbnail_url": node.get("thumbnail_src").or_else(|| node.get("display_url")).cloned(),
        "display_url": node.get("display_url").cloned(),
        "like_count": node.pointer("/edge_media_preview_like/count").cloned(),
        "comment_count": node.pointer("/edge_media_to_comment/count").cloned(),
        "taken_at": node.get("taken_at_timestamp").cloned(),
        "caption": node.pointer("/edge_media_to_caption/edges/0/node/text").cloned(),
        "alt_text": node.get("accessibility_caption").cloned(),
        "dimensions": node.get("dimensions").cloned(),
        "product_type": node.get("product_type").cloned(),
    })
}

fn classify(node: &Value) -> &'static str {
    if node.get("product_type").and_then(Value::as_str) == Some("clips") {
        return "reel";
    }
    match node.get("__typename").and_then(Value::as_str) {
        Some("GraphSidecar") => "carousel",
        Some("GraphVideo") => "video",
        Some("GraphImage") => "photo",
        _ => "post",
    }
}
