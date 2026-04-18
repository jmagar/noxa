//! In-memory manifest cache for [`FilesystemContentStore`].
//!
//! Stores a lazy-populated `HashMap<url, StoredDoc>` that is kept valid by
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
    /// All documents in the store, keyed by URL for O(1) exact-URL lookup.
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
    pub async fn invalidate(&self) {
        let mut guard = self.0.lock().await;
        *guard = None;
    }
}
