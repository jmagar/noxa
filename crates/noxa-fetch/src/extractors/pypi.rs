use serde_json::{Value, json};

use super::{ExtractorInfo, host_matches, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "pypi",
    label: "PyPI Package",
    description: "Extract package metadata from PyPI.",
    url_patterns: &["https://pypi.org/project/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "pypi.org") && url.contains("/project/")
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (name, version) = parse_project(url).ok_or_else(|| {
        FetchError::Build(format!("pypi: cannot parse package name from '{url}'"))
    })?;
    let api_url = match &version {
        Some(version) => format!("https://pypi.org/pypi/{name}/{version}/json"),
        None => format!("https://pypi.org/pypi/{name}/json"),
    };
    let pkg = client.get_json(&api_url).await?;
    let info = pkg.get("info").cloned().unwrap_or_else(|| json!({}));
    let release_count = pkg
        .get("releases")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    let latest_release_date = info
        .get("version")
        .and_then(Value::as_str)
        .and_then(|version| pkg.pointer(&format!("/releases/{version}/0/upload_time")))
        .cloned();

    Ok(json!({
        "url": url,
        "name": info.get("name").cloned(),
        "version": info.get("version").cloned(),
        "summary": info.get("summary").cloned(),
        "homepage": info.get("home_page").cloned(),
        "license": info.get("license").cloned(),
        "license_classifier": pick_license_classifier(info.get("classifiers")),
        "author": info.get("author").cloned(),
        "author_email": info.get("author_email").cloned(),
        "maintainer": info.get("maintainer").cloned(),
        "requires_python": info.get("requires_python").cloned(),
        "requires_dist": info.get("requires_dist").cloned(),
        "keywords": info.get("keywords").cloned(),
        "classifiers": info.get("classifiers").cloned(),
        "yanked": info.get("yanked").cloned(),
        "yanked_reason": info.get("yanked_reason").cloned(),
        "project_urls": info.get("project_urls").cloned(),
        "release_count": release_count,
        "latest_release_date": latest_release_date,
    }))
}

fn parse_project(url: &str) -> Option<(String, Option<String>)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "pypi.org" && host != "www.pypi.org" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 || segs[0] != "project" {
        return None;
    }
    Some((segs[1].to_string(), segs.get(2).map(|value| (*value).to_string())))
}

fn pick_license_classifier(classifiers: Option<&Value>) -> Option<String> {
    classifiers
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .filter(|classifier| classifier.starts_with("License ::"))
        .max_by_key(|classifier| classifier.len())
        .map(ToString::to_string)
}
