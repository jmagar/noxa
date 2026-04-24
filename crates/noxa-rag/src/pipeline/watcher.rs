use std::path::PathBuf;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use notify::event::{ModifyKind, RenameMode};
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use tokio::task::JoinHandle;

use crate::error::RagError;

use super::scan;
use super::{DeleteJob, IndexJob, PipelineJob};

struct BoundedSender(std::sync::mpsc::SyncSender<DebounceEventResult>);

impl notify_debouncer_full::DebounceEventHandler for BoundedSender {
    fn handle_event(&mut self, event: DebounceEventResult) {
        let _ = self.0.send(event);
    }
}

/// Send one job to the worker channel, backing off when the channel is full.
///
/// Called from the blocking bridge thread; parks the thread on the channel's
/// condvar until a slot opens or shutdown is requested.
pub(super) fn send_job(
    job: PipelineJob,
    tx: &async_channel::Sender<PipelineJob>,
    shutdown: &tokio_util::sync::CancellationToken,
) {
    if shutdown.is_cancelled() {
        return;
    }
    match tx.try_send(job) {
        Ok(()) => return,
        Err(async_channel::TrySendError::Closed(_)) => return,
        Err(async_channel::TrySendError::Full(j)) => {
            tracing::warn!("job queue saturated (256/256), backing off — embed/upsert catching up");
            // Park the blocking thread on the channel's condvar until a slot opens.
            // Zero CPU overhead vs. the previous 10ms-sleep spin. Safe to call from
            // spawn_blocking. Returns Err only when all receivers are dropped (shutdown
            // drain), at which point we exit cleanly.
            let _ = tx.send_blocking(j);
        }
    }
}

/// Creates the fs debouncer, registers the watch directory, and spawns the
/// blocking bridge task that forwards events into `tx`.  Returns the bridge
/// `JoinHandle` so the caller can await it during shutdown.
///
/// The debouncer is moved into the `spawn_blocking` closure so it stays alive
/// for the entire lifetime of the bridge task.
pub(super) fn setup_watcher(
    watch_dirs: &[PathBuf],
    debounce_ms: u64,
    tx: async_channel::Sender<PipelineJob>,
    shutdown: tokio_util::sync::CancellationToken,
) -> Result<JoinHandle<()>, RagError> {
    let (notify_tx, notify_rx) = std::sync::mpsc::sync_channel::<DebounceEventResult>(256);

    let mut debouncer =
        new_debouncer(Duration::from_millis(debounce_ms), None, BoundedSender(notify_tx))
            .map_err(|e| RagError::WatcherSetup(format!("failed to create fs watcher: {e}")))?;

    for watch_dir in watch_dirs {
        debouncer
            .watcher()
            .watch(watch_dir, RecursiveMode::Recursive)
            .map_err(|e| {
                RagError::WatcherSetup(format!(
                    "failed to watch directory {}: {e}",
                    watch_dir.display()
                ))
            })?;
        tracing::info!(path = %watch_dir.display(), "watching directory recursively");
    }

    // Capture watch_dirs for confinement checks in the bridge closure.
    let watch_dirs_owned: Vec<PathBuf> = watch_dirs.to_vec();

    let bridge_handle = tokio::task::spawn_blocking(move || {
        // Keep debouncer alive for the duration of the bridge.
        let _debouncer = debouncer;

        // Returns true iff `path`'s parent can be canonicalized within a watch dir.
        // Used to guard delete events — the deleted file itself cannot be canonicalized.
        let is_delete_path_confined = |path: &std::path::Path| -> bool {
            path.parent()
                .and_then(|p| p.canonicalize().ok())
                .map(|canon| watch_dirs_owned.iter().any(|d| canon.starts_with(d)))
                .unwrap_or(false)
        };

        loop {
            match notify_rx.recv_timeout(Duration::from_millis(250)) {
                Ok(Ok(events)) => {
                    if shutdown.is_cancelled() {
                        break;
                    }
                    for debounced in events {
                        let event = &debounced.event;
                        let kind = &event.kind;

                        // Rename — debouncer-full coalesces OS rename pairs into a single
                        // RenameMode::Both event: paths[0]=old, paths[1]=new.
                        // Emit DeleteJob(old) so stale chunks are removed, then index new path.
                        if matches!(
                            kind,
                            notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both))
                        ) && event.paths.len() == 2
                        {
                            let old_path = &event.paths[0];
                            let new_path = event.paths[1].clone();
                            if scan::has_indexable_extension(old_path)
                                && is_delete_path_confined(old_path)
                            {
                                let span = tracing::info_span!(
                                    "delete_job",
                                    path = %old_path.display()
                                );
                                send_job(
                                    PipelineJob::Delete(DeleteJob {
                                        path: old_path.clone(),
                                        span,
                                    }),
                                    &tx,
                                    &shutdown,
                                );
                            }
                            for path in scan::collect_indexable_paths(&new_path) {
                                let span =
                                    tracing::info_span!("index_job", path = %path.display());
                                send_job(
                                    PipelineJob::Index(IndexJob { path, span }),
                                    &tx,
                                    &shutdown,
                                );
                            }
                            continue;
                        }

                        // Explicit remove event — debouncer-full exposes EventKind::Remove
                        // directly (unlike debouncer-mini which coalesced everything into Any).
                        if matches!(kind, notify::EventKind::Remove(_)) {
                            for path in &event.paths {
                                if scan::has_indexable_extension(path)
                                    && is_delete_path_confined(path)
                                {
                                    let span = tracing::info_span!(
                                        "delete_job",
                                        path = %path.display()
                                    );
                                    send_job(
                                        PipelineJob::Delete(DeleteJob {
                                            path: path.clone(),
                                            span,
                                        }),
                                        &tx,
                                        &shutdown,
                                    );
                                }
                            }
                            continue;
                        }

                        // Create / Modify / Any — index all indexable paths.
                        for path in event.paths.iter().flat_map(|p| scan::collect_indexable_paths(p)) {
                            let span = tracing::info_span!("index_job", path = %path.display());
                            send_job(PipelineJob::Index(IndexJob { path, span }), &tx, &shutdown);
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
