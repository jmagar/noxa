use std::path::{Path, PathBuf};
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
    watch_root: Arc<PathBuf>,
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
        let watch_root = watch_root.clone();

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
                                watch_root.as_ref(),
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
    watch_dir: &Path,
    debounce_ms: u64,
    tx: mpsc::Sender<IndexJob>,
    shutdown: tokio_util::sync::CancellationToken,
) -> Result<JoinHandle<()>, RagError> {
    let (notify_tx, notify_rx) = std::sync::mpsc::sync_channel::<DebounceEventResult>(256);

    let mut debouncer =
        new_debouncer(Duration::from_millis(debounce_ms), BoundedSender(notify_tx))
            .map_err(|e| RagError::Generic(format!("failed to create fs watcher: {e}")))?;

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
                            let span =
                                tracing::info_span!("index_job", path = %path.display());
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
    watch_dir: PathBuf,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let paths = match tokio::task::spawn_blocking({
            let dir = watch_dir.clone();
            move || scan::collect_indexable_paths(&dir)
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
                        indexed = counters.files_indexed.load(std::sync::atomic::Ordering::Relaxed),
                        failed  = counters.files_failed.load(std::sync::atomic::Ordering::Relaxed),
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

// ---------------------------------------------------------------------------
// Public entry point — thin composer
// ---------------------------------------------------------------------------

pub(crate) async fn run(pipeline: &Pipeline) -> Result<(), RagError> {
    // --- validate config & extract source params ---
    let (watch_dir, debounce_ms) = match &pipeline.config.source {
        SourceConfig::FsWatcher { watch_dir, debounce_ms } => {
            (watch_dir.clone(), *debounce_ms)
        }
    };

    if pipeline.config.pipeline.embed_concurrency == 0 {
        return Err(RagError::Config(
            "pipeline.embed_concurrency must be > 0 or no workers will run".to_string(),
        ));
    }

    tracing::info!(
        watch_dir = %watch_dir.display(),
        debounce_ms,
        embed_concurrency = pipeline.config.pipeline.embed_concurrency,
        "pipeline starting"
    );

    let session_start = Instant::now();
    let watch_root = Arc::new(scan::canonical_watch_root(&watch_dir).await?);

    // --- channel ---
    let (tx, rx) = mpsc::channel::<IndexJob>(256);
    let rx = Arc::new(Mutex::new(rx));

    // --- spawn workers ---
    let worker_handles = spawn_workers(pipeline, rx, watch_root);

    // --- setup fs watcher + blocking bridge ---
    let bridge_handle = setup_watcher(
        &watch_dir,
        debounce_ms,
        tx.clone(),
        pipeline.shutdown.clone(),
    )?;

    // --- startup delta scan ---
    let startup_handle = spawn_startup_scan(
        tx.clone(),
        pipeline.store.clone(),
        pipeline.shutdown.clone(),
        watch_dir.clone(),
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
