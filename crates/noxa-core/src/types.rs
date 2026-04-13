/// Core types for extraction output.
/// All types are serializable for JSON output to LLM consumers.
use serde::{Deserialize, Serialize};

use crate::domain::DomainType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub metadata: Metadata,
    pub content: Content,
    pub domain_data: Option<DomainData>,
    /// JSON-LD structured data extracted from `<script type="application/ld+json">` blocks.
    /// Contains Schema.org markup (Product, Article, BreadcrumbList, etc.) when present.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_data: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub language: Option<String>,
    pub url: Option<String>,
    pub site_name: Option<String>,
    pub image: Option<String>,
    pub favicon: Option<String>,
    pub word_count: usize,
    // RAG-pipeline fields (all Option<T> for backward compat with existing web extraction callers)
    /// SHA-256 hex digest of the raw source bytes. Used as a dedup key by noxa-rag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Source classification: 'web' | 'file' | 'mcp' | 'notebook' | 'email'
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    /// Absolute filesystem path — populated for file:// sources only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// ISO 8601 timestamp: fs mtime for files, published_at for web content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    /// True when the document hit the max_chunks_per_page limit and was cut short.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_truncated: Option<bool>,
    /// Detected tech stack (e.g. ["React", "TypeScript", "Tailwind"]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub technologies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub markdown: String,
    pub plain_text: String,
    pub links: Vec<Link>,
    pub images: Vec<Image>,
    pub code_blocks: Vec<CodeBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_html: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Link {
    pub text: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub alt: String,
    pub src: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub code: String,
}

/// Domain-specific extracted data. For MVP, only the detected type is stored.
/// Future: each variant carries structured fields (e.g., Article { author, date, ... }).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainData {
    pub domain_type: DomainType,
}

/// Options for controlling content extraction behavior.
#[derive(Debug, Clone, Default)]
pub struct ExtractionOptions {
    /// CSS selectors for elements to include. If non-empty, only these elements
    /// are extracted (skipping the scoring algorithm entirely).
    pub include_selectors: Vec<String>,
    /// CSS selectors for elements to exclude from the output.
    pub exclude_selectors: Vec<String>,
    /// If true, skip scoring and pick the first `article`, `main`, or `[role="main"]` element.
    pub only_main_content: bool,
    /// If true, populate `Content::raw_html` with the extracted content's HTML.
    pub include_raw_html: bool,
}
