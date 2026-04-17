use std::path::{Path, PathBuf};

use serde_json::json;
use tracing::info;

use crate::server::{NoxaMcp, RESEARCH_MAX_POLLS};
use crate::tools::ResearchParams;

impl NoxaMcp {
    pub(super) async fn research_impl(&self, params: ResearchParams) -> Result<String, String> {
        let cloud = self
            .cloud
            .as_ref()
            .ok_or("Research requires NOXA_API_KEY. Get a key at https://noxa.io")?;

        let research_dir = research_dir();
        let slug = slugify(&params.query);

        if let Some(cached) = load_cached_research(&research_dir, &slug) {
            info!(query = %params.query, "returning cached research");
            return Ok(cached);
        }

        let mut body = json!({ "query": params.query });
        if let Some(deep) = params.deep {
            body["deep"] = json!(deep);
        }
        if let Some(ref topic) = params.topic {
            body["topic"] = json!(topic);
        }

        let start_resp = cloud.post("research", body).await?;
        let job_id = start_resp
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Research API did not return a job ID")?
            .to_string();

        info!(job_id = %job_id, "research job started, polling for completion");

        for poll in 0..RESEARCH_MAX_POLLS {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let status_resp = cloud.get(&format!("research/{job_id}")).await?;
            let status = status_resp
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            match status {
                "completed" => {
                    return Ok(build_completed_response(
                        &params,
                        &research_dir,
                        &slug,
                        &status_resp,
                    ));
                }
                "failed" => {
                    let error = status_resp
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown error");
                    return Err(format!("Research job failed: {error}"));
                }
                _ => {
                    if poll % 20 == 19 {
                        info!(job_id = %job_id, poll, "research still in progress...");
                    }
                }
            }
        }

        Err(format!(
            "Research job {job_id} timed out after ~10 minutes of polling. \
             Check status manually via the noxa API: GET /v1/research/{job_id}"
        ))
    }
}

fn build_completed_response(
    params: &ResearchParams,
    research_dir: &Path,
    slug: &str,
    status_resp: &serde_json::Value,
) -> String {
    let (report_path, json_path) = save_research(research_dir, slug, status_resp);
    let sources_count = status_resp
        .get("sources_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let findings_count = status_resp
        .get("findings_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let mut response = json!({
        "status": "completed",
        "query": params.query,
        "report_file": report_path,
        "json_file": json_path,
        "sources_count": sources_count,
        "findings_count": findings_count,
    });

    if let Some(findings) = status_resp.get("findings") {
        response["findings"] = findings.clone();
    }
    if let Some(sources) = status_resp.get("sources") {
        response["sources"] = sources.clone();
    }

    serde_json::to_string_pretty(&response).unwrap_or_default()
}

fn research_dir() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".noxa")
        .join("research");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn slugify(query: &str) -> String {
    let slug = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    if slug.len() > 60 {
        slug[..60].to_string()
    } else {
        slug
    }
}

fn load_cached_research(dir: &Path, slug: &str) -> Option<String> {
    let json_path = dir.join(format!("{slug}.json"));
    let report_path = dir.join(format!("{slug}.md"));

    if !json_path.exists() || !report_path.exists() {
        return None;
    }

    let json_str = std::fs::read_to_string(&json_path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    let mut response = json!({
        "status": "completed",
        "cached": true,
        "query": data.get("query").cloned().unwrap_or(json!("")),
        "report_file": report_path.to_string_lossy(),
        "json_file": json_path.to_string_lossy(),
        "sources_count": data.get("sources_count").cloned().unwrap_or(json!(0)),
        "findings_count": data.get("findings_count").cloned().unwrap_or(json!(0)),
    });

    if let Some(findings) = data.get("findings") {
        response["findings"] = findings.clone();
    }
    if let Some(sources) = data.get("sources") {
        response["sources"] = sources.clone();
    }

    Some(serde_json::to_string_pretty(&response).unwrap_or_default())
}

fn save_research(dir: &Path, slug: &str, data: &serde_json::Value) -> (String, String) {
    let json_path = dir.join(format!("{slug}.json"));
    let report_path = dir.join(format!("{slug}.md"));

    if let Ok(json_str) = serde_json::to_string_pretty(data) {
        std::fs::write(&json_path, json_str).ok();
    }
    if let Some(report) = data.get("report").and_then(|v| v.as_str()) {
        std::fs::write(&report_path, report).ok();
    }

    (
        report_path.to_string_lossy().to_string(),
        json_path.to_string_lossy().to_string(),
    )
}
