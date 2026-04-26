use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use tokenizers::Tokenizer;
use tokio_util::sync::CancellationToken;

use crate::config::RagConfig;
use crate::embed::DynEmbedProvider;
use crate::error::RagError;
use crate::store::DynVectorStore;

mod heartbeat;
mod parse;
mod process;
mod runtime;
mod scan;
mod startup_scan;
mod watcher;
mod worker;

struct CounterSnapshot {
    indexed: usize,
    failed: usize,
    parse_failures: usize,
    total_chunks: usize,
    total_parse_ms: u64,
    total_chunk_ms: u64,
    total_embed_ms: u64,
    total_upsert_ms: u64,
}

#[derive(Default)]
struct SessionCounters {
    files_indexed: std::sync::atomic::AtomicUsize,
    files_failed: std::sync::atomic::AtomicUsize,
    /// Parse-level failures from `parse_file` errors — tracked separately from
    /// broader process errors so the heartbeat can report them independently.
    parse_failures: std::sync::atomic::AtomicUsize,
    total_chunks: std::sync::atomic::AtomicUsize,
    total_parse_ms: std::sync::atomic::AtomicU64,
    total_chunk_ms: std::sync::atomic::AtomicU64,
    total_embed_ms: std::sync::atomic::AtomicU64,
    total_upsert_ms: std::sync::atomic::AtomicU64,
}

impl SessionCounters {
    fn snapshot(&self) -> CounterSnapshot {
        use std::sync::atomic::Ordering::Relaxed;
        CounterSnapshot {
            indexed: self.files_indexed.load(Relaxed),
            failed: self.files_failed.load(Relaxed),
            parse_failures: self.parse_failures.load(Relaxed),
            total_chunks: self.total_chunks.load(Relaxed),
            total_parse_ms: self.total_parse_ms.load(Relaxed),
            total_chunk_ms: self.total_chunk_ms.load(Relaxed),
            total_embed_ms: self.total_embed_ms.load(Relaxed),
            total_upsert_ms: self.total_upsert_ms.load(Relaxed),
        }
    }

    fn record_success(&self, stats: &JobStats) {
        use std::sync::atomic::Ordering::Relaxed;
        if stats.chunks > 0 {
            self.files_indexed.fetch_add(1, Relaxed);
        }
        self.total_chunks.fetch_add(stats.chunks, Relaxed);
        self.total_parse_ms.fetch_add(stats.parse_ms, Relaxed);
        self.total_chunk_ms.fetch_add(stats.chunk_ms, Relaxed);
        self.total_embed_ms.fetch_add(stats.embed_ms, Relaxed);
        self.total_upsert_ms.fetch_add(stats.upsert_ms, Relaxed);
    }

    fn record_failure(&self) {
        self.files_failed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_parse_failure(&self) {
        self.parse_failures
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
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

#[derive(Default)]
struct JobStats {
    chunks: usize,
    parse_ms: u64,
    chunk_ms: u64,
    embed_ms: u64,
    upsert_ms: u64,
}

/// Shared per-worker context cloned from the Pipeline before each worker task spawns.
///
/// Collapses the 10-parameter `process_job` signature into a single borrow, making
/// call sites readable and making it cheap to add new shared state without touching
/// every call site.
#[derive(Clone)]
struct WorkerContext {
    embed: DynEmbedProvider,
    store: DynVectorStore,
    tokenizer: Arc<Tokenizer>,
    config: Arc<RagConfig>,
    url_locks: Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    git_branch_cache: Arc<DashMap<PathBuf, Option<String>>>,
    watch_roots: Arc<Vec<PathBuf>>,
    counters: Arc<SessionCounters>,
    failed_jobs_log_lock: Arc<tokio::sync::Mutex<()>>,
    shutdown: CancellationToken,
}

impl WorkerContext {
    fn from_pipeline(pipeline: &Pipeline) -> Self {
        Self {
            embed: pipeline.embed.clone(),
            store: pipeline.store.clone(),
            tokenizer: pipeline.tokenizer.clone(),
            config: pipeline.config.clone(),
            url_locks: pipeline.url_locks.clone(),
            git_branch_cache: pipeline.git_branch_cache.clone(),
            watch_roots: pipeline
                .watch_roots
                .get()
                .expect("watch_roots set before spawn_workers")
                .clone(),
            counters: pipeline.counters.clone(),
            failed_jobs_log_lock: pipeline.failed_jobs_log_lock.clone(),
            shutdown: pipeline.shutdown.clone(),
        }
    }
}

pub struct Pipeline {
    config: Arc<RagConfig>,
    embed: DynEmbedProvider,
    store: DynVectorStore,
    tokenizer: Arc<Tokenizer>,
    shutdown: CancellationToken,
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
    /// Canonicalized watch roots, set once during `run()` before workers are spawned.
    /// Workers access this directly instead of receiving it as a spawn parameter.
    watch_roots: std::sync::OnceLock<Arc<Vec<PathBuf>>>,
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
            config: Arc::new(config),
            embed,
            store,
            tokenizer,
            shutdown,
            url_locks: Arc::new(DashMap::new()),
            git_branch_cache: Arc::new(DashMap::new()),
            counters: Arc::new(SessionCounters::default()),
            failed_jobs_log_lock: Arc::new(tokio::sync::Mutex::new(())),
            watch_roots: std::sync::OnceLock::new(),
        }
    }

    /// Run the filesystem watcher pipeline.
    ///
    /// Returns when the CancellationToken is cancelled.
    pub async fn run(&self) -> Result<(), RagError> {
        runtime::run(self).await
    }
}
