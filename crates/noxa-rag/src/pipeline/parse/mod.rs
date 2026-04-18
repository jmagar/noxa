use std::path::Path;

use noxa_core::types::ExtractionResult;
use serde::Deserialize;

use crate::error::RagError;
use crate::types::PointPayload;

mod binary;
mod rich;
mod text;

pub(crate) use binary::{parse_ipynb_file, parse_office_zip_file, parse_pdf_file};
pub(crate) use rich::{parse_email_file, parse_feed_file, parse_subtitle_file};
pub(crate) use text::{
    extract_xml_text, parse_html_file, parse_json_file, parse_jsonl_file, parse_log_file,
    parse_markdown_file, parse_plain_text_file, parse_xml_file,
};

#[derive(Debug, Clone)]
pub(crate) struct ParsedFile {
    pub extraction: ExtractionResult,
    pub provenance: IngestionProvenance,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct IngestionProvenance {
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub platform_url: Option<String>,
    #[serde(default)]
    pub seed_url: Option<String>,
    #[serde(default)]
    pub search_query: Option<String>,
    #[serde(default)]
    pub crawl_depth: Option<u32>,
    #[serde(default)]
    pub email_to: Vec<String>,
    #[serde(default)]
    pub email_message_id: Option<String>,
    #[serde(default)]
    pub email_thread_id: Option<String>,
    #[serde(default)]
    pub email_has_attachments: Option<bool>,
    #[serde(default)]
    pub feed_url: Option<String>,
    #[serde(default)]
    pub feed_item_id: Option<String>,
    #[serde(default)]
    pub pptx_slide_count: Option<u32>,
    #[serde(default)]
    pub pptx_has_notes: Option<bool>,
    #[serde(default)]
    pub subtitle_start_s: Option<f64>,
    #[serde(default)]
    pub subtitle_end_s: Option<f64>,
    #[serde(default)]
    pub subtitle_source_file: Option<String>,
}

pub(crate) async fn parse_file(path: &Path, bytes: Vec<u8>) -> Result<ParsedFile, RagError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "json".to_string());
    let file_url = file_url_for_path(path);
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    match ext.as_str() {
        "json" => parse_json_file(&bytes, file_url, title),
        "md" => Ok(parse_markdown_file(bytes, file_url, title)),
        "rst" | "org" => Ok(parse_markdown_file(bytes, file_url, title)),
        "txt" => Ok(parse_plain_text_file(bytes, file_url, title)),
        "yaml" | "yml" | "toml" => Ok(parse_plain_text_file(bytes, file_url, title)),
        "log" => Ok(parse_log_file(bytes, file_url, title)),
        "html" | "htm" => parse_html_file(bytes, file_url).await,
        "ipynb" => {
            spawn_blocking_parse("ipynb", move || parse_ipynb_file(&bytes, file_url, title)).await
        }
        "pdf" => spawn_blocking_parse("PDF", move || parse_pdf_file(&bytes, file_url, title)).await,
        "docx" => {
            spawn_blocking_parse("DOCX", move || {
                parse_office_zip_file(&bytes, file_url, title, "docx")
            })
            .await
        }
        "odt" => {
            spawn_blocking_parse("ODT", move || {
                parse_office_zip_file(&bytes, file_url, title, "odt")
            })
            .await
        }
        "pptx" => {
            spawn_blocking_parse("PPTX", move || {
                parse_office_zip_file(&bytes, file_url, title, "pptx")
            })
            .await
        }
        "jsonl" => Ok(parse_jsonl_file(bytes, file_url, title)),
        "xml" | "opml" => Ok(parse_xml_file(bytes, file_url, title)),
        "rss" | "atom" => parse_feed_file(bytes, file_url, title),
        "eml" => parse_email_file(&bytes, file_url, title),
        "vtt" | "srt" => Ok(parse_subtitle_file(bytes, file_url, title)),
        other => Err(RagError::Parse(format!(
            "unsupported file extension: .{other}"
        ))),
    }
}

async fn spawn_blocking_parse<F>(label: &'static str, f: F) -> Result<ParsedFile, RagError>
where
    F: FnOnce() -> Result<ParsedFile, RagError> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| RagError::Parse(format!("{label} spawn_blocking: {e}")))?
}

fn file_url_for_path(path: &Path) -> String {
    url::Url::from_file_path(path)
        .or_else(|_| {
            std::env::current_dir()
                .ok()
                .map(|cwd| cwd.join(path))
                .and_then(|abs| url::Url::from_file_path(abs).ok())
                .ok_or(())
        })
        .map(|u| u.to_string())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn json_string(value: &serde_json::Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn json_u32(value: &serde_json::Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .or_else(|| value.as_str().and_then(|s| s.parse::<u32>().ok()))
}

pub(crate) fn extract_ingestion_provenance(value: &serde_json::Value) -> IngestionProvenance {
    let Some(obj) = value.as_object() else {
        return IngestionProvenance::default();
    };

    let metadata = obj.get("metadata").and_then(|v| v.as_object());
    let metadata_value = |key: &str| metadata.and_then(|m| m.get(key));
    let top_value = |key: &str| obj.get(key);

    IngestionProvenance {
        external_id: top_value("external_id").and_then(json_string),
        platform_url: top_value("platform_url").and_then(json_string),
        seed_url: top_value("seed_url")
            .and_then(json_string)
            .or_else(|| metadata_value("seed_url").and_then(json_string)),
        search_query: top_value("search_query")
            .and_then(json_string)
            .or_else(|| metadata_value("search_query").and_then(json_string)),
        crawl_depth: top_value("crawl_depth")
            .and_then(json_u32)
            .or_else(|| metadata_value("crawl_depth").and_then(json_u32)),
        ..IngestionProvenance::default()
    }
}

fn merge_provenance(
    metadata: &noxa_core::Metadata,
    provenance: &IngestionProvenance,
) -> IngestionProvenance {
    IngestionProvenance {
        external_id: provenance.external_id.clone(),
        platform_url: provenance.platform_url.clone(),
        seed_url: provenance
            .seed_url
            .clone()
            .or_else(|| metadata.seed_url.clone()),
        search_query: provenance
            .search_query
            .clone()
            .or_else(|| metadata.search_query.clone()),
        crawl_depth: provenance.crawl_depth.or(metadata.crawl_depth),
        email_to: provenance.email_to.clone(),
        email_message_id: provenance.email_message_id.clone(),
        email_thread_id: provenance.email_thread_id.clone(),
        email_has_attachments: provenance.email_has_attachments,
        feed_url: provenance.feed_url.clone(),
        feed_item_id: provenance.feed_item_id.clone(),
        pptx_slide_count: provenance.pptx_slide_count,
        pptx_has_notes: provenance.pptx_has_notes,
        subtitle_start_s: provenance.subtitle_start_s,
        subtitle_end_s: provenance.subtitle_end_s,
        subtitle_source_file: provenance.subtitle_source_file.clone(),
    }
}

pub(crate) fn build_point_payload(
    chunk: &crate::types::Chunk,
    result: &ExtractionResult,
    git_branch: Option<String>,
    provenance: &IngestionProvenance,
    url: &str,
) -> PointPayload {
    let provenance = merge_provenance(&result.metadata, provenance);

    PointPayload {
        text: chunk.text.clone(),
        url: url.to_string(),
        domain: chunk.domain.clone(),
        chunk_index: chunk.chunk_index,
        total_chunks: chunk.total_chunks,
        token_estimate: chunk.token_estimate,
        title: result.metadata.title.clone(),
        author: result.metadata.author.clone(),
        published_date: result.metadata.published_date.clone(),
        language: result.metadata.language.clone(),
        source_type: result.metadata.source_type.clone(),
        content_hash: result.metadata.content_hash.clone(),
        technologies: result.metadata.technologies.clone(),
        is_truncated: result.metadata.is_truncated,
        file_path: result.metadata.file_path.clone(),
        last_modified: result.metadata.last_modified.clone(),
        git_branch,
        external_id: provenance.external_id,
        platform_url: provenance.platform_url,
        seed_url: provenance.seed_url,
        search_query: provenance.search_query,
        crawl_depth: provenance.crawl_depth,
        email_to: provenance.email_to,
        email_message_id: provenance.email_message_id,
        email_thread_id: provenance.email_thread_id,
        email_has_attachments: provenance.email_has_attachments,
        feed_url: provenance.feed_url,
        feed_item_id: provenance.feed_item_id,
        pptx_slide_count: provenance.pptx_slide_count,
        pptx_has_notes: provenance.pptx_has_notes,
        subtitle_start_s: provenance.subtitle_start_s,
        subtitle_end_s: provenance.subtitle_end_s,
        subtitle_source_file: provenance.subtitle_source_file,
    }
}

pub(crate) fn make_text_result(
    markdown: String,
    plain_text: String,
    url: String,
    title: Option<String>,
    source_type: &str,
    word_count: usize,
) -> ExtractionResult {
    ExtractionResult {
        metadata: noxa_core::Metadata {
            title,
            description: None,
            author: None,
            published_date: None,
            language: None,
            url: Some(url),
            site_name: None,
            image: None,
            favicon: None,
            word_count,
            content_hash: None,
            source_type: Some(source_type.to_string()),
            file_path: None,
            last_modified: None,
            is_truncated: None,
            technologies: Vec::new(),
            seed_url: None,
            crawl_depth: None,
            search_query: None,
            fetched_at: None,
        },
        content: noxa_core::Content {
            markdown,
            plain_text,
            links: Vec::new(),
            images: Vec::new(),
            code_blocks: Vec::new(),
            raw_html: None,
        },
        domain_data: None,
        structured_data: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
