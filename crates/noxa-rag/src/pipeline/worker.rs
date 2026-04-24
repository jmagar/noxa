use tokio::task::JoinHandle;
use tracing::Instrument;

use super::process;
use super::{Pipeline, PipelineJob, WorkerContext};

pub(super) fn spawn_workers(
    pipeline: &Pipeline,
    rx: async_channel::Receiver<PipelineJob>,
) -> Vec<JoinHandle<()>> {
    let ctx = WorkerContext {
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
    };
    let mut handles = Vec::with_capacity(pipeline.config.pipeline.embed_concurrency);

    for worker_id in 0..pipeline.config.pipeline.embed_concurrency {
        // async_channel is MPMC — every clone dequeues from the same shared
        // queue in parallel, eliminating the Arc<Mutex<Receiver>> serialisation
        // bottleneck of tokio::sync::mpsc.
        let rx = rx.clone();
        let ctx = ctx.clone();

        let handle = tokio::spawn(async move {
            tracing::debug!(worker_id, "index worker started");
            loop {
                let job = rx.recv().await;
                match job {
                    Ok(PipelineJob::Index(job)) => {
                        let span = job.span.clone();
                        async {
                            match process::process_job(job, &ctx).await {
                                Ok(stats) => {
                                    if stats.chunks > 0 {
                                        ctx.counters
                                            .files_indexed
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    ctx.counters.total_chunks.fetch_add(
                                        stats.chunks,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    ctx.counters.total_parse_ms.fetch_add(
                                        stats.parse_ms,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    ctx.counters.total_chunk_ms.fetch_add(
                                        stats.chunk_ms,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    ctx.counters.total_embed_ms.fetch_add(
                                        stats.embed_ms,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    ctx.counters.total_upsert_ms.fetch_add(
                                        stats.upsert_ms,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "index job failed");
                                    ctx.counters
                                        .files_failed
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                        }
                        .instrument(span)
                        .await;
                    }
                    Ok(PipelineJob::Delete(job)) => {
                        // Derive the file:// URL from the raw (non-canonicalized) path.
                        // canonicalize() would fail because the file no longer exists.
                        let span = job.span.clone();
                        async {
                            process::process_delete_job(job, &ctx.store).await;
                        }
                        .instrument(span)
                        .await;
                    }
                    Err(async_channel::RecvError) => {
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
