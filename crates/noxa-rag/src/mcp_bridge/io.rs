use std::path::{Path, PathBuf};

use noxa_core::{Content, ExtractionResult, Metadata};
use serde::{Deserialize, Serialize};

use crate::RagError;

use super::{BridgeDocument, McpSource, WriteStatus};

pub fn relative_output_path(source: McpSource, external_id: &str) -> PathBuf {
    PathBuf::from("mcp").join(source.as_str()).join(format!(
        "{}-{:016x}.json",
        sanitize_component(external_id),
        stable_component_hash(external_id)
    ))
}

pub async fn write_bridge_document(
    root: &Path,
    document: &BridgeDocument,
) -> Result<WriteStatus, RagError> {
    let path = root.join(relative_output_path(document.source, &document.external_id));
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let payload = StoredExtractionResult {
        extraction: document.extraction.clone(),
        external_id: Some(document.external_id.clone()),
        platform_url: document.platform_url.clone(),
    };
    let serialized = serde_json::to_vec_pretty(&payload)?;
    if tokio::fs::read(&path).await.ok().as_deref() == Some(serialized.as_slice()) {
        return Ok(WriteStatus::Unchanged);
    }

    let tmp_path = temp_output_path(&path);
    tokio::fs::write(&tmp_path, &serialized).await?;
    // Remove destination before rename so the operation succeeds on Windows,
    // where rename(src, dst) errors when dst already exists.
    let _ = tokio::fs::remove_file(&path).await;
    tokio::fs::rename(&tmp_path, &path).await?;
    Ok(WriteStatus::Written)
}

#[allow(clippy::too_many_arguments)]
pub fn build_extraction(
    url: String,
    title: Option<String>,
    published_date: Option<String>,
    author: Option<String>,
    language: Option<String>,
    technologies: Vec<String>,
    markdown: String,
    plain_text: String,
) -> ExtractionResult {
    ExtractionResult {
        metadata: Metadata {
            title,
            description: None,
            author,
            published_date,
            language,
            url: Some(url),
            site_name: None,
            image: None,
            favicon: None,
            word_count: count_words(&plain_text),
            content_hash: None,
            source_type: Some("mcp".to_string()),
            file_path: None,
            last_modified: None,
            is_truncated: None,
            technologies,
            seed_url: None,
            crawl_depth: None,
            search_query: None,
            fetched_at: None,
        },
        content: Content {
            markdown,
            plain_text,
            links: Vec::new(),
            images: Vec::new(),
            code_blocks: Vec::new(),
            raw_html: None,
        },
        domain_data: None,
        vertical_data: None,
        structured_data: Vec::new(),
    }
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

fn stable_component_hash(value: &str) -> u64 {
    use std::hash::{DefaultHasher, Hasher};
    let mut hasher = DefaultHasher::new();
    hasher.write(value.as_bytes());
    hasher.finish()
}

fn temp_output_path(path: &Path) -> PathBuf {
    let suffix = format!(
        "tmp-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    path.with_extension(format!("json.{suffix}"))
}

fn count_words(value: &str) -> usize {
    value.split_whitespace().count()
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct StoredExtractionResult {
    #[serde(flatten)]
    pub extraction: ExtractionResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_url: Option<String>,
}
