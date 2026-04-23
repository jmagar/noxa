use std::path::PathBuf;
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
    /// Parse-level failures from `parse_file` errors — tracked separately from
    /// broader process errors so the heartbeat can report them independently.
    parse_failures: std::sync::atomic::AtomicUsize,
    total_chunks: std::sync::atomic::AtomicUsize,
    total_embed_ms: std::sync::atomic::AtomicU64,
    total_upsert_ms: std::sync::atomic::AtomicU64,
}

struct IndexJob {
    path: std::path::PathBuf,
    span: tracing::Span,
}

/// A deletion job: remove all Qdrant chunks for the given file URL.
///
/// Used when the fs-watcher detects that a watched file no longer exists on disk.
/// `path` is the raw (non-canonicalized) path reported by the watcher — we cannot
/// call `canonicalize()` on a deleted file.
struct DeleteJob {
    path: std::path::PathBuf,
    span: tracing::Span,
}

/// Discriminated union of index and delete pipeline jobs.
///
/// Both variants share a tracing span so log lines are tied to the originating
/// fs-watcher event regardless of which worker picks the job up.
enum PipelineJob {
    /// Index (or re-index) a file that exists on disk.
    Index(IndexJob),
    /// Delete all Qdrant chunks for a file that was removed from disk.
    Delete(DeleteJob),
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
    /// Cache of git root directory → branch name, shared across workers to avoid
    /// redundant `.git/HEAD` reads. Keyed by the git root so all files in the same
    /// repo share one cache entry per session.
    git_branch_cache: Arc<DashMap<PathBuf, Option<String>>>,
    /// Session-level metrics shared between workers, heartbeat, and shutdown tasks.
    counters: Arc<SessionCounters>,
    /// Serialises failed-jobs log rotation: check-size → rotate → append must be atomic
    /// across concurrent workers to avoid double-rename races.
    failed_jobs_log_lock: Arc<tokio::sync::Mutex<()>>,
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
            git_branch_cache: Arc::new(DashMap::new()),
            counters: Arc::new(SessionCounters::default()),
            failed_jobs_log_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Run the filesystem watcher pipeline.
    ///
    /// Returns when the CancellationToken is cancelled.
    pub async fn run(&self) -> Result<(), RagError> {
        runtime::run(self).await
    }
}
