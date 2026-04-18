//! In-memory manifest cache for [`FilesystemContentStore`].
//!
//! Stores a lazy-populated `HashMap<file_path, StoredDoc>` that is kept valid by
//! invalidating the whole cache on every write operation.  Subsequent calls to
//! `list_all_docs()` avoid a full filesystem traversal as long as the cache is
//! fresh.
//!
//! ## Design choices
//!
//! * **Invalidate, don't update** — every `write()` sets the cache to `None`.
//!   Incremental maintenance is error-prone (oversized-skip path, changelog
//!   trimming, etc.).  The store is never large enough that re-populating the
//!   whole cache is expensive.
//! * **TTL safety net** — a 30-second TTL ensures the cache can never drift
//!   from disk state if the process is long-running or the store is modified
//!   externally.
//! * **Arc<tokio::sync::Mutex<…>>** so `FilesystemContentStore` stays `Clone`.
//! * **Errors are not cached** — if the walk fails, the caller gets the error
//!   and the cache remains `None`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use super::enumerate::StoredDoc;

/// Default cache TTL.  After this duration a cache hit is treated as a miss
/// and the full walk is re-run.
pub(crate) const CACHE_TTL: Duration = Duration::from_secs(30);

/// The payload stored inside the mutex.
#[derive(Debug)]
pub(crate) struct ManifestCache {
    /// All documents in the store, keyed by file path (string form) to
    /// preserve documents that share a URL (e.g. query-stripped duplicates).
    pub docs: HashMap<String, StoredDoc>,
    /// When the cache was last populated.
    pub populated_at: Instant,
}

impl ManifestCache {
    /// Returns `true` if the cache is still within its TTL.
    pub fn is_fresh(&self) -> bool {
        self.populated_at.elapsed() < CACHE_TTL
    }
}

/// A shareable, `Clone`-able handle to the manifest cache.
///
/// Internally an `Arc<Mutex<Option<ManifestCache>>>` so cloning the store
/// shares the same cache (consistent semantics across handles pointing to the
/// same store root).
#[derive(Clone, Debug)]
pub(crate) struct ManifestCacheHandle(pub Arc<Mutex<Option<ManifestCache>>>);

impl ManifestCacheHandle {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    /// Invalidate the cache.  Called by every write path so that the next
    /// `list_all_docs()` call triggers a fresh walk.
    ///
    /// # Known TOCTOU race
    /// A concurrent `list_all_docs()` call may observe the `None` set here,
    /// perform a full walk, and then overwrite this invalidation by storing a
    /// new snapshot that excludes the just-written document.  The 30-second TTL
    /// bounds the drift window.  A generation counter (increment on invalidate,
    /// check before committing the repopulated snapshot) would close this race
    /// but is deferred as a future improvement.
    pub async fn invalidate(&self) {
        let mut guard = self.0.lock().await;
        *guard = None;
    }
}
