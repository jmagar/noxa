use serde::{Deserialize, Serialize};
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
    // ── Ingestion-provenance fields from IngestionContext ───────────────────
    /// Opaque platform id: 'linkding:42', 'memos:7' (Wave 3+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// Native platform UI URL (Wave 3+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_url: Option<String>,
    // ── Web-provenance fields from IngestionContext ─────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crawl_depth: Option<u32>,
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
}

/// RAG-pipeline provenance carried alongside ExtractionResult through ingestion.
///
/// These fields have no meaning to noxa-fetch, noxa-mcp, or WASM consumers — they
/// live here in noxa-rag, not in noxa-core. At upsert time both Metadata and
/// IngestionContext are serialized into PointPayload.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngestionContext {
    /// Matches Metadata.source_type: 'web' | 'file' | 'mcp' | 'notebook' | 'email'
    pub source_type: String,
    /// SHA-256 hex digest — duplicated from Metadata.content_hash for fast access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    // Platform fields — populated when MCP sources land in Wave 3.
    /// Opaque platform identifier: 'linkding:42', 'memos:7', 'paperless:15'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// Native UI URL (not the canonical content URL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_url: Option<String>,
    // AI session fields — populated when AI session sources land.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    // Web provenance — populated by noxa-fetch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crawl_depth: Option<u32>,
}
