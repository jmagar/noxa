use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "github_pr",
    label: "GitHub Pull Request",
    description: "Extract pull request metadata from GitHub.",
    url_patterns: &["https://github.com/*/*/pull/*"],
};

pub fn matches(url: &str) -> bool {
    parse_pr(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (owner, repo, number) = parse_pr(url).ok_or_else(|| {
        FetchError::Build(format!("github_pr: cannot parse pull-request URL '{url}'"))
    })?;
    let api_url = format!("https://api.github.com/repos/{owner}/{repo}/pulls/{number}");
    let p = client.get_json(&api_url).await?;

    Ok(json!({
        "url": url,
        "owner": owner,
        "repo": repo,
        "number": p.get("number").cloned(),
        "title": p.get("title").cloned(),
        "body": p.get("body").cloned(),
        "state": p.get("state").cloned(),
        "draft": p.get("draft").cloned(),
        "merged": p.get("merged").cloned(),
        "merged_at": p.get("merged_at").cloned(),
        "merge_commit_sha": p.get("merge_commit_sha").cloned(),
        "author": p.pointer("/user/login").cloned(),
        "labels": names_array(p.get("labels")),
        "milestone": p.pointer("/milestone/title").cloned(),
        "head_ref": p.pointer("/head/ref").cloned(),
        "base_ref": p.pointer("/base/ref").cloned(),
        "head_sha": p.pointer("/head/sha").cloned(),
        "additions": p.get("additions").cloned(),
        "deletions": p.get("deletions").cloned(),
        "changed_files": p.get("changed_files").cloned(),
        "commits": p.get("commits").cloned(),
        "comments": p.get("comments").cloned(),
        "review_comments": p.get("review_comments").cloned(),
        "created_at": p.get("created_at").cloned(),
        "updated_at": p.get("updated_at").cloned(),
        "closed_at": p.get("closed_at").cloned(),
        "html_url": p.get("html_url").cloned(),
    }))
}

fn parse_pr(url: &str) -> Option<(String, String, u64)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "github.com" && host != "www.github.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() < 4 || (segs[2] != "pull" && segs[2] != "pulls") {
        return None;
    }
    Some((segs[0].to_string(), segs[1].to_string(), segs[3].parse().ok()?))
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
