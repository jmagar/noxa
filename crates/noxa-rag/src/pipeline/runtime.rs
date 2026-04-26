use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;

use crate::config::SourceConfig;
use crate::error::RagError;

use super::Pipeline;
use super::scan;

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
            tracing::warn!(
                timeout_secs,
                "workers did not drain within timeout, forcing exit"
            );
            return Err(RagError::DrainTimeout);
        }
    }

    let snap = pipeline.counters.snapshot();
    let avg = |total: u64| -> u64 {
        if snap.indexed > 0 {
            total / snap.indexed as u64
        } else {
            0
        }
    };
    let avg_embed_ms = avg(snap.total_embed_ms);
    let avg_upsert_ms = avg(snap.total_upsert_ms);
    tracing::info!(
        indexed = snap.indexed,
        failed = snap.failed,
        parse_failures = snap.parse_failures,
        chunks = snap.total_chunks,
        avg_embed_ms,
        avg_upsert_ms,
        duration_s = session_start.elapsed().as_secs(),
        "session complete"
    );

    Ok(())
}

pub(crate) async fn run(pipeline: &Pipeline) -> Result<(), RagError> {
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

    let (tx, rx) = async_channel::bounded(pipeline.config.pipeline.job_queue_capacity);

    let worker_handles = spawn_workers(pipeline, rx, Arc::clone(&watch_roots));

    let bridge_handle = setup_watcher(
        &watch_dirs,
        debounce_ms,
        tx.clone(),
        pipeline.shutdown.clone(),
    )?;

    let startup_handle = spawn_startup_scan(
        tx.clone(),
        pipeline.store.clone(),
        pipeline.shutdown.clone(),
        watch_dirs,
        pipeline.config.pipeline.startup_scan_concurrency,
    );

    let heartbeat_handle = spawn_heartbeat(
        pipeline.counters.clone(),
        pipeline.url_locks.clone(),
        pipeline.shutdown.clone(),
        session_start,
    );

    pipeline.shutdown.cancelled().await;
    tracing::info!("shutdown signal received, draining pipeline");

    // Drop tx BEFORE awaiting workers so their recv loops see channel closed.
    drop(tx);

    let _ = tokio::join!(bridge_handle, heartbeat_handle, startup_handle);

    drain_and_report(worker_handles, pipeline, session_start).await
}
