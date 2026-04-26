use serde_json::{Value, json};

use super::{ExtractorInfo, host_matches, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "npm",
    label: "npm Package",
    description: "Extract package metadata from npm.",
    url_patterns: &["https://www.npmjs.com/package/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "npmjs.com") && url.contains("/package/")
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let name = parse_name(url)
        .ok_or_else(|| FetchError::Build(format!("npm: cannot parse name from '{url}'")))?;
    let encoded = encode_package_name(&name);
    let registry_url = format!("https://registry.npmjs.org/{encoded}");
    let pkg = client.get_json(&registry_url).await?;
    let latest_version = pkg
        .pointer("/dist-tags/latest")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let latest_manifest = latest_version
        .as_ref()
        .and_then(|version| pkg.pointer(&format!("/versions/{version}")));
    let release_count = pkg
        .get("versions")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    let latest_release_date = latest_version
        .as_ref()
        .and_then(|version| pkg.pointer(&format!("/time/{version}")))
        .cloned();
    let weekly_downloads = fetch_weekly_downloads(client, &encoded).await.ok();

    Ok(json!({
        "url": url,
        "name": pkg.get("name").cloned().unwrap_or_else(|| json!(name)),
        "description": pkg.get("description").cloned(),
        "latest_version": latest_version,
        "license": latest_manifest.and_then(|manifest| manifest.get("license").cloned()),
        "homepage": pkg.get("homepage").cloned(),
        "repository": pkg.pointer("/repository/url").cloned(),
        "dependencies": latest_manifest.and_then(|manifest| manifest.get("dependencies").cloned()),
        "dev_dependencies": latest_manifest.and_then(|manifest| manifest.get("devDependencies").cloned()),
        "peer_dependencies": latest_manifest.and_then(|manifest| manifest.get("peerDependencies").cloned()),
        "keywords": pkg.get("keywords").cloned(),
        "maintainers": pkg.get("maintainers").cloned(),
        "deprecated": latest_manifest.and_then(|manifest| manifest.get("deprecated").cloned()),
        "release_count": release_count,
        "latest_release_date": latest_release_date,
        "weekly_downloads": weekly_downloads,
    }))
}

async fn fetch_weekly_downloads(
    client: &dyn ExtractorHttp,
    encoded_name: &str,
) -> Result<i64, FetchError> {
    let url = format!("https://api.npmjs.org/downloads/point/last-week/{encoded_name}");
    let downloads = client.get_json(&url).await?;
    downloads
        .get("downloads")
        .and_then(Value::as_i64)
        .ok_or_else(|| FetchError::BodyDecode("npm downloads response missing downloads".into()))
}

fn parse_name(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "www.npmjs.com" && host != "npmjs.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 || segs[0] != "package" {
        return None;
    }
    if segs[1].starts_with('@') {
        return Some(format!("{}/{}", segs[1], segs.get(2)?));
    }
    Some(segs[1].to_string())
}

fn encode_package_name(name: &str) -> String {
    name.replace('@', "%40").replace('/', "%2F")
}
