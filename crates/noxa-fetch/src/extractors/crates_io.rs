use serde_json::{Value, json};

use super::{ExtractorInfo, host_matches, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "crates_io",
    label: "crates.io Crate",
    description: "Extract package metadata from crates.io.",
    url_patterns: &["https://crates.io/crates/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "crates.io") && url.contains("/crates/")
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let name = parse_name(url)
        .ok_or_else(|| FetchError::Build(format!("crates.io: cannot parse name from '{url}'")))?;
    let api_url = format!("https://crates.io/api/v1/crates/{name}");
    let body = client.get_json(&api_url).await?;
    let crate_info = body.get("crate").cloned().unwrap_or_else(|| json!({}));
    let versions = body
        .get("versions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let latest = versions
        .iter()
        .find(|version| {
            !version
                .get("yanked")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .or_else(|| versions.first());

    Ok(json!({
        "url": url,
        "name": crate_info.get("id").cloned(),
        "description": crate_info.get("description").cloned(),
        "homepage": crate_info.get("homepage").cloned(),
        "documentation": crate_info.get("documentation").cloned(),
        "repository": crate_info.get("repository").cloned(),
        "max_stable_version": crate_info.get("max_stable_version").cloned(),
        "max_version": crate_info.get("max_version").cloned(),
        "newest_version": crate_info.get("newest_version").cloned(),
        "downloads": crate_info.get("downloads").cloned(),
        "recent_downloads": crate_info.get("recent_downloads").cloned(),
        "categories": crate_info.get("categories").cloned().unwrap_or_else(|| json!([])),
        "keywords": crate_info.get("keywords").cloned().unwrap_or_else(|| json!([])),
        "release_count": versions.len(),
        "latest_release_date": latest.and_then(|version| version.get("created_at").cloned()),
        "latest_license": latest.and_then(|version| version.get("license").cloned()),
        "latest_rust_version": latest.and_then(|version| version.get("rust_version").cloned()),
        "latest_yanked": latest.and_then(|version| version.get("yanked").cloned()),
        "created_at": crate_info.get("created_at").cloned(),
        "updated_at": crate_info.get("updated_at").cloned(),
    }))
}

fn parse_name(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "crates.io" && host != "www.crates.io" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 || segs[0] != "crates" {
        return None;
    }
    Some(segs[1].to_string())
}
