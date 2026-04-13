// Pipeline — filesystem watcher → chunk → embed → upsert
//
// Architecture:
//   notify-debouncer-mini (sync mpsc) → spawn_blocking bridge → tokio mpsc IndexJob queue
//   → embed_concurrency worker tasks → process_job()
//
// Key design decisions:
//   - Carry tracing::Span in IndexJob; tokio::spawn would drop it otherwise.
//   - Per-URL mutex (DashMap<String, Arc<Mutex<()>>>) prevents concurrent delete+upsert races.
//   - Workers bounded to embed_concurrency provide natural backpressure without a separate semaphore.
//   - notify-debouncer-mini 0.4.x uses a callback/sender API, not a receiver() method.
//     We use std::sync::mpsc::Sender<DebounceEventResult> as the handler and bridge via spawn_blocking.

use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use noxa_core::types::ExtractionResult;
use tokenizers::Tokenizer;

use crate::chunker;
use crate::config::{RagConfig, SourceConfig};
use crate::embed::DynEmbedProvider;
use crate::error::RagError;
use crate::store::DynVectorStore;
use crate::types::{Point, PointPayload};

// ─── Session counters ─────────────────────────────────────────────────────────

/// Shared session metrics updated by workers and read by the heartbeat/shutdown tasks.
#[derive(Default)]
struct SessionCounters {
    files_indexed: AtomicUsize,
    files_failed: AtomicUsize,
    total_chunks: AtomicUsize,
    total_embed_ms: AtomicU64,
    total_upsert_ms: AtomicU64,
}

// ─── IndexJob ────────────────────────────────────────────────────────────────

/// A unit of work: index the .json file at `path`.
/// The tracing `span` is carried explicitly because tokio::spawn does NOT
/// automatically propagate the current span into the new task.
struct IndexJob {
    path: PathBuf,
    span: tracing::Span,
}

// ─── Pipeline ────────────────────────────────────────────────────────────────

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
        // Extract watch config.
        let (watch_dir, debounce_ms) = match &self.config.source {
            SourceConfig::FsWatcher {
                watch_dir,
                debounce_ms,
            } => (watch_dir.clone(), *debounce_ms),
        };

        if self.config.pipeline.embed_concurrency == 0 {
            return Err(RagError::Config(
                "pipeline.embed_concurrency must be > 0 or no workers will run".to_string(),
            ));
        }

        tracing::info!(
            watch_dir = %watch_dir.display(),
            debounce_ms,
            embed_concurrency = self.config.pipeline.embed_concurrency,
            "pipeline starting"
        );

        // Bounded job queue: backpressure at 256 queued jobs.
        let (tx, rx) = tokio::sync::mpsc::channel::<IndexJob>(256);

        // Spawn worker pool — each worker owns a cloned rx.
        // We share a single receiver via Arc<Mutex<Receiver>> so all workers
        // compete fairly for jobs.
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let mut worker_handles = Vec::with_capacity(self.config.pipeline.embed_concurrency);

        for worker_id in 0..self.config.pipeline.embed_concurrency {
            let rx = rx.clone();
            let embed = self.embed.clone();
            let store = self.store.clone();
            let tokenizer = self.tokenizer.clone();
            let config = self.config.clone();
            let url_locks = self.url_locks.clone();
            let counters = self.counters.clone();

            let handle = tokio::spawn(async move {
                tracing::debug!(worker_id, "index worker started");
                loop {
                    let job = {
                        let mut guard = rx.lock().await;
                        guard.recv().await
                    };
                    match job {
                        Some(job) => {
                            let span = job.span.clone();
                            async {
                                match process_job(
                                    job, &embed, &store, &tokenizer, &config, &url_locks,
                                )
                                .await
                                {
                                    Ok(stats) => {
                                        counters.files_indexed.fetch_add(1, Ordering::Relaxed);
                                        counters
                                            .total_chunks
                                            .fetch_add(stats.chunks, Ordering::Relaxed);
                                        counters
                                            .total_embed_ms
                                            .fetch_add(stats.embed_ms, Ordering::Relaxed);
                                        counters
                                            .total_upsert_ms
                                            .fetch_add(stats.upsert_ms, Ordering::Relaxed);
                                    }
                                    Err(e) => {
                                        tracing::error!(error = %e, "index job failed");
                                        counters.files_failed.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                            .instrument(span)
                            .await;
                        }
                        None => {
                            // Sender dropped — workers drain and exit.
                            tracing::debug!(worker_id, "index worker shutting down");
                            break;
                        }
                    }
                }
            });

            worker_handles.push(handle);
        }

        // Build notify debouncer with a *bounded* sync channel as the event handler.
        // notify-debouncer-mini 0.4.x implements DebounceEventHandler for
        // std::sync::mpsc::Sender (unbounded) but not SyncSender, so we wrap
        // SyncSender in a small newtype.  When the bridge is blocked on
        // blocking_send (Tokio queue full) the sync_channel fills and the
        // debouncer's send() call blocks too — closing the backpressure loop.
        struct BoundedSender(std::sync::mpsc::SyncSender<DebounceEventResult>);
        impl notify_debouncer_mini::DebounceEventHandler for BoundedSender {
            fn handle_event(&mut self, event: DebounceEventResult) {
                // Blocks when the channel is full, propagating backpressure.
                let _ = self.0.send(event);
            }
        }

        let (notify_tx, notify_rx) = std::sync::mpsc::sync_channel::<DebounceEventResult>(256);

        let mut debouncer =
            new_debouncer(Duration::from_millis(debounce_ms), BoundedSender(notify_tx))
                .map_err(|e| RagError::Generic(format!("failed to create fs watcher: {e}")))?;

        debouncer
            .watcher()
            .watch(&watch_dir, RecursiveMode::Recursive)
            .map_err(|e| {
                RagError::Generic(format!(
                    "failed to watch directory {}: {e}",
                    watch_dir.display()
                ))
            })?;

        tracing::info!(path = %watch_dir.display(), "watching directory recursively");

        // Bridge: wrap the blocking notify_rx.recv() in spawn_blocking so it
        // doesn't block the tokio reactor.  Send jobs to the tokio job queue.
        let shutdown_clone = self.shutdown.clone();
        let tx_clone = tx.clone();

        let bridge_handle = tokio::task::spawn_blocking(move || {
            // Keep `debouncer` alive for the duration of this thread.
            let _debouncer = debouncer;

            loop {
                // recv_timeout lets us periodically check whether we should stop.
                // We check every 250 ms regardless of debounce setting.
                match notify_rx.recv_timeout(Duration::from_millis(250)) {
                    Ok(Ok(events)) => {
                        if shutdown_clone.is_cancelled() {
                            break;
                        }
                        for event in events {
                            for path in collect_indexable_paths(&event.path) {
                                let span = tracing::info_span!(
                                    "index_job",
                                    path = %path.display(),
                                );
                                let job = IndexJob { path, span };
                                // Retry with a short sleep so shutdown can interrupt a full queue.
                                let mut pending_job = job;
                                let mut saturated_logged = false;
                                loop {
                                    match tx_clone.try_send(pending_job) {
                                        Ok(()) => break,
                                        Err(tokio::sync::mpsc::error::TrySendError::Full(job)) => {
                                            if shutdown_clone.is_cancelled() {
                                                break;
                                            }
                                            if !saturated_logged {
                                                tracing::warn!(
                                                    "job queue saturated (256/256), \
                                                     backing off — embed/upsert catching up"
                                                );
                                                saturated_logged = true;
                                            }
                                            pending_job = job;
                                            std::thread::sleep(Duration::from_millis(10));
                                        }
                                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                            // Receiver dropped — workers are done; exit.
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(error = ?e, "fs watcher error");
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Check if we should stop.
                        if shutdown_clone.is_cancelled() {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }

            tracing::info!("fs watcher bridge exiting");
        });

        // Heartbeat: log pipeline health every 60s.
        let heartbeat_counters = self.counters.clone();
        let heartbeat_shutdown = self.shutdown.clone();
        let session_start = Instant::now();
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            interval.tick().await; // consume immediate first tick
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let uptime_m = session_start.elapsed().as_secs() / 60;
                        tracing::info!(
                            indexed = heartbeat_counters.files_indexed.load(Ordering::Relaxed),
                            failed = heartbeat_counters.files_failed.load(Ordering::Relaxed),
                            uptime_m,
                            "pipeline alive"
                        );
                    }
                    _ = heartbeat_shutdown.cancelled() => break,
                }
            }
        });

        // Wait for cancellation signal.
        self.shutdown.cancelled().await;
        tracing::info!("shutdown signal received, draining pipeline");

        // Drop tx so workers drain their queues and exit.
        drop(tx);

        // Wait for bridge and heartbeat to finish.
        let _ = bridge_handle.await;
        let _ = heartbeat_handle.await;

        // Wait for all workers to drain — 10s hard limit to prevent a stuck
        // job from blocking indefinite shutdown.
        let drain = async {
            for handle in worker_handles {
                let _ = handle.await;
            }
        };
        match tokio::time::timeout(Duration::from_secs(10), drain).await {
            Ok(_) => tracing::info!("pipeline shut down cleanly"),
            Err(_) => {
                tracing::warn!("workers did not drain within 10s, forcing exit");
                return Err(RagError::Generic(
                    "workers did not drain within 10s".to_string(),
                ));
            }
        }

        // Shutdown session summary.
        let indexed = self.counters.files_indexed.load(Ordering::Relaxed);
        let failed = self.counters.files_failed.load(Ordering::Relaxed);
        let chunks = self.counters.total_chunks.load(Ordering::Relaxed);
        let embed_ms = self.counters.total_embed_ms.load(Ordering::Relaxed);
        let upsert_ms = self.counters.total_upsert_ms.load(Ordering::Relaxed);
        let avg_embed_ms = if indexed > 0 { embed_ms / indexed as u64 } else { 0 };
        let avg_upsert_ms = if indexed > 0 { upsert_ms / indexed as u64 } else { 0 };
        tracing::info!(
            indexed,
            failed,
            chunks,
            avg_embed_ms,
            avg_upsert_ms,
            duration_s = session_start.elapsed().as_secs(),
            "session complete"
        );

        Ok(())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns true iff the path has a `.json` extension AND exists on disk.
/// We check both because rename events (vim/emacs atomic saves) may fire for
/// temp files that are gone by the time we process them.
fn is_indexable(path: &Path) -> bool {
    path.extension().map(|e| e == "json").unwrap_or(false) && path.exists()
}

fn collect_indexable_paths(path: &Path) -> Vec<PathBuf> {
    if is_indexable(path) {
        return vec![path.to_path_buf()];
    }

    if !path.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    collect_indexable_paths_recursive(path, &mut found);
    found.sort();
    found
}

fn collect_indexable_paths_recursive(path: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if is_indexable(&entry_path) {
            found.push(entry_path);
        } else if entry_path.is_dir() {
            collect_indexable_paths_recursive(&entry_path, found);
        }
    }
}

/// Returns true iff `host` resolves to a private/loopback/link-local address.
fn is_private_ip(host: &str) -> bool {
    if let Ok(addr) = host.parse::<IpAddr>() {
        return match addr {
            IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
            IpAddr::V6(ip) => {
                ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
            }
        };
    }
    false
}

/// Validate that `url` uses http or https and does not point to a private IP.
fn validate_url_scheme(url: &str) -> Result<(), RagError> {
    if url.is_empty() {
        return Err(RagError::Generic(
            "extraction result has no URL".to_string(),
        ));
    }
    let parsed =
        url::Url::parse(url).map_err(|e| RagError::Generic(format!("invalid URL {url:?}: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(RagError::Generic(format!(
                "URL scheme {other:?} is not allowed (only http/https)"
            )));
        }
    }

    if let Some(host) = parsed.host_str() {
        if is_private_ip(host) {
            return Err(RagError::Generic(format!(
                "URL {url:?} uses a private/loopback IP literal as its host — indexing blocked"
            )));
        }
        // Also block bare "localhost" hostname.
        if host.eq_ignore_ascii_case("localhost") {
            return Err(RagError::Generic(
                "URL points to localhost — indexing blocked".to_string(),
            ));
        }
    }

    Ok(())
}

/// Append a failed-job record to the configured log file (NDJSON format).
/// Silently ignores if no log path is configured.
async fn append_failed_job(path: &Path, error: &impl std::fmt::Display, config: &RagConfig) {
    let Some(ref log_path) = config.pipeline.failed_jobs_log else {
        return;
    };
    let entry = serde_json::json!({
        "path": path.to_string_lossy(),
        "error": error.to_string(),
        "ts": chrono::Utc::now().to_rfc3339(),
    });
    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await
    {
        use tokio::io::AsyncWriteExt;
        let _ = file.write_all(format!("{}\n", entry).as_bytes()).await;
    }
}

// ─── Core processing ─────────────────────────────────────────────────────────

/// Per-job timing and volume stats reported back to the worker loop.
struct JobStats {
    chunks: usize,
    embed_ms: u64,
    upsert_ms: u64,
}

async fn process_job(
    job: IndexJob,
    embed: &DynEmbedProvider,
    store: &DynVectorStore,
    tokenizer: &Arc<Tokenizer>,
    config: &RagConfig,
    url_locks: &Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
) -> Result<JobStats, RagError> {
    let job_start = Instant::now();

    // ── 1. Open file and check size from the same FD (TOCTOU fix) ────────────
    let t0 = Instant::now();
    let mut file = tokio::fs::File::open(&job.path).await?;
    let size = file.metadata().await?.len();

    const MAX_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024; // 50 MiB
    if size > MAX_FILE_SIZE_BYTES {
        tracing::warn!(
            path = ?job.path,
            size,
            "file too large (>50MB), skipping"
        );
        return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
    }

    let mut content = String::with_capacity(size as usize);
    file.read_to_string(&mut content).await?;
    let parse_ms = t0.elapsed().as_millis() as u64;

    // ── 2. Parse JSON ─────────────────────────────────────────────────────────
    let result: ExtractionResult = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(path = ?job.path, error = %e, "json parse failed, skipping");
            append_failed_job(&job.path, &e, config).await;
            return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
        }
    };

    // ── 3. URL validation ─────────────────────────────────────────────────────
    let raw_url = result.metadata.url.as_deref().unwrap_or("").to_string();
    if let Err(e) = validate_url_scheme(&raw_url) {
        tracing::warn!(path = ?job.path, error = %e, "url validation failed, skipping");
        return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
    }
    // Normalize so the mutex key and stored payload match what delete_by_url queries.
    let url = crate::store::qdrant::normalize_url(&raw_url);

    // ── 4. Chunk ──────────────────────────────────────────────────────────────
    let t1 = Instant::now();
    let chunks = chunker::chunk(&result, &config.chunker, tokenizer);
    if chunks.is_empty() {
        tracing::info!(url = %url, "no indexable content after chunking");
        return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
    }
    let chunk_ms = t1.elapsed().as_millis() as u64;

    // ── 5. Embed ──────────────────────────────────────────────────────────────
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let total_tokens: u64 = chunks.iter().map(|c| c.token_estimate as u64).sum();
    let t2 = Instant::now();
    let vectors = embed.embed(&texts).await?;
    let embed_ms = t2.elapsed().as_millis() as u64;
    let embed_tokens_per_sec = if embed_ms > 0 {
        total_tokens * 1_000 / embed_ms
    } else {
        0
    };

    if vectors.len() != chunks.len() {
        return Err(RagError::Embed {
            message: format!(
                "embed returned {} vectors for {} chunks",
                vectors.len(),
                chunks.len()
            ),
            status: None,
        });
    }

    // ── 6. Build points with deterministic UUID v5 ────────────────────────────
    // Use the normalized URL for both the UUID seed and payload.url so that
    // delete_by_url (which also normalizes) matches the stored value for any
    // equivalent URL form (trailing slash, fragment, etc.).
    let n_chunks = chunks.len();
    let points: Vec<Point> = chunks
        .iter()
        .zip(vectors.iter())
        .enumerate()
        .map(|(i, (chunk, vector))| {
            let id = uuid::Uuid::new_v5(
                &config.uuid_namespace,
                format!("{}#chunk{}", url, i).as_bytes(),
            );
            Point {
                id,
                vector: vector.clone(),
                payload: PointPayload {
                    text: chunk.text.clone(),
                    url: url.clone(),
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
                    // IngestionContext provenance fields — populated in Wave 3 by MCP sources.
                    external_id: None,
                    platform_url: None,
                    seed_url: None,
                    search_query: None,
                    crawl_depth: None,
                },
            }
        })
        .collect();

    // ── 7. Per-URL mutex: delete-then-upsert under lock ───────────────────────
    let url_lock = url_locks
        .entry(url.clone())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _guard = url_lock.lock().await;

    // SAFETY NOTE: delete-before-upsert is destructive: if upsert fails after
    // delete, the document is temporarily unindexed until the next file event
    // triggers a re-index. This is acceptable given the current store API lacks
    // a transactional delete-by-url-excluding-ids. UUIDs are deterministic (v5),
    // so re-indexing is always idempotent.
    //
    // Capture the result instead of returning immediately so we can always run
    // the eviction logic below, even on error paths.
    let t3 = Instant::now();
    let store_result: Result<u64, RagError> = async {
        let stale = store.delete_by_url(&url).await?;
        let delete_ms = t3.elapsed().as_millis() as u64;

        let t4 = Instant::now();
        let upserted = store.upsert(points).await.map_err(|e| {
            tracing::error!(
                url = %url,
                error = %e,
                "upsert failed after delete — document temporarily unindexed until next file event"
            );
            e
        })?;
        let upsert_ms = t4.elapsed().as_millis() as u64;

        if stale > 0 {
            tracing::info!(
                url = %url,
                format = "json",
                chunks = upserted,
                stale_deleted = stale,
                embed_tokens = total_tokens,
                embed_tokens_per_sec,
                parse_ms,
                chunk_ms,
                embed_ms,
                delete_ms,
                upsert_ms,
                total_ms = job_start.elapsed().as_millis() as u64,
                "reindexed"
            );
        } else {
            tracing::info!(
                url = %url,
                format = "json",
                chunks = upserted,
                embed_tokens = total_tokens,
                embed_tokens_per_sec,
                parse_ms,
                chunk_ms,
                embed_ms,
                delete_ms,
                upsert_ms,
                total_ms = job_start.elapsed().as_millis() as u64,
                "indexed"
            );
        }

        Ok(upsert_ms)
    }
    .await;

    // Always evict the lock entry — including on error paths — to prevent
    // unbounded DashMap growth during store outages.
    drop(_guard);
    // Drop the local Arc clone before eviction check so strong_count reaches 1.
    drop(url_lock);
    url_locks.remove_if(&url, |_, v| Arc::strong_count(v) == 1);

    let upsert_ms = store_result?;

    Ok(JobStats { chunks: n_chunks, embed_ms, upsert_ms })
}

#[cfg(test)]
mod tests {
    use super::collect_indexable_paths;
    use std::fs;

    #[test]
    fn collect_indexable_paths_finds_nested_json_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let nested = root.join("docs/get-started");
        fs::create_dir_all(&nested).expect("create nested dirs");
        fs::write(root.join("top.json"), "{}").expect("write top-level json");
        fs::write(nested.join("guide.json"), "{}").expect("write nested json");
        fs::write(nested.join("ignore.txt"), "nope").expect("write non-json");

        let paths = collect_indexable_paths(root);
        let rendered: Vec<String> = paths
            .into_iter()
            .map(|p| p.strip_prefix(root).unwrap().display().to_string())
            .collect();

        assert_eq!(rendered, vec!["docs/get-started/guide.json", "top.json"]);
    }
}
