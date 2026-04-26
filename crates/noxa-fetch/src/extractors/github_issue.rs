use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "github_issue",
    label: "GitHub Issue",
    description: "Extract issue metadata from GitHub.",
    url_patterns: &["https://github.com/*/*/issues/*"],
};

pub fn matches(url: &str) -> bool {
    parse_issue(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (owner, repo, number) = parse_issue(url).ok_or_else(|| {
        FetchError::Build(format!("github_issue: cannot parse issue URL '{url}'"))
    })?;
    let api_url = format!("https://api.github.com/repos/{owner}/{repo}/issues/{number}");
    let issue = client.get_json(&api_url).await?;
    if issue.get("pull_request").is_some() {
        return Err(FetchError::Build(format!(
            "github_issue: '{owner}/{repo}#{number}' is a pull request, use github_pr"
        )));
    }

    Ok(json!({
        "url": url,
        "owner": owner,
        "repo": repo,
        "number": issue.get("number").cloned(),
        "title": issue.get("title").cloned(),
        "body": issue.get("body").cloned(),
        "state": issue.get("state").cloned(),
        "state_reason": issue.get("state_reason").cloned(),
        "author": issue.pointer("/user/login").cloned(),
        "labels": names_array(issue.get("labels")),
        "assignees": logins_array(issue.get("assignees")),
        "milestone": issue.pointer("/milestone/title").cloned(),
        "comments": issue.get("comments").cloned(),
        "locked": issue.get("locked").cloned(),
        "created_at": issue.get("created_at").cloned(),
        "updated_at": issue.get("updated_at").cloned(),
        "closed_at": issue.get("closed_at").cloned(),
        "html_url": issue.get("html_url").cloned(),
    }))
}

fn parse_issue(url: &str) -> Option<(String, String, u64)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "github.com" && host != "www.github.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() < 4 || segs[2] != "issues" {
        return None;
    }
    Some((
        segs[0].to_string(),
        segs[1].to_string(),
        segs[3].parse().ok()?,
    ))
}

fn names_array(value: Option<&Value>) -> Value {
    let names: Vec<_> = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("name").cloned())
        .collect();
    json!(names)
}

fn logins_array(value: Option<&Value>) -> Value {
    let logins: Vec<_> = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("login").cloned())
        .collect();
    json!(logins)
}
