use serde_json::{Value, json};

use super::{ExtractorInfo, host_matches, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "hackernews",
    label: "Hacker News Item",
    description: "Extract Hacker News story or comment metadata.",
    url_patterns: &["https://news.ycombinator.com/item?id=*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "news.ycombinator.com") && url.contains("item?id=")
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let id = parse_item_id(url).ok_or_else(|| {
        FetchError::Build(format!("hackernews: cannot parse item id from '{url}'"))
    })?;
    let api_url = format!("https://hn.algolia.com/api/v1/items/{id}");
    let item = client.get_json(&api_url).await?;
    let comments: Vec<_> = item
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(comment_json)
        .collect();

    Ok(json!({
        "url": url,
        "post": {
            "id": item.get("id").cloned(),
            "type": item.get("type").cloned(),
            "title": item.get("title").cloned(),
            "url": item.get("url").cloned(),
            "author": item.get("author").cloned(),
            "points": item.get("points").cloned(),
            "text": item.get("text").cloned(),
            "created_at": item.get("created_at").cloned(),
            "created_at_unix": item.get("created_at_i").cloned(),
            "comment_count": count_descendants(&item),
            "permalink": format!("https://news.ycombinator.com/item?id={id}"),
        },
        "comments": comments,
    }))
}

fn parse_item_id(url: &str) -> Option<u64> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.host_str()? == "hn.algolia.com" {
        return parsed.path_segments()?.find_map(|segment| segment.parse().ok());
    }
    parsed
        .query_pairs()
        .find_map(|(key, value)| (key == "id").then(|| value.parse().ok()).flatten())
}

fn comment_json(item: &Value) -> Option<Value> {
    if item.get("type").and_then(Value::as_str) != Some("comment") {
        return None;
    }
    let replies: Vec<_> = item
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(comment_json)
        .collect();
    Some(json!({
        "id": item.get("id").cloned(),
        "author": item.get("author").cloned(),
        "text": item.get("text").cloned(),
        "created_at": item.get("created_at").cloned(),
        "created_at_unix": item.get("created_at_i").cloned(),
        "parent_id": item.get("parent_id").cloned(),
        "story_id": item.get("story_id").cloned(),
        "replies": replies,
    }))
}

fn count_descendants(item: &Value) -> usize {
    item.get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|child| child.get("type").and_then(Value::as_str) == Some("comment"))
        .map(|child| 1 + count_descendants(child))
        .sum()
}
