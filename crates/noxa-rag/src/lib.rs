/// noxa-rag — RAG pipeline crate.
///
/// Watches noxa output directory for ExtractionResult JSON files,
/// chunks them, embeds via TEI, and upserts to Qdrant.
///
/// # Crate structure
/// - `embed` — EmbedProvider trait + TeiProvider impl
/// - `store` — VectorStore trait + QdrantStore impl
/// - `chunker` — ExtractionResult → Vec<Chunk>
/// - `config` — RagConfig (TOML deserialization)
/// - `factory` — build_embed_provider / build_vector_store
/// - `pipeline` — filesystem watcher orchestration
/// - `error` — RagError enum
// Tokenizer Sync compile-time assertion.
// tokenizers::Tokenizer must be Sync to be used across tokio workers.
// If this fails to compile, workers cannot safely share the tokenizer.
const _: () = {
    fn _assert_sync<T: Sync>() {}
    fn _check() {
        _assert_sync::<tokenizers::Tokenizer>();
    }
};

pub mod chunker;
pub mod config;
pub mod embed;
pub mod error;
pub mod factory;
pub mod pipeline;
pub mod store;
pub mod types;

// Re-export most-used types at crate root
pub use config::{RagConfig, load_config};
pub use embed::{DynEmbedProvider, EmbedProvider};
pub use error::RagError;
pub use factory::{build_embed_provider, build_vector_store};
pub use store::{DynVectorStore, VectorStore};
pub use types::{Chunk, Point, PointPayload, SearchResult};
