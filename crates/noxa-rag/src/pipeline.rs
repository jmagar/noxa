use std::sync::Arc;

use dashmap::DashMap;
use tokenizers::Tokenizer;
use tokio_util::sync::CancellationToken;

use crate::config::RagConfig;
use crate::embed::DynEmbedProvider;
use crate::error::RagError;
use crate::store::DynVectorStore;

mod parse;
mod process;
mod runtime;
mod scan;

#[derive(Default)]
struct SessionCounters {
    files_indexed: std::sync::atomic::AtomicUsize,
    files_failed: std::sync::atomic::AtomicUsize,
    total_chunks: std::sync::atomic::AtomicUsize,
    total_embed_ms: std::sync::atomic::AtomicU64,
    total_upsert_ms: std::sync::atomic::AtomicU64,
}

struct IndexJob {
    path: std::path::PathBuf,
    span: tracing::Span,
}

struct JobStats {
    chunks: usize,
    embed_ms: u64,
    upsert_ms: u64,
}

pub struct Pipeline {
    pub config: RagConfig,
    pub embed: DynEmbedProvider,
    pub store: DynVectorStore,
    pub tokenizer: Arc<Tokenizer>,
    pub shutdown: CancellationToken,
    /// Per-URL mutex: prevents concurrent delete-then-upsert races for the same URL.
    url_locks: Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Session-level metrics shared between workers, heartbeat, and shutdown tasks.
    counters: Arc<SessionCounters>,
}

impl Pipeline {
    pub fn new(
        config: RagConfig,
        embed: DynEmbedProvider,
        store: DynVectorStore,
        tokenizer: Arc<Tokenizer>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            config,
            embed,
            store,
            tokenizer,
            shutdown,
            url_locks: Arc::new(DashMap::new()),
            counters: Arc::new(SessionCounters::default()),
        }
    }

    /// Run the filesystem watcher pipeline.
    ///
    /// Returns when the CancellationToken is cancelled.
    pub async fn run(&self) -> Result<(), RagError> {
        runtime::run(self).await
    }
}
