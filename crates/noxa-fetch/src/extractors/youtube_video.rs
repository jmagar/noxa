use regex::Regex;
use serde_json::{Value, json};

use super::{ExtractorInfo, host_matches, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "youtube_video",
    label: "YouTube Video",
    description: "Extract video metadata from YouTube.",
    url_patterns: &["https://www.youtube.com/watch?v=*", "https://youtu.be/*"],
};

pub fn matches(url: &str) -> bool {
    (host_matches(url, "youtube.com") && (url.contains("watch?v=") || url.contains("/shorts/")))
        || host_matches(url, "youtu.be")
        || host_matches(url, "youtube-nocookie.com")
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let video_id = parse_video_id(url).ok_or_else(|| {
        FetchError::Build(format!("youtube_video: cannot parse video id from '{url}'"))
    })?;
    let canonical = format!("https://www.youtube.com/watch?v={video_id}");
    let html = client.get_text(&canonical).await?;
    let player = extract_player_response(&html)
        .ok_or_else(|| FetchError::BodyDecode("youtube: no player response found".into()))?;
    let details = player.get("videoDetails");
    let microformat = player.pointer("/microformat/playerMicroformatRenderer");

    Ok(json!({
        "url": url,
        "canonical_url": canonical,
        "data_source": "player_response",
        "video_id": video_id,
        "title": get_str(details, "title"),
        "description": get_str(details, "shortDescription"),
        "author": get_str(details, "author"),
        "channel_id": get_str(details, "channelId"),
        "channel_url": get_str(microformat, "ownerProfileUrl"),
        "view_count": get_int(details, "viewCount"),
        "length_seconds": get_int(details, "lengthSeconds"),
        "is_live": details.and_then(|d| d.get("isLiveContent")).and_then(Value::as_bool),
        "is_private": details.and_then(|d| d.get("isPrivate")).and_then(Value::as_bool),
        "is_unlisted": microformat.and_then(|m| m.get("isUnlisted")).and_then(Value::as_bool),
        "allow_ratings": details.and_then(|d| d.get("allowRatings")).and_then(Value::as_bool),
        "category": get_str(microformat, "category"),
        "upload_date": get_str(microformat, "uploadDate"),
        "publish_date": get_str(microformat, "publishDate"),
        "keywords": details.and_then(|d| d.get("keywords")).cloned().unwrap_or_else(|| json!([])),
        "thumbnails": details
            .and_then(|d| d.pointer("/thumbnail/thumbnails"))
            .cloned()
            .unwrap_or_else(|| json!([])),
        "caption_tracks": Vec::<Value>::new(),
    }))
}

fn parse_video_id(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host == "youtu.be" {
        return parsed.path_segments()?.next().map(ToString::to_string);
    }
    if host.ends_with("youtube.com") || host.ends_with("youtube-nocookie.com") {
        if parsed.path() == "/watch" {
            return parsed
                .query_pairs()
                .find_map(|(key, value)| (key == "v").then(|| value.to_string()));
        }
        let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
        if matches!(segs.first(), Some(&"shorts") | Some(&"embed")) {
            return segs.get(1).map(|value| (*value).to_string());
        }
    }
    None
}

fn extract_player_response(html: &str) -> Option<Value> {
    let re = Regex::new(r"(?:var\s+)?ytInitialPlayerResponse\s*=\s*(\{.+?\})\s*;").ok()?;
    serde_json::from_str(re.captures(html)?.get(1)?.as_str()).ok()
}

fn get_str(value: Option<&Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn get_int(value: Option<&Value>, key: &str) -> Option<i64> {
    value.and_then(|value| value.get(key)).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|string| string.parse().ok()))
    })
}
