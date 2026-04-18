use std::collections::HashMap;

use serde::Deserialize;

use crate::error::RagError;
use crate::types::{Point, PointPayload, SearchMetadataFilter, SearchResult};

pub(crate) fn point_payload_to_map(
    payload: &PointPayload,
) -> Result<HashMap<String, serde_json::Value>, RagError> {
    let value = serde_json::to_value(payload)
        .map_err(|error| RagError::Store(format!("point payload serialization failed: {error}")))?;
    let map = value
        .as_object()
        .cloned()
        .ok_or_else(|| RagError::Store("point payload is not a JSON object".to_string()))?;
    Ok(map.into_iter().collect())
}

pub(crate) fn point_to_qdrant_payload(point: &Point) -> Result<super::http::QdrantPoint, RagError> {
    Ok(super::http::QdrantPoint {
        id: point.id.to_string(),
        vector: point.vector.clone(),
        payload: point_payload_to_map(&point.payload)?,
    })
}

pub(crate) fn search_filter(filter: Option<&SearchMetadataFilter>) -> Option<serde_json::Value> {
    filter.and_then(|filter| {
        let mut must = Vec::new();
        if let Some(value) = &filter.file_path {
            must.push(serde_json::json!({ "key": "file_path", "match": { "value": value } }));
        }
        if let Some(value) = &filter.last_modified {
            must.push(serde_json::json!({ "key": "last_modified", "match": { "value": value } }));
        }
        if let Some(value) = &filter.git_branch {
            must.push(serde_json::json!({ "key": "git_branch", "match": { "value": value } }));
        }
        if must.is_empty() {
            None
        } else {
            Some(serde_json::json!({ "must": must }))
        }
    })
}

pub(crate) fn search_result_from_payload(
    score: f32,
    payload: HashMap<String, serde_json::Value>,
) -> Result<SearchResult, serde_json::Error> {
    let json = serde_json::Value::Object(payload.into_iter().collect());
    #[derive(Debug, Deserialize)]
    struct SearchResultPayload {
        text: String,
        url: String,
        #[serde(default)]
        chunk_index: usize,
        #[serde(default)]
        token_estimate: usize,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        author: Option<String>,
        #[serde(default)]
        published_date: Option<String>,
        #[serde(default)]
        language: Option<String>,
        #[serde(default)]
        source_type: Option<String>,
        #[serde(default)]
        content_hash: Option<String>,
        #[serde(default)]
        technologies: Vec<String>,
        #[serde(default)]
        file_path: Option<String>,
        #[serde(default)]
        last_modified: Option<String>,
        #[serde(default)]
        git_branch: Option<String>,
        #[serde(default)]
        email_to: Vec<String>,
        #[serde(default)]
        email_message_id: Option<String>,
        #[serde(default)]
        email_thread_id: Option<String>,
        #[serde(default)]
        email_has_attachments: Option<bool>,
        #[serde(default)]
        feed_url: Option<String>,
        #[serde(default)]
        feed_item_id: Option<String>,
        #[serde(default)]
        pptx_slide_count: Option<u32>,
        #[serde(default)]
        pptx_has_notes: Option<bool>,
        #[serde(default)]
        subtitle_start_s: Option<f64>,
        #[serde(default)]
        subtitle_end_s: Option<f64>,
        #[serde(default)]
        subtitle_source_file: Option<String>,
    }

    let point_payload: SearchResultPayload = serde_json::from_value(json)?;

    Ok(SearchResult {
        text: point_payload.text,
        url: point_payload.url,
        score,
        chunk_index: point_payload.chunk_index,
        token_estimate: point_payload.token_estimate,
        title: point_payload.title,
        author: point_payload.author,
        published_date: point_payload.published_date,
        language: point_payload.language,
        source_type: point_payload.source_type,
        content_hash: point_payload.content_hash,
        technologies: point_payload.technologies,
        file_path: point_payload.file_path,
        last_modified: point_payload.last_modified,
        git_branch: point_payload.git_branch,
        email_to: point_payload.email_to,
        email_message_id: point_payload.email_message_id,
        email_thread_id: point_payload.email_thread_id,
        email_has_attachments: point_payload.email_has_attachments,
        feed_url: point_payload.feed_url,
        feed_item_id: point_payload.feed_item_id,
        pptx_slide_count: point_payload.pptx_slide_count,
        pptx_has_notes: point_payload.pptx_has_notes,
        subtitle_start_s: point_payload.subtitle_start_s,
        subtitle_end_s: point_payload.subtitle_end_s,
        subtitle_source_file: point_payload.subtitle_source_file,
    })
}
