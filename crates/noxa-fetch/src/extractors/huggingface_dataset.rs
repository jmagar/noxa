use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "huggingface_dataset",
    label: "Hugging Face Dataset",
    description: "Extract dataset metadata from Hugging Face.",
    url_patterns: &["https://huggingface.co/datasets/*"],
};

pub fn matches(url: &str) -> bool {
    parse_dataset_path(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let dataset_path = parse_dataset_path(url).ok_or_else(|| {
        FetchError::Build(format!(
            "hf_dataset: cannot parse dataset path from '{url}'"
        ))
    })?;
    let api_url = format!("https://huggingface.co/api/datasets/{dataset_path}");
    let dataset = client.get_json(&api_url).await?;
    let siblings = dataset
        .get("siblings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(json!({
        "url": url,
        "id": dataset.get("id").cloned(),
        "private": dataset.get("private").cloned(),
        "gated": dataset.get("gated").cloned(),
        "downloads": dataset.get("downloads").cloned(),
        "downloads_30d": dataset.get("downloadsAllTime").cloned(),
        "likes": dataset.get("likes").cloned(),
        "tags": dataset.get("tags").cloned().unwrap_or_else(|| json!([])),
        "license": dataset.pointer("/cardData/license").cloned(),
        "language": dataset.pointer("/cardData/language").cloned(),
        "task_categories": dataset.pointer("/cardData/task_categories").cloned(),
        "size_categories": dataset.pointer("/cardData/size_categories").cloned(),
        "annotations_creators": dataset.pointer("/cardData/annotations_creators").cloned(),
        "configs": dataset.pointer("/cardData/configs").cloned(),
        "created_at": dataset.get("createdAt").cloned(),
        "last_modified": dataset.get("lastModified").cloned(),
        "sha": dataset.get("sha").cloned(),
        "file_count": siblings.len(),
        "files": siblings,
    }))
}

fn parse_dataset_path(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "huggingface.co" && host != "www.huggingface.co" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.first() != Some(&"datasets") || !(segs.len() == 2 || segs.len() == 3) {
        return None;
    }
    match segs.as_slice() {
        ["datasets", name] => Some((*name).to_string()),
        ["datasets", owner, name] => Some(format!("{owner}/{name}")),
        _ => None,
    }
}
