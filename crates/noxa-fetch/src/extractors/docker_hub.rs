use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "docker_hub",
    label: "Docker Hub Repository",
    description: "Extract repository metadata from Docker Hub.",
    url_patterns: &["https://hub.docker.com/r/*", "https://hub.docker.com/_/*"],
};

pub fn matches(url: &str) -> bool {
    parse_repo(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (namespace, name) = parse_repo(url)
        .ok_or_else(|| FetchError::Build(format!("docker_hub: cannot parse repo from '{url}'")))?;
    let api_url = format!("https://hub.docker.com/v2/repositories/{namespace}/{name}");
    let r = client.get_json(&api_url).await?;

    Ok(json!({
        "url": url,
        "namespace": r.get("namespace").cloned(),
        "name": r.get("name").cloned(),
        "full_name": format!("{namespace}/{name}"),
        "pull_count": r.get("pull_count").cloned(),
        "star_count": r.get("star_count").cloned(),
        "description": r.get("description").cloned(),
        "full_description": r.get("full_description").cloned(),
        "last_updated": r.get("last_updated").cloned(),
        "date_registered": r.get("date_registered").cloned(),
        "is_official": namespace == "library",
        "is_private": r.get("is_private").cloned(),
        "status_description": r.get("status_description").cloned(),
        "categories": r.get("categories").cloned().unwrap_or_else(|| json!([])),
    }))
}

fn parse_repo(url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.host_str()? != "hub.docker.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    match segs.as_slice() {
        ["_", name, ..] => Some(("library".to_string(), (*name).to_string())),
        ["r", namespace, name, ..] => Some(((*namespace).to_string(), (*name).to_string())),
        _ => None,
    }
}
