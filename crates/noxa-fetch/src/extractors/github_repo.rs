use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "github_repo",
    label: "GitHub Repository",
    description: "Extract repository metadata from GitHub.",
    url_patterns: &["https://github.com/*/*"],
};

pub fn matches(url: &str) -> bool {
    parse_owner_repo(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (owner, repo) = parse_owner_repo(url).ok_or_else(|| {
        FetchError::Build(format!("github_repo: cannot parse owner/repo from '{url}'"))
    })?;
    let api_url = format!("https://api.github.com/repos/{owner}/{repo}");
    let r = client.get_json(&api_url).await?;

    Ok(json!({
        "url": url,
        "owner": r.pointer("/owner/login").cloned(),
        "name": r.get("name").cloned(),
        "full_name": r.get("full_name").cloned(),
        "description": r.get("description").cloned(),
        "homepage": r.get("homepage").cloned(),
        "language": r.get("language").cloned(),
        "topics": r.get("topics").cloned().unwrap_or_else(|| json!([])),
        "license": r.pointer("/license/spdx_id").cloned(),
        "license_name": r.pointer("/license/name").cloned(),
        "default_branch": r.get("default_branch").cloned(),
        "stars": r.get("stargazers_count").cloned(),
        "forks": r.get("forks_count").cloned(),
        "watchers": r.get("subscribers_count").cloned(),
        "open_issues": r.get("open_issues_count").cloned(),
        "size_kb": r.get("size").cloned(),
        "archived": r.get("archived").cloned(),
        "fork": r.get("fork").cloned(),
        "is_template": r.get("is_template").cloned(),
        "has_issues": r.get("has_issues").cloned(),
        "has_wiki": r.get("has_wiki").cloned(),
        "has_pages": r.get("has_pages").cloned(),
        "has_discussions": r.get("has_discussions").cloned(),
        "created_at": r.get("created_at").cloned(),
        "updated_at": r.get("updated_at").cloned(),
        "pushed_at": r.get("pushed_at").cloned(),
        "html_url": r.get("html_url").cloned(),
    }))
}

fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "github.com" && host != "www.github.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() != 2 || RESERVED_OWNERS.contains(&segs[0]) {
        return None;
    }
    Some((segs[0].to_string(), segs[1].to_string()))
}

const RESERVED_OWNERS: &[&str] = &[
    "settings",
    "marketplace",
    "explore",
    "topics",
    "trending",
    "collections",
    "events",
    "sponsors",
    "issues",
    "pulls",
    "notifications",
    "new",
    "organizations",
    "login",
    "join",
    "search",
    "about",
];
