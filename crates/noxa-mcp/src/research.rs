use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::error::NoxaMcpError;

#[derive(Debug, Clone)]
pub struct ResearchArtifacts {
    pub report_path: PathBuf,
    pub json_path: PathBuf,
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

pub fn load_cached_research(dir: &Path, slug: &str) -> Result<Option<Value>, NoxaMcpError> {
    let json_path = dir.join(format!("{slug}.json"));
    let report_path = dir.join(format!("{slug}.md"));

    if !json_path.exists() || !report_path.exists() {
        return Ok(None);
    }

    let json_str =
        std::fs::read_to_string(&json_path).map_err(|source| NoxaMcpError::ReadFile {
            path: json_path.clone(),
            source,
        })?;
    let data = serde_json::from_str(&json_str).map_err(|source| NoxaMcpError::ParseFile {
        path: json_path.clone(),
        source,
    })?;
    Ok(Some(data))
}

pub fn save_research(
    dir: &Path,
    slug: &str,
    data: &Value,
) -> Result<ResearchArtifacts, NoxaMcpError> {
    let json_path = dir.join(format!("{slug}.json"));
    let report_path = dir.join(format!("{slug}.md"));
    let json_str =
        serde_json::to_string_pretty(data).map_err(|source| NoxaMcpError::Serialization {
            context: "research response",
            source,
        })?;

    std::fs::write(&json_path, json_str).map_err(|source| NoxaMcpError::WriteFile {
        path: json_path.clone(),
        source,
    })?;

    if let Some(report) = data.get("report").and_then(|v| v.as_str()) {
        std::fs::write(&report_path, report).map_err(|source| NoxaMcpError::WriteFile {
            path: report_path.clone(),
            source,
        })?;
    }

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
        "report_file": artifacts.report_path.to_string_lossy(),
        "json_file": artifacts.json_path.to_string_lossy(),
        "sources_count": data.get("sources_count").cloned().unwrap_or(json!(0)),
        "findings_count": data.get("findings_count").cloned().unwrap_or(json!(0)),
    });

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
