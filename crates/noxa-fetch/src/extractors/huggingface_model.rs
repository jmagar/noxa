use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "huggingface_model",
    label: "Hugging Face Model",
    description: "Extract model metadata from Hugging Face.",
    url_patterns: &["https://huggingface.co/*/*"],
};

pub fn matches(url: &str) -> bool {
    parse_owner_name(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let (owner, name) = parse_owner_name(url).ok_or_else(|| {
        FetchError::Build(format!("hf model: cannot parse owner/name from '{url}'"))
    })?;
    let api_url = format!("https://huggingface.co/api/models/{owner}/{name}");
    let model = client.get_json(&api_url).await?;
    let siblings = model
        .get("siblings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(json!({
        "url": url,
        "id": model.get("id").cloned(),
        "model_id": model.get("modelId").cloned(),
        "private": model.get("private").cloned(),
        "gated": model.get("gated").cloned(),
        "downloads": model.get("downloads").cloned(),
        "downloads_30d": model.get("downloadsAllTime").cloned(),
        "likes": model.get("likes").cloned(),
        "library_name": model.get("library_name").cloned(),
        "pipeline_tag": model.get("pipeline_tag").cloned(),
        "tags": model.get("tags").cloned().unwrap_or_else(|| json!([])),
        "license": model.pointer("/cardData/license").cloned(),
        "language": model.pointer("/cardData/language").cloned(),
        "datasets": model.pointer("/cardData/datasets").cloned(),
        "base_model": model.pointer("/cardData/base_model").cloned(),
        "model_type": model.pointer("/cardData/model_type").cloned(),
        "created_at": model.get("createdAt").cloned(),
        "last_modified": model.get("lastModified").cloned(),
        "sha": model.get("sha").cloned(),
        "file_count": siblings.len(),
        "files": siblings,
    }))
}

fn parse_owner_name(url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "huggingface.co" && host != "www.huggingface.co" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() != 2 || RESERVED_NAMESPACES.contains(&segs[0]) {
        return None;
    }
    Some((segs[0].to_string(), segs[1].to_string()))
}

const RESERVED_NAMESPACES: &[&str] = &[
    "datasets",
    "spaces",
    "blog",
    "docs",
    "api",
    "models",
    "papers",
    "pricing",
    "tasks",
    "join",
    "login",
    "settings",
    "organizations",
    "new",
    "search",
];
