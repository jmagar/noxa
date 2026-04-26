use tokio::task::JoinHandle;
use tracing::Instrument;

use super::process;
use super::{Pipeline, PipelineJob, WorkerContext};

pub(super) fn spawn_workers(
    pipeline: &Pipeline,
    rx: async_channel::Receiver<PipelineJob>,
) -> Vec<JoinHandle<()>> {
    let ctx = WorkerContext::from_pipeline(pipeline);
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
                                Ok(stats) => ctx.counters.record_success(&stats),
                                Err(e) => {
                                    tracing::error!(error = %e, "index job failed");
                                    ctx.counters.record_failure();
                                }
                            }
                        }
                        .instrument(span)
                        .await;
                    }
                    Ok(PipelineJob::Delete(job)) => {
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
