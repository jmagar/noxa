use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;

use crate::config::SourceConfig;
use crate::error::RagError;

use super::scan;
use super::Pipeline;

use super::heartbeat::spawn_heartbeat;
use super::startup_scan::spawn_startup_scan;
use super::watcher::setup_watcher;
use super::worker::spawn_workers;

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
    let timeout_secs = pipeline.config.pipeline.drain_timeout_secs;
    match tokio::time::timeout(Duration::from_secs(timeout_secs), drain).await {
        Ok(_) => tracing::info!("pipeline shut down cleanly"),
        Err(_) => {
            tracing::warn!(timeout_secs, "workers did not drain within timeout, forcing exit");
            return Err(RagError::DrainTimeout);
        }
    }

    let snap = pipeline.counters.snapshot();
    let avg_embed_ms = if snap.indexed > 0 { snap.total_embed_ms / snap.indexed as u64 } else { 0 };
    let avg_upsert_ms = if snap.indexed > 0 { snap.total_upsert_ms / snap.indexed as u64 } else { 0 };
    tracing::info!(
        indexed        = snap.indexed,
        failed         = snap.failed,
        parse_failures = snap.parse_failures,
        chunks         = snap.total_chunks,
        avg_embed_ms,
        avg_upsert_ms,
        duration_s     = session_start.elapsed().as_secs(),
        "session complete"
    );

    Ok(())
}

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
    pipeline
        .watch_roots
        .set(Arc::new(scan::canonical_watch_roots(&watch_dirs).await?))
        .ok();

    // --- channel (MPMC: every worker gets its own Receiver clone) ---
    let (tx, rx) = async_channel::bounded(pipeline.config.pipeline.job_queue_capacity);

    // --- spawn workers ---
    let worker_handles = spawn_workers(pipeline, rx);

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
        pipeline.config.pipeline.startup_scan_concurrency,
    );

    // --- heartbeat (also sweeps stale url_locks every 60s) ---
    let heartbeat_handle = spawn_heartbeat(
        pipeline.counters.clone(),
        pipeline.url_locks.clone(),
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
