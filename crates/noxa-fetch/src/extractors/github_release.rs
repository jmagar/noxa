use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "github_release",
    label: "GitHub Release",
    description: "Extract release metadata from GitHub.",
    url_patterns: &["https://github.com/*/*/releases/tag/*"],
};

pub fn matches(url: &str) -> bool {
    parse_release(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (owner, repo, tag) = parse_release(url).ok_or_else(|| {
        FetchError::Build(format!("github_release: cannot parse release URL '{url}'"))
    })?;
    let api_url = format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}");
    let r = client.get_json(&api_url).await?;
    let assets = r.get("assets").cloned().unwrap_or_else(|| json!([]));
    let total_downloads = assets
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|asset| asset.get("download_count").and_then(Value::as_i64))
        .sum::<i64>();
    let asset_count = assets.as_array().map_or(0, Vec::len);

    Ok(json!({
        "url": url,
        "owner": owner,
        "repo": repo,
        "tag_name": r.get("tag_name").cloned(),
        "name": r.get("name").cloned(),
        "body": r.get("body").cloned(),
        "draft": r.get("draft").cloned(),
        "prerelease": r.get("prerelease").cloned(),
        "author": r.pointer("/author/login").cloned(),
        "created_at": r.get("created_at").cloned(),
        "published_at": r.get("published_at").cloned(),
        "asset_count": asset_count,
        "total_downloads": total_downloads,
        "assets": assets,
        "html_url": r.get("html_url").cloned(),
    }))
}

fn parse_release(url: &str) -> Option<(String, String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "github.com" && host != "www.github.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() < 5 || segs[2] != "releases" || segs[3] != "tag" {
        return None;
    }
    Some((
        segs[0].to_string(),
        segs[1].to_string(),
        segs[4].to_string(),
    ))
}
