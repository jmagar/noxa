use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::error::RagError;

/// Wrapper that owns the `[rag]` section of noxa.toml.
#[derive(Debug, Deserialize)]
struct TomlRoot {
    rag: Option<RagConfig>,
}

/// RAG pipeline configuration from the `[rag]` section of noxa.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct RagConfig {
    pub source: SourceConfig,
    pub embed_provider: EmbedProviderConfig,
    pub vector_store: VectorStoreConfig,
    pub chunker: ChunkerConfig,
    pub pipeline: PipelineConfig,
    /// UUID namespace for deterministic point IDs.
    /// Default: 6ba7b810-9dad-11d1-80b4-00c04fd430c8
    #[serde(default = "default_uuid_namespace")]
    pub uuid_namespace: uuid::Uuid,
}

fn default_uuid_namespace() -> uuid::Uuid {
    uuid::Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap()
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceConfig {
    FsWatcher {
        /// Primary field — always non-empty after `load_config` normalization.
        #[serde(default)]
        watch_dirs: Vec<PathBuf>,
        /// Legacy single-dir form. Consumed during normalization; always `None` post-load.
        #[serde(default)]
        watch_dir: Option<PathBuf>,
        #[serde(default = "default_debounce_ms")]
        debounce_ms: u64,
    },
}

pub(crate) fn default_debounce_ms() -> u64 {
    500
}

fn normalize_source(config: &mut RagConfig) -> Result<(), RagError> {
    match &mut config.source {
        SourceConfig::FsWatcher {
            watch_dirs,
            watch_dir,
            ..
        } => {
            let has_dirs = !watch_dirs.is_empty();
            let has_legacy = watch_dir.is_some();

            if has_dirs && has_legacy {
                return Err(RagError::Config(
                    "set watch_dir and watch_dirs simultaneously".to_string(),
                ));
            }
            if !has_dirs && !has_legacy {
                return Err(RagError::Config(
                    "watch_dirs must not be empty".to_string(),
                ));
            }
            if has_legacy {
                *watch_dirs = vec![watch_dir.take().unwrap()];
            }
            Ok(())
        }
    }
}

fn validate_supported_backends(config: &RagConfig) -> Result<(), RagError> {
    match &config.embed_provider {
        EmbedProviderConfig::Tei { .. } => {}
        EmbedProviderConfig::OpenAi { .. } => {
            return Err(RagError::Config(
                "rag.embed_provider.type = \"open_ai\" is not supported in this build; use \"tei\""
                    .to_string(),
            ));
        }
        EmbedProviderConfig::VoyageAi { .. } => {
            return Err(RagError::Config(
                "rag.embed_provider.type = \"voyage_ai\" is not supported in this build; use \"tei\""
                    .to_string(),
            ));
        }
    }

    match &config.vector_store {
        VectorStoreConfig::Qdrant { .. } => {}
        VectorStoreConfig::InMemory => {
            return Err(RagError::Config(
                "rag.vector_store.type = \"in_memory\" is not supported in this build; use \"qdrant\""
                    .to_string(),
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EmbedProviderConfig {
    Tei {
        url: String,
        model: String,
        /// Optional: load tokenizer from local path (avoids HF Hub at startup).
        local_path: Option<PathBuf>,
        /// Optional Bearer token sent as `Authorization: Bearer <token>` on every
        /// TEI request. When `None`, no auth header is sent (backward-compatible).
        #[serde(default)]
        auth_token: Option<String>,
        /// Optional instruction prefix applied to search-time query embeddings only.
        ///
        /// Required for instruction-tuned models such as Qwen3-Embedding-0.6B.
        /// Documents are indexed as plain text — no prefix. Queries are formatted as:
        ///   `Instruct: {instruction}\nQuery: {query_text}`
        ///
        /// Default: "Given a web search query, retrieve relevant passages that answer the query"
        #[serde(default = "default_query_instruction")]
        query_instruction: Option<String>,
        /// MRL dimension override — truncate vectors client-side after embedding.
        ///
        /// Qwen3-Embedding-0.6B supports Matryoshka Representation Learning: any prefix
        /// of the 1024-dim vector is meaningful. Set to 512 or 256 to reduce Qdrant
        /// storage at a small quality cost (~3% and ~7% respectively).
        ///
        /// Must be ≤ the model's probed output dimensions. Changing this on an existing
        /// collection requires deleting and recreating it (dimension mismatch at startup
        /// will produce a clear error).
        ///
        /// Defaults to None (use probed dimensions, typically 1024 for Qwen3-0.6B).
        #[serde(default)]
        dimensions: Option<usize>,
    },
    OpenAi {
        api_key: String,
        model: String,
    },
    VoyageAi {
        api_key: String,
        model: String,
    },
}

fn default_query_instruction() -> Option<String> {
    Some(
        "Given a web search query, retrieve relevant passages that answer the query".to_string(),
    )
}

impl EmbedProviderConfig {
    /// Format a query with the Qwen3-style instruction prefix if configured.
    ///
    /// Returns `Cow::Borrowed` (no allocation) when no instruction is set.
    /// Only applies to the TEI provider — all others pass the query through unchanged.
    /// Never call this on document text during indexing.
    pub fn format_query<'a>(&'a self, query: &'a str) -> std::borrow::Cow<'a, str> {
        match self {
            EmbedProviderConfig::Tei {
                query_instruction: Some(instruction),
                ..
            } => std::borrow::Cow::Owned(format!("Instruct: {instruction}\nQuery: {query}")),
            _ => std::borrow::Cow::Borrowed(query),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VectorStoreConfig {
    Qdrant {
        /// REST URL — port 6333 (e.g. http://127.0.0.1:53333 if port-mapped).
        url: String,
        collection: String,
        /// Optional API key. Override with NOXA_RAG_QDRANT_API_KEY env var.
        api_key: Option<String>,
    },
    /// Dev/test only — factory returns RagError::Config("not implemented").
    InMemory,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkerConfig {
    #[serde(default = "default_target_tokens")]
    pub target_tokens: usize,
    #[serde(default = "default_overlap_tokens")]
    pub overlap_tokens: usize,
    #[serde(default = "default_min_words")]
    pub min_words: usize,
    #[serde(default = "default_max_chunks_per_page")]
    pub max_chunks_per_page: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            target_tokens: default_target_tokens(),
            overlap_tokens: default_overlap_tokens(),
            min_words: default_min_words(),
            max_chunks_per_page: default_max_chunks_per_page(),
        }
    }
}

fn default_target_tokens() -> usize {
    512
}
fn default_overlap_tokens() -> usize {
    64
}
fn default_min_words() -> usize {
    50
}
fn default_max_chunks_per_page() -> usize {
    100
}

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineConfig {
    #[serde(default = "default_embed_concurrency")]
    pub embed_concurrency: usize,
    /// MUST be an absolute path — systemd daemon runs with CWD = /.
    pub failed_jobs_log: Option<PathBuf>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            embed_concurrency: default_embed_concurrency(),
            failed_jobs_log: None,
        }
    }
}

fn default_embed_concurrency() -> usize {
    4
}

/// Load and validate the `[rag]` section from a noxa.toml file.
pub fn load_config(path: &Path) -> Result<RagConfig, RagError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        RagError::Config(format!("cannot read config file {}: {}", path.display(), e))
    })?;

    let root: TomlRoot = toml::from_str(&content)
        .map_err(|e| RagError::Config(format!("config parse error: {}", e)))?;

    let mut config = root.rag.ok_or_else(|| {
        RagError::Config(format!("missing [rag] section in {}", path.display()))
    })?;

    normalize_source(&mut config)?;
    validate_supported_backends(&config)?;

    // Validate embed_concurrency > 0
    if config.pipeline.embed_concurrency == 0 {
        return Err(RagError::Config(
            "pipeline.embed_concurrency must be > 0 or no workers will run".to_string(),
        ));
    }

    // Validate failed_jobs_log is absolute if set
    if let Some(ref log_path) = config.pipeline.failed_jobs_log
        && !log_path.is_absolute()
    {
        return Err(RagError::Config(format!(
            "pipeline.failed_jobs_log must be an absolute path (got: {}). \
             systemd daemon runs with CWD = / and relative paths resolve there.",
            log_path.display()
        )));
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn minimal_toml(source_section: &str) -> String {
        format!(
            r#"
{source_section}

[rag.embed_provider]
type = "tei"
url = "http://127.0.0.1:8080"
model = "test"

[rag.vector_store]
type = "qdrant"
url = "http://127.0.0.1:6333"
collection = "test"

[rag.chunker]

[rag.pipeline]
"#
        )
    }

    fn write_config(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("tempfile");
        f.write_all(content.as_bytes()).expect("write");
        f
    }

    #[test]
    fn load_config_legacy_watch_dir_normalizes_to_watch_dirs() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let toml = minimal_toml(&format!(
            r#"[rag.source]
type = "fs_watcher"
watch_dir = "{}"
"#,
            tmp_dir.path().display()
        ));
        let f = write_config(&toml);
        let config = load_config(f.path()).expect("load_config");
        match &config.source {
            SourceConfig::FsWatcher {
                watch_dirs,
                watch_dir,
                ..
            } => {
                assert_eq!(watch_dirs.len(), 1);
                assert_eq!(watch_dirs[0], tmp_dir.path());
                assert!(watch_dir.is_none());
            }
        }
    }

    #[test]
    fn load_config_watch_dirs_passes_through_unchanged() {
        let tmp1 = tempfile::tempdir().expect("tempdir1");
        let tmp2 = tempfile::tempdir().expect("tempdir2");
        let toml = minimal_toml(&format!(
            r#"[rag.source]
type = "fs_watcher"
watch_dirs = ["{}", "{}"]
"#,
            tmp1.path().display(),
            tmp2.path().display()
        ));
        let f = write_config(&toml);
        let config = load_config(f.path()).expect("load_config");
        match &config.source {
            SourceConfig::FsWatcher { watch_dirs, .. } => {
                assert_eq!(watch_dirs.len(), 2);
            }
        }
    }

    #[test]
    fn load_config_both_set_returns_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let toml = minimal_toml(&format!(
            r#"[rag.source]
type = "fs_watcher"
watch_dir = "{path}"
watch_dirs = ["{path}"]
"#,
            path = tmp.path().display()
        ));
        let f = write_config(&toml);
        let err = load_config(f.path()).expect_err("should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("simultaneously"),
            "error should mention 'simultaneously', got: {msg}"
        );
    }

    #[test]
    fn load_config_neither_set_returns_error() {
        let toml = minimal_toml(
            r#"[rag.source]
type = "fs_watcher"
"#,
        );
        let f = write_config(&toml);
        let err = load_config(f.path()).expect_err("should fail");
        assert!(matches!(err, RagError::Config(_)));
    }

    #[test]
    fn load_config_empty_watch_dirs_returns_error() {
        let toml = minimal_toml(
            r#"[rag.source]
type = "fs_watcher"
watch_dirs = []
"#,
        );
        let f = write_config(&toml);
        let err = load_config(f.path()).expect_err("should fail");
        assert!(matches!(err, RagError::Config(_)));
    }

    #[test]
    fn format_query_applies_instruction_prefix_for_tei() {
        let config = EmbedProviderConfig::Tei {
            url: "http://tei.test".to_string(),
            model: "Qwen3-Embedding-0.6B".to_string(),
            local_path: None,
            auth_token: None,
            query_instruction: Some(
                "Given a web search query, retrieve relevant passages that answer the query"
                    .to_string(),
            ),
            dimensions: None,
        };
        let result = config.format_query("rust async runtime comparison");
        assert_eq!(
            result.as_ref(),
            "Instruct: Given a web search query, retrieve relevant passages that answer the query\nQuery: rust async runtime comparison"
        );
    }

    #[test]
    fn format_query_none_instruction_returns_query_unchanged() {
        let config = EmbedProviderConfig::Tei {
            url: "http://tei.test".to_string(),
            model: "some-model".to_string(),
            local_path: None,
            auth_token: None,
            query_instruction: None,
            dimensions: None,
        };
        let result = config.format_query("my query");
        assert_eq!(result.as_ref(), "my query");
        assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn format_query_openai_returns_query_unchanged() {
        let config = EmbedProviderConfig::OpenAi {
            api_key: "sk-test".to_string(),
            model: "text-embedding-3-small".to_string(),
        };
        let result = config.format_query("my query");
        assert_eq!(result.as_ref(), "my query");
    }

    #[test]
    fn load_config_default_query_instruction_is_set() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let toml = minimal_toml(&format!(
            r#"[rag.source]
type = "fs_watcher"
watch_dirs = ["{}"]
"#,
            tmp_dir.path().display()
        ));
        let f = write_config(&toml);
        let config = load_config(f.path()).expect("load_config");
        match &config.embed_provider {
            EmbedProviderConfig::Tei {
                query_instruction, ..
            } => {
                assert!(
                    query_instruction.is_some(),
                    "default query_instruction should be Some"
                );
                assert!(
                    query_instruction
                        .as_deref()
                        .unwrap()
                        .contains("web search query"),
                    "default instruction should mention 'web search query'"
                );
            }
            _ => panic!("expected Tei embed provider"),
        }
    }
}
