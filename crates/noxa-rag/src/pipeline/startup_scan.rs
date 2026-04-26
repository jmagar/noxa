use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::stream::{self, StreamExt};
use tokio::task::JoinHandle;

use crate::store::{DynVectorStore, HashExistsResult};

use super::scan;
use super::{IndexJob, PipelineJob};

/// Number of paths batched into a single `spawn_blocking` call during startup scan.
///
/// With ~few µs per `startup_scan_key` call, a batch of 256 completes in the low-ms range,
/// keeping cancellation latency bounded while amortizing per-task scheduler overhead across
/// 256 paths instead of 1. At 10 000 files this produces ≈40 batches — comfortably spread
/// across `scan_concurrency` blocking workers.
const STARTUP_SCAN_BATCH: usize = 256;

pub(super) fn spawn_startup_scan(
    tx: async_channel::Sender<PipelineJob>,
    store: DynVectorStore,
    shutdown: tokio_util::sync::CancellationToken,
    watch_dirs: Vec<PathBuf>,
    scan_concurrency: usize,
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

        // Batch paths into groups of STARTUP_SCAN_BATCH before dispatching to the blocking
        // thread pool. This reduces per-task scheduler overhead from O(N) spawn_blocking calls
        // (one per file) to O(N / STARTUP_SCAN_BATCH) calls, saving ~2–5µs per file on large
        // startup scans while keeping cancellation latency bounded to one batch worth of work.
        let batches: Vec<Vec<PathBuf>> = paths
            .chunks(STARTUP_SCAN_BATCH)
            .map(|c| c.to_vec())
            .collect();

        stream::iter(batches)
            .for_each_concurrent(scan_concurrency, |batch| {
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

                    // Compute startup_scan_key for every path in the batch inside a single
                    // blocking task, returning (PathBuf, Option<(hash, url)>) per path.
                    let keys: Vec<(PathBuf, Option<(String, String)>)> =
                        match tokio::task::spawn_blocking(move || {
                            batch
                                .into_iter()
                                .map(|p| {
                                    let k = scan::startup_scan_key(&p);
                                    (p, k)
                                })
                                .collect()
                        })
                        .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!(error = %e, "startup scan: batch spawn_blocking panicked");
                                return;
                            }
                        };

                    for (path, hash_and_url) in keys {
                        if shutdown.is_cancelled() {
                            return;
                        }

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
                                    _ = tx.send(PipelineJob::Index(IndexJob { path, span })) => {}
                                    _ = shutdown.cancelled() => {}
                                }
                                queued.fetch_add(1, Ordering::Relaxed);
                                continue;
                            }
                        };

                        match store.url_with_file_hash_exists_checked(&url, &hash).await {
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
                                    _ = tx.send(PipelineJob::Index(IndexJob { path, span })) => {}
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use tempfile::tempdir;
    use tokio_util::sync::CancellationToken;

    use crate::error::RagError;
    use crate::store::{DynVectorStore, HashExistsResult, VectorStore};
    use crate::types::{Point, SearchMetadataFilter, SearchResult};

    use super::super::PipelineJob;
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

        async fn url_with_file_hash_exists_checked(
            &self,
            url: &str,
            _file_hash: &str,
        ) -> HashExistsResult {
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

    fn file_url(path: &std::path::Path) -> String {
        url::Url::from_file_path(path)
            .expect("from_file_path")
            .to_string()
    }

    async fn run_scan_and_collect(store: DynVectorStore, watch_dir: PathBuf) -> HashSet<PathBuf> {
        let shutdown = CancellationToken::new();
        let (tx, rx) = async_channel::bounded::<PipelineJob>(256);

        let handle = spawn_startup_scan(tx.clone(), store, shutdown.clone(), vec![watch_dir], 16);

        handle.await.expect("startup scan panicked");
        drop(tx);

        let mut queued = HashSet::new();
        while let Ok(job) = rx.try_recv() {
            if let PipelineJob::Index(index_job) = job {
                queued.insert(index_job.path);
            }
        }
        queued
    }

    #[tokio::test]
    async fn startup_scan_queues_not_indexed_files() {
        let dir = tempdir().expect("tempdir");

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
