use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

/// A chunk produced from an ExtractionResult.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    pub source_url: String,
    pub domain: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub char_offset: usize,
    pub token_estimate: usize,
    /// The nearest preceding markdown heading (h1–h3) for this chunk, if any.
    pub section_header: Option<String>,
}

/// A point ready for upsert into the vector store.
#[derive(Debug, Clone)]
pub struct Point {
    /// UUID v5 deterministic ID: url#chunkN
    pub id: Uuid,
    pub vector: Vec<f32>,
    pub payload: PointPayload,
}

/// Payload stored alongside each vector in the store.
///
/// All optional fields use `skip_serializing_if = "Option::is_none"` so existing
/// Qdrant points (stored by older pipeline versions) return null for new keys —
/// Qdrant is safe to add new nullable payload fields without migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointPayload {
    pub text: String,
    /// Normalized URL (strip fragment, trailing slash, lowercase scheme+host).
    pub url: String,
    pub domain: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub token_estimate: usize,
    // ── Metadata fields from noxa-core ─────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// 'web' | 'file' | 'mcp' | 'notebook' | 'email'
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    /// SHA-256 hex digest of raw source bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub technologies: Vec<String>,
    /// True when the document was cut at max_chunks_per_page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_truncated: Option<bool>,
    // ── File-source fields ──────────────────────────────────────────────────
    /// Absolute filesystem path (file:// sources only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// ISO 8601 mtime for files, published_at for web content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    /// Git branch detected from .git/HEAD walk-up (file:// sources only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    // ── Ingestion-provenance fields ─────────────────────────────────────────
    /// Opaque platform id: 'linkding:42', 'memos:7' (Wave 3+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// Native platform UI URL (Wave 3+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_url: Option<String>,
    // ── Web-provenance fields ────────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crawl_depth: Option<u32>,
    // ── Source-specific metadata ─────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub email_to: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_has_attachments: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pptx_slide_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pptx_has_notes: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle_start_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle_end_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle_source_file: Option<String>,
    // ── Structural metadata ──────────────────────────────────────────────────
    /// Nearest preceding markdown h1–h3 heading for this chunk, if detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_header: Option<String>,
    /// xxHash3 hex digest of raw file bytes — used by the startup delta scan to
    /// skip files whose on-disk contents have not changed since last index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
}

impl PointPayload {
    pub fn to_qdrant_payload(&self) -> HashMap<String, Value> {
        match serde_json::to_value(self).expect("point payload should serialize") {
            Value::Object(map) => map.into_iter().collect(),
            other => panic!("point payload serialized to non-object value: {other:?}"),
        }
    }
}

/// A search result returned by VectorStore::search().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub text: String,
    pub url: String,
    pub score: f32,
    pub chunk_index: usize,
    pub token_estimate: usize,
    // Extended metadata fields (None when stored by older pipeline versions)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub technologies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub email_to: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_has_attachments: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pptx_slide_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pptx_has_notes: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle_start_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle_end_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle_source_file: Option<String>,
}

impl SearchResult {
    pub fn from_qdrant_payload(
        payload: HashMap<String, Value>,
        score: f32,
    ) -> Result<Self, serde_json::Error> {
        let mut map: Map<String, Value> = payload.into_iter().collect();
        map.insert("score".to_string(), Value::from(score));
        serde_json::from_value(Value::Object(map))
    }
}

/// Narrow metadata filter for vector search.
///
/// `hnsw_ef` overrides the per-request HNSW search parameter sent to Qdrant.
/// When `None`, the search layer uses 128 (good interactive-query default).
/// Set higher (e.g. 256) for recall-sensitive batch workloads; lower (e.g. 64)
/// for latency-critical paths where some recall loss is acceptable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchMetadataFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    /// Override HNSW ef parameter for this search request.
    /// Default when None: 128. Qdrant collection default is ef_construct (200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hnsw_ef: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::SearchResult;

    #[test]
    fn search_result_deserializes_landed_file_metadata() {
        let value = serde_json::json!({
            "text": "chunk text",
            "url": "file:///tmp/report.md",
            "score": 0.99,
            "chunk_index": 2,
            "token_estimate": 123,
            "file_path": "/tmp/report.md",
            "last_modified": "2026-04-15T12:34:56Z",
            "git_branch": "main",
            "email_to": ["team@example.com"],
            "email_message_id": "<msg@example.com>",
            "feed_url": "https://example.com/feed.xml",
            "feed_item_id": "entry-1",
            "pptx_slide_count": 12,
            "pptx_has_notes": true,
            "subtitle_start_s": 1.25,
            "subtitle_end_s": 9.75,
            "subtitle_source_file": "demo.mp4"
        });

        let result: SearchResult =
            serde_json::from_value(value).expect("search result should deserialize");

        assert_eq!(result.file_path.as_deref(), Some("/tmp/report.md"));
        assert_eq!(
            result.last_modified.as_deref(),
            Some("2026-04-15T12:34:56Z")
        );
        assert_eq!(result.git_branch.as_deref(), Some("main"));
        assert_eq!(result.email_to, vec!["team@example.com".to_string()]);
        assert_eq!(
            result.email_message_id.as_deref(),
            Some("<msg@example.com>")
        );
        assert_eq!(
            result.feed_url.as_deref(),
            Some("https://example.com/feed.xml")
        );
        assert_eq!(result.feed_item_id.as_deref(), Some("entry-1"));
        assert_eq!(result.pptx_slide_count, Some(12));
        assert_eq!(result.pptx_has_notes, Some(true));
        assert_eq!(result.subtitle_start_s, Some(1.25));
        assert_eq!(result.subtitle_end_s, Some(9.75));
        assert_eq!(result.subtitle_source_file.as_deref(), Some("demo.mp4"));
    }
}
