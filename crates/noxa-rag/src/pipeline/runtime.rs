use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};
use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tracing::Instrument;

use crate::config::SourceConfig;
use crate::error::RagError;
use crate::store::{DynVectorStore, HashExistsResult};

use super::process;
use super::scan;
use super::{IndexJob, Pipeline};

/// Maximum concurrent Qdrant existence probes during startup delta scan.
const STARTUP_SCAN_CONCURRENCY: usize = 16;

// ---------------------------------------------------------------------------
// BoundedSender — sync bridge for notify_debouncer_mini events into a bounded
// std::sync::mpsc channel.  Must NOT be pub.
// ---------------------------------------------------------------------------

struct BoundedSender(std::sync::mpsc::SyncSender<DebounceEventResult>);

impl notify_debouncer_mini::DebounceEventHandler for BoundedSender {
    fn handle_event(&mut self, event: DebounceEventResult) {
        let _ = self.0.send(event);
    }
}

// ---------------------------------------------------------------------------
// Component: worker pool
// ---------------------------------------------------------------------------

fn spawn_workers(
    pipeline: &Pipeline,
    rx: Arc<Mutex<mpsc::Receiver<IndexJob>>>,
    watch_roots: Arc<Vec<PathBuf>>,
) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::with_capacity(pipeline.config.pipeline.embed_concurrency);

    for worker_id in 0..pipeline.config.pipeline.embed_concurrency {
        let rx = rx.clone();
        let embed = pipeline.embed.clone();
        let store = pipeline.store.clone();
        let tokenizer = pipeline.tokenizer.clone();
        let config = pipeline.config.clone();
        let url_locks = pipeline.url_locks.clone();
        let counters = pipeline.counters.clone();
        let failed_jobs_log_lock = pipeline.failed_jobs_log_lock.clone();
        let watch_roots = watch_roots.clone();

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
                            match process::process_job(
                                job,
                                &embed,
                                &store,
                                &tokenizer,
                                &config,
                                &url_locks,
                                &watch_roots,
                                &counters,
                                &failed_jobs_log_lock,
                            )
                            .await
                            {
                                Ok(stats) => {
                                    if stats.chunks > 0 {
                                        counters
                                            .files_indexed
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    counters.total_chunks.fetch_add(
                                        stats.chunks,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    counters.total_embed_ms.fetch_add(
                                        stats.embed_ms,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    counters.total_upsert_ms.fetch_add(
                                        stats.upsert_ms,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "index job failed");
                                    counters
                                        .files_failed
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                        }
                        .instrument(span)
                        .await;
                    }
                    None => {
                        tracing::debug!(worker_id, "index worker shutting down");
                        break;
                    }
                }
            }
        });

        handles.push(handle);
    }

    handles
}

// ---------------------------------------------------------------------------
// Component: fs watcher + blocking bridge
// ---------------------------------------------------------------------------

/// Creates the fs debouncer, registers the watch directory, and spawns the
/// blocking bridge task that forwards events into `tx`.  Returns the bridge
/// `JoinHandle` so the caller can await it during shutdown.
///
/// The debouncer is moved into the `spawn_blocking` closure so it stays alive
/// for the entire lifetime of the bridge task.
fn setup_watcher(
    watch_dirs: &[PathBuf],
    debounce_ms: u64,
    tx: mpsc::Sender<IndexJob>,
    shutdown: tokio_util::sync::CancellationToken,
) -> Result<JoinHandle<()>, RagError> {
    let (notify_tx, notify_rx) = std::sync::mpsc::sync_channel::<DebounceEventResult>(256);

    let mut debouncer = new_debouncer(Duration::from_millis(debounce_ms), BoundedSender(notify_tx))
        .map_err(|e| RagError::Generic(format!("failed to create fs watcher: {e}")))?;

    for watch_dir in watch_dirs {
        debouncer
            .watcher()
            .watch(watch_dir, RecursiveMode::Recursive)
            .map_err(|e| {
                RagError::Generic(format!(
                    "failed to watch directory {}: {e}",
                    watch_dir.display()
                ))
            })?;
        tracing::info!(path = %watch_dir.display(), "watching directory recursively");
    }

    let bridge_handle = tokio::task::spawn_blocking(move || {
        // Keep debouncer alive for the duration of the bridge.
        let _debouncer = debouncer;

        loop {
            match notify_rx.recv_timeout(Duration::from_millis(250)) {
                Ok(Ok(events)) => {
                    if shutdown.is_cancelled() {
                        break;
                    }
                    for event in events {
                        for path in scan::collect_indexable_paths(&event.path) {
                            let span = tracing::info_span!("index_job", path = %path.display());
                            let job = IndexJob { path, span };
                            let mut pending_job = job;
                            let mut saturated_logged = false;
                            loop {
                                match tx.try_send(pending_job) {
                                    Ok(()) => break,
                                    Err(tokio::sync::mpsc::error::TrySendError::Full(job)) => {
                                        if shutdown.is_cancelled() {
                                            break;
                                        }
                                        if !saturated_logged {
                                            tracing::warn!(
                                                "job queue saturated (256/256), backing off — embed/upsert catching up"
                                            );
                                            saturated_logged = true;
                                        }
                                        pending_job = job;
                                        std::thread::sleep(Duration::from_millis(10));
                                    }
                                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
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
                    if shutdown.is_cancelled() {
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

    Ok(bridge_handle)
}

// ---------------------------------------------------------------------------
// Component: startup delta scan
// ---------------------------------------------------------------------------

fn spawn_startup_scan(
    tx: mpsc::Sender<IndexJob>,
    store: DynVectorStore,
    shutdown: tokio_util::sync::CancellationToken,
    watch_dirs: Vec<PathBuf>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let paths = match tokio::task::spawn_blocking({
            let dirs = watch_dirs.clone();
            move || {
                let mut all: Vec<PathBuf> = dirs
                    .iter()
                    .flat_map(|d| scan::collect_indexable_paths(d))
                    .collect();
                all.sort();
                all.dedup();
                all
            }
        })
        .await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, "startup scan: collect_indexable_paths panicked");
                return;
            }
        };

        let total = paths.len();
        tracing::info!(count = total, "startup scan: checking files for delta");

        let queued = Arc::new(AtomicUsize::new(0));
        let skipped = Arc::new(AtomicUsize::new(0));
        let backend_errors = Arc::new(AtomicUsize::new(0));

        stream::iter(paths)
            .for_each_concurrent(STARTUP_SCAN_CONCURRENCY, |path| {
                let tx = tx.clone();
                let store = store.clone();
                let shutdown = shutdown.clone();
                let queued = Arc::clone(&queued);
                let skipped = Arc::clone(&skipped);
                let backend_errors = Arc::clone(&backend_errors);

                async move {
                    if shutdown.is_cancelled() {
                        return;
                    }

                    let path2 = path.clone();
                    let hash_and_url =
                        tokio::task::spawn_blocking(move || scan::startup_scan_key(&path2))
                            .await
                            .ok()
                            .flatten();

                    let (hash, url) = match hash_and_url {
                        Some(t) => t,
                        None => {
                            tracing::debug!(
                                path = %path.display(),
                                "startup scan: no url/hash, queuing"
                            );
                            let span =
                                tracing::info_span!("index_job", path = %path.display());
                            tokio::select! {
                                _ = tx.send(IndexJob { path, span }) => {}
                                _ = shutdown.cancelled() => {}
                            }
                            queued.fetch_add(1, Ordering::Relaxed);
                            return;
                        }
                    };

                    match store.url_with_hash_exists_checked(&url, &hash).await {
                        HashExistsResult::Exists => {
                            skipped.fetch_add(1, Ordering::Relaxed);
                            tracing::debug!(
                                path = %path.display(),
                                url = %url,
                                "startup scan: already indexed, skipping"
                            );
                        }
                        HashExistsResult::NotIndexed => {
                            let span =
                                tracing::info_span!("index_job", path = %path.display());
                            tokio::select! {
                                _ = tx.send(IndexJob { path, span }) => {}
                                _ = shutdown.cancelled() => {}
                            }
                            queued.fetch_add(1, Ordering::Relaxed);
                        }
                        HashExistsResult::BackendError(ref msg) => {
                            // Do NOT re-queue on backend failure — a degraded Qdrant endpoint
                            // must not trigger a full reindex storm.  The file will be
                            // re-evaluated on next startup once the backend recovers.
                            backend_errors.fetch_add(1, Ordering::Relaxed);
                            tracing::warn!(
                                path = %path.display(),
                                url = %url,
                                error = %msg,
                                "startup scan: backend error during delta check — skipping requeue to avoid reindex storm"
                            );
                        }
                    }
                }
            })
            .await;

        let queued = queued.load(Ordering::Relaxed);
        let skipped = skipped.load(Ordering::Relaxed);
        let backend_errors = backend_errors.load(Ordering::Relaxed);
        if backend_errors > 0 {
            tracing::warn!(
                total,
                queued,
                skipped,
                backend_errors,
                "startup scan complete — some files skipped due to backend errors; they will be re-evaluated on next startup"
            );
        } else {
            tracing::info!(total, queued, skipped, "startup scan complete");
        }
    })
}

// ---------------------------------------------------------------------------
// Component: heartbeat / metrics ticker
// ---------------------------------------------------------------------------

fn spawn_heartbeat(
    counters: Arc<super::SessionCounters>,
    shutdown: tokio_util::sync::CancellationToken,
    session_start: Instant,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let uptime_m = session_start.elapsed().as_secs() / 60;
                    tracing::info!(
                        indexed       = counters.files_indexed.load(std::sync::atomic::Ordering::Relaxed),
                        failed        = counters.files_failed.load(std::sync::atomic::Ordering::Relaxed),
                        parse_failures = counters.parse_failures.load(std::sync::atomic::Ordering::Relaxed),
                        uptime_m,
                        "pipeline alive"
                    );
                }
                _ = shutdown.cancelled() => break,
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Component: shutdown drain + final metrics
// ---------------------------------------------------------------------------

async fn drain_and_report(
    worker_handles: Vec<JoinHandle<()>>,
    pipeline: &Pipeline,
    session_start: Instant,
) -> Result<(), RagError> {
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

    let indexed = pipeline
        .counters
        .files_indexed
        .load(std::sync::atomic::Ordering::Relaxed);
    let failed = pipeline
        .counters
        .files_failed
        .load(std::sync::atomic::Ordering::Relaxed);
    let parse_failures = pipeline
        .counters
        .parse_failures
        .load(std::sync::atomic::Ordering::Relaxed);
    let chunks = pipeline
        .counters
        .total_chunks
        .load(std::sync::atomic::Ordering::Relaxed);
    let embed_ms = pipeline
        .counters
        .total_embed_ms
        .load(std::sync::atomic::Ordering::Relaxed);
    let upsert_ms = pipeline
        .counters
        .total_upsert_ms
        .load(std::sync::atomic::Ordering::Relaxed);
    let avg_embed_ms = if indexed > 0 {
        embed_ms / indexed as u64
    } else {
        0
    };
    let avg_upsert_ms = if indexed > 0 {
        upsert_ms / indexed as u64
    } else {
        0
    };
    tracing::info!(
        indexed,
        failed,
        parse_failures,
        chunks,
        avg_embed_ms,
        avg_upsert_ms,
        duration_s = session_start.elapsed().as_secs(),
        "session complete"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry point — thin composer
// ---------------------------------------------------------------------------

pub(crate) async fn run(pipeline: &Pipeline) -> Result<(), RagError> {
    // --- validate config & extract source params ---
    let (watch_dirs, debounce_ms) = match &pipeline.config.source {
        SourceConfig::FsWatcher {
            watch_dirs,
            debounce_ms,
            ..
        } => (watch_dirs.clone(), *debounce_ms),
    };

    if pipeline.config.pipeline.embed_concurrency == 0 {
        return Err(RagError::Config(
            "pipeline.embed_concurrency must be > 0 or no workers will run".to_string(),
        ));
    }

    tracing::info!(
        watch_dirs = ?watch_dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>(),
        debounce_ms,
        embed_concurrency = pipeline.config.pipeline.embed_concurrency,
        "pipeline starting"
    );

    let session_start = Instant::now();
    let watch_roots = Arc::new(scan::canonical_watch_roots(&watch_dirs).await?);

    // --- channel ---
    let (tx, rx) = mpsc::channel::<IndexJob>(256);
    let rx = Arc::new(Mutex::new(rx));

    // --- spawn workers ---
    let worker_handles = spawn_workers(pipeline, rx, watch_roots);

    // --- setup fs watcher + blocking bridge ---
    let bridge_handle = setup_watcher(
        &watch_dirs,
        debounce_ms,
        tx.clone(),
        pipeline.shutdown.clone(),
    )?;

    // --- startup delta scan ---
    let startup_handle = spawn_startup_scan(
        tx.clone(),
        pipeline.store.clone(),
        pipeline.shutdown.clone(),
        watch_dirs,
    );

    // --- heartbeat ---
    let heartbeat_handle = spawn_heartbeat(
        pipeline.counters.clone(),
        pipeline.shutdown.clone(),
        session_start,
    );

    // --- wait for shutdown signal ---
    pipeline.shutdown.cancelled().await;
    tracing::info!("shutdown signal received, draining pipeline");

    // Drop tx BEFORE awaiting workers so their recv loops see channel closed.
    drop(tx);

    let _ = bridge_handle.await;
    let _ = heartbeat_handle.await;
    let _ = startup_handle.await;

    // --- drain workers + log final metrics ---
    drain_and_report(worker_handles, pipeline, session_start).await
}

// ---------------------------------------------------------------------------
// Tests: startup delta scan routing
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use tempfile::tempdir;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use crate::error::RagError;
    use crate::store::{DynVectorStore, HashExistsResult, VectorStore};
    use crate::types::{Point, SearchMetadataFilter, SearchResult};

    use super::spawn_startup_scan;

    // ── Mock VectorStore ──────────────────────────────────────────────────────

    /// A mock VectorStore that returns a fixed `HashExistsResult` for URLs it
    /// knows about (keyed by URL string) and `NotIndexed` for unknown URLs.
    /// Counts the number of times `url_with_hash_exists_checked` is called.
    struct MockStore {
        results: HashMap<String, HashExistsResult>,
        call_count: Arc<AtomicUsize>,
    }

    impl MockStore {
        fn new(results: HashMap<String, HashExistsResult>) -> Arc<Self> {
            Arc::new(Self {
                results,
                call_count: Arc::new(AtomicUsize::new(0)),
            })
        }
    }

    #[async_trait]
    impl VectorStore for MockStore {
        async fn upsert(&self, _points: Vec<Point>) -> Result<usize, RagError> {
            unimplemented!("MockStore::upsert not needed for startup scan tests")
        }

        async fn delete_by_url(&self, _url: &str) -> Result<(), RagError> {
            unimplemented!("MockStore::delete_by_url not needed for startup scan tests")
        }

        async fn delete_stale_by_url(
            &self,
            _url: &str,
            _keep_ids: &[uuid::Uuid],
        ) -> Result<(), RagError> {
            unimplemented!("MockStore::delete_stale_by_url not needed for startup scan tests")
        }

        async fn search(
            &self,
            _vector: &[f32],
            _limit: usize,
            _filter: Option<&SearchMetadataFilter>,
        ) -> Result<Vec<SearchResult>, RagError> {
            unimplemented!("MockStore::search not needed for startup scan tests")
        }

        async fn collection_point_count(&self) -> Result<u64, RagError> {
            unimplemented!("MockStore::collection_point_count not needed for startup scan tests")
        }

        async fn url_with_hash_exists_checked(&self, url: &str, _hash: &str) -> HashExistsResult {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            self.results
                .get(url)
                .cloned()
                .unwrap_or(HashExistsResult::NotIndexed)
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    // ── Helper: compute file:// URL for a real file path ─────────────────────

    fn file_url(path: &std::path::Path) -> String {
        url::Url::from_file_path(path)
            .expect("from_file_path")
            .to_string()
    }

    // ── Helper: run startup scan and drain all queued jobs ────────────────────

    async fn run_scan_and_collect(store: DynVectorStore, watch_dir: PathBuf) -> HashSet<PathBuf> {
        let shutdown = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel::<super::super::IndexJob>(256);

        let handle = spawn_startup_scan(tx.clone(), store, shutdown.clone(), vec![watch_dir]);

        // Wait for the scan to finish before draining.
        handle.await.expect("startup scan panicked");

        // Drop our sender so the channel shows closed to any downstream reader.
        drop(tx);

        let mut queued = HashSet::new();
        while let Ok(job) = rx.try_recv() {
            queued.insert(job.path);
        }
        queued
    }

    // ── Test 1: NotIndexed URLs are queued ────────────────────────────────────

    /// Files whose URL maps to `NotIndexed` must be pushed to the work queue.
    #[tokio::test]
    async fn startup_scan_queues_not_indexed_files() {
        let dir = tempdir().expect("tempdir");

        // Write a real .md file so startup_scan_key can read it.
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# hello").expect("write file");
        let url = file_url(&path);

        let mut results = HashMap::new();
        results.insert(url, HashExistsResult::NotIndexed);

        let store = MockStore::new(results);
        let call_count = Arc::clone(&store.call_count);
        let dyn_store: DynVectorStore = store;

        let queued = run_scan_and_collect(dyn_store, dir.path().to_path_buf()).await;

        assert!(
            queued.contains(&path),
            "NotIndexed file should be queued for indexing"
        );
        assert_eq!(
            call_count.load(Ordering::Relaxed),
            1,
            "url_with_hash_exists_checked should be called once per file"
        );
    }

    // ── Test 2: Exists URLs are NOT queued ───────────────────────────────────

    /// Files whose URL maps to `Exists` must be skipped — already up-to-date.
    #[tokio::test]
    async fn startup_scan_skips_already_indexed_files() {
        let dir = tempdir().expect("tempdir");

        let path = dir.path().join("indexed.md");
        std::fs::write(&path, "# indexed content").expect("write file");
        let url = file_url(&path);

        let mut results = HashMap::new();
        results.insert(url, HashExistsResult::Exists);

        let store = MockStore::new(results);
        let call_count = Arc::clone(&store.call_count);
        let dyn_store: DynVectorStore = store;

        let queued = run_scan_and_collect(dyn_store, dir.path().to_path_buf()).await;

        assert!(!queued.contains(&path), "Exists file should NOT be queued");
        assert_eq!(
            call_count.load(Ordering::Relaxed),
            1,
            "url_with_hash_exists_checked should be called once"
        );
    }

    // ── Test 3: BackendError URLs are NOT queued ─────────────────────────────

    /// Files whose existence check returns `BackendError` must NOT be queued.
    /// This is the key regression guard: a degraded Qdrant must not trigger a
    /// full reindex storm.
    #[tokio::test]
    async fn startup_scan_does_not_requeue_on_backend_error() {
        let dir = tempdir().expect("tempdir");

        let path = dir.path().join("fragile.md");
        std::fs::write(&path, "# content").expect("write file");
        let url = file_url(&path);

        let mut results = HashMap::new();
        results.insert(
            url,
            HashExistsResult::BackendError("connection refused".to_string()),
        );

        let store = MockStore::new(results);
        let call_count = Arc::clone(&store.call_count);
        let dyn_store: DynVectorStore = store;

        let queued = run_scan_and_collect(dyn_store, dir.path().to_path_buf()).await;

        assert!(
            !queued.contains(&path),
            "BackendError file must NOT be queued — avoid reindex storm on degraded Qdrant"
        );
        assert_eq!(
            call_count.load(Ordering::Relaxed),
            1,
            "url_with_hash_exists_checked should still be called (probe must happen)"
        );
    }

    // ── Test 4: Mixed results across multiple files ───────────────────────────

    /// When a scan contains a mix of Exists / NotIndexed / BackendError files,
    /// only the NotIndexed ones are queued.
    #[tokio::test]
    async fn startup_scan_routes_mixed_results_correctly() {
        let dir = tempdir().expect("tempdir");

        let exists_path = dir.path().join("exists.md");
        let not_indexed_path = dir.path().join("new.md");
        let backend_error_path = dir.path().join("degraded.md");

        for path in &[&exists_path, &not_indexed_path, &backend_error_path] {
            std::fs::write(path, "# content").expect("write file");
        }

        let mut results = HashMap::new();
        results.insert(file_url(&exists_path), HashExistsResult::Exists);
        results.insert(file_url(&not_indexed_path), HashExistsResult::NotIndexed);
        results.insert(
            file_url(&backend_error_path),
            HashExistsResult::BackendError("timeout".to_string()),
        );

        let store = MockStore::new(results);
        let call_count = Arc::clone(&store.call_count);
        let dyn_store: DynVectorStore = store;

        let queued = run_scan_and_collect(dyn_store, dir.path().to_path_buf()).await;

        assert!(
            queued.contains(&not_indexed_path),
            "NotIndexed file should be queued"
        );
        assert!(
            !queued.contains(&exists_path),
            "Exists file should NOT be queued"
        );
        assert!(
            !queued.contains(&backend_error_path),
            "BackendError file must NOT be queued"
        );
        assert_eq!(
            call_count.load(Ordering::Relaxed),
            3,
            "should probe all 3 files regardless of result"
        );
    }
}
