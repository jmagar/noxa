use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::NoxaMcpError;

#[derive(Debug, Clone)]
pub struct ResearchArtifacts {
    pub report_path: Option<PathBuf>,
    pub json_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchRequest {
    pub query: String,
    pub deep: bool,
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedResearch {
    schema_version: u32,
    request: ResearchRequest,
    response: Value,
}

pub fn slugify(query: &str) -> String {
    let s: String = query
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
    if s.len() > 60 { s[..60].to_string() } else { s }
}

pub fn cache_key(request: &ResearchRequest) -> String {
    let payload = serde_json::to_vec(request).expect("research request should serialize");
    let digest = Sha256::digest(payload);
    let suffix = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{}-{suffix}", slugify(&request.query))
}

pub fn load_cached_research(
    dir: &Path,
    request: &ResearchRequest,
) -> Result<Option<(Value, ResearchArtifacts)>, NoxaMcpError> {
    let key = cache_key(request);
    let json_path = dir.join(format!("{key}.json"));
    if !json_path.exists() {
        return Ok(None);
    }

    let json_str =
        std::fs::read_to_string(&json_path).map_err(|source| NoxaMcpError::ReadFile {
            path: json_path.clone(),
            source,
        })?;
    let cached: CachedResearch =
        serde_json::from_str(&json_str).map_err(|source| NoxaMcpError::ParseFile {
            path: json_path.clone(),
            source,
        })?;
    if cached.request != *request {
        return Ok(None);
    }

    let report_path = if cached
        .response
        .get("report")
        .and_then(|value| value.as_str())
        .is_some()
    {
        let report_path = dir.join(format!("{key}.md"));
        if !report_path.exists() {
            return Ok(None);
        }
        Some(report_path)
    } else {
        None
    };

    Ok(Some((
        cached.response,
        ResearchArtifacts {
            report_path,
            json_path,
        },
    )))
}

pub fn save_research(
    dir: &Path,
    request: &ResearchRequest,
    data: &Value,
) -> Result<ResearchArtifacts, NoxaMcpError> {
    let key = cache_key(request);
    let json_path = dir.join(format!("{key}.json"));
    let report_path = dir.join(format!("{key}.md"));
    let envelope = CachedResearch {
        schema_version: 1,
        request: request.clone(),
        response: data.clone(),
    };
    let json_str =
        serde_json::to_string_pretty(&envelope).map_err(|source| NoxaMcpError::Serialization {
            context: "research response",
            source,
        })?;

    let report_path = if let Some(report) = data.get("report").and_then(|v| v.as_str()) {
        std::fs::write(&report_path, report).map_err(|source| NoxaMcpError::WriteFile {
            path: report_path.clone(),
            source,
        })?;
        Some(report_path)
    } else {
        None
    };

    std::fs::write(&json_path, json_str).map_err(|source| NoxaMcpError::WriteFile {
        path: json_path.clone(),
        source,
    })?;

    Ok(ResearchArtifacts {
        report_path,
        json_path,
    })
}

pub fn build_research_response(
    query: &str,
    data: &Value,
    artifacts: &ResearchArtifacts,
    cached: bool,
) -> Value {
    let mut response = json!({
        "status": "completed",
        "query": query,
        "json_file": artifacts.json_path.to_string_lossy(),
        "sources_count": data.get("sources_count").cloned().unwrap_or(json!(0)),
        "findings_count": data.get("findings_count").cloned().unwrap_or(json!(0)),
    });

    if let Some(ref report_path) = artifacts.report_path {
        response["report_file"] = json!(report_path.to_string_lossy());
    }
    if cached {
        response["cached"] = json!(true);
    }
    if let Some(findings) = data.get("findings") {
        response["findings"] = findings.clone();
    }
    if let Some(sources) = data.get("sources") {
        response["sources"] = sources.clone();
    }

    response
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn request(query: &str, deep: bool, topic: Option<&str>) -> ResearchRequest {
        ResearchRequest {
            query: query.to_string(),
            deep,
            topic: topic.map(str::to_string),
        }
    }

    #[test]
    fn cache_key_uses_full_request_identity() {
        let base = request("Hello?", false, None);
        let deep = request("Hello?", true, None);
        let topic = request("Hello?", false, Some("science"));
        let punctuation_collision = request("Hello!", false, None);

        assert_ne!(cache_key(&base), cache_key(&deep));
        assert_ne!(cache_key(&base), cache_key(&topic));
        assert_ne!(cache_key(&base), cache_key(&punctuation_collision));
    }

    #[test]
    fn load_cached_research_verifies_request_and_artifacts() {
        let dir = tempdir().unwrap();
        let first = request("Hello!", false, None);
        let second = request("Hello?", false, None);
        let response = json!({
            "status": "completed",
            "report": "# report",
            "findings": ["one"],
        });

        let first_artifacts = save_research(dir.path(), &first, &response).unwrap();
        assert!(first_artifacts.json_path.exists());
        assert!(first_artifacts.report_path.as_ref().unwrap().exists());

        let cached = load_cached_research(dir.path(), &first).unwrap();
        assert!(cached.is_some());

        let collision = load_cached_research(dir.path(), &second).unwrap();
        assert!(collision.is_none());

        std::fs::remove_file(first_artifacts.report_path.unwrap()).unwrap();
        let missing_report = load_cached_research(dir.path(), &first).unwrap();
        assert!(missing_report.is_none());
    }

    #[test]
    fn save_research_only_returns_confirmed_artifact_paths() {
        let dir = tempdir().unwrap();
        let request = request("No report", false, None);
        let response = json!({
            "status": "completed",
            "findings": [],
        });

        let artifacts = save_research(dir.path(), &request, &response).unwrap();
        assert!(artifacts.json_path.exists());
        assert!(artifacts.report_path.is_none());
    }

    #[test]
    fn save_research_propagates_write_failures() {
        let dir = tempdir().unwrap();
        let blocking_file = dir.path().join("not-a-directory");
        std::fs::write(&blocking_file, "x").unwrap();

        let err = save_research(
            &blocking_file,
            &request("Blocked", false, None),
            &json!({
                "status": "completed",
                "report": "# blocked",
            }),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("failed to write"));
    }
}
