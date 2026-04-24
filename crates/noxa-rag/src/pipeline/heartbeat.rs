use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::task::JoinHandle;

use super::SessionCounters;

pub(super) fn spawn_heartbeat(
    counters: Arc<SessionCounters>,
    url_locks: Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
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
                    // Periodic sweep: drop per-URL locks nobody is actively
                    // holding. `strong_count == 1` means only the DashMap owns
                    // the Arc; strong_count > 1 means a worker has cloned it
                    // and is (or is about to be) holding the mutex.  This
                    // prevents unbounded growth under file churn because the
                    // inline cleanup after each job only fires if that worker
                    // is the last clone alive.
                    let before = url_locks.len();
                    url_locks.retain(|_, v| Arc::strong_count(v) > 1);
                    let after = url_locks.len();
                    if before != after {
                        tracing::debug!(
                            swept = before - after,
                            remaining = after,
                            "url_locks heartbeat sweep"
                        );
                    }
                    let snap = counters.snapshot();
                    tracing::info!(
                        indexed        = snap.indexed,
                        failed         = snap.failed,
                        parse_failures = snap.parse_failures,
                        parse_ms       = snap.total_parse_ms,
                        chunk_ms       = snap.total_chunk_ms,
                        embed_ms       = snap.total_embed_ms,
                        upsert_ms      = snap.total_upsert_ms,
                        url_locks      = after,
                        uptime_m,
                        "pipeline alive"
                    );
                }
                _ = shutdown.cancelled() => break,
            }
        }
    })
}
