//! In-memory manifest cache for [`FilesystemContentStore`].
//!
//! Stores a lazy-populated `HashMap<file_path, StoredDoc>` that is kept valid by
//! invalidating the whole cache on every write operation.  Subsequent calls to
//! `list_all_docs()` avoid a full filesystem traversal as long as the cache is
//! fresh.
//!
//! ## Design choices
//!
//! * **Invalidate, don't update** — every `write()` clears the cache and bumps a
//!   generation token.  Incremental maintenance is error-prone (oversized-skip
//!   path, changelog trimming, etc.).
//! * **TTL safety net** — a 30-second TTL ensures the cache can never drift
//!   from disk state if the process is long-running or the store is modified
//!   externally.
//! * **Generation checks** — `list_all_docs()` captures a generation token before
//!   walking and only publishes a fresh snapshot when the token is unchanged.
//! * **Arc<tokio::sync::Mutex<…>>** so `FilesystemContentStore` stays `Clone`.
//! * **Errors are not cached** — if the walk fails, the caller gets the error
//!   and the cache remains invalid.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use super::enumerate::StoredDoc;

/// Default cache TTL.  After this duration a cache hit is treated as a miss
/// and the full walk is re-run.
pub(crate) const CACHE_TTL: Duration = Duration::from_secs(30);

/// The payload stored for cached manifests.
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

#[derive(Debug)]
pub(crate) struct ManifestCacheState {
    pub cache: Option<ManifestCache>,
    pub generation: u64,
}

/// A shareable, `Clone`-able handle to the manifest cache.
///
/// Internally an `Arc<tokio::sync::Mutex<...>>` so cloning the store shares the
/// same state (consistent semantics across handles pointing to the same store
/// root).
#[derive(Clone, Debug)]
pub(crate) struct ManifestCacheHandle(pub Arc<Mutex<ManifestCacheState>>);

impl ManifestCacheHandle {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ManifestCacheState {
            cache: None,
            generation: 0,
        })))
    }

    /// Capture the current generation token for a traversal race check.
    pub async fn generation_for_walk(&self) -> u64 {
        self.0.lock().await.generation
    }

    /// Invalidate the cache and advance the generation token.
    ///
    /// `list_all_docs()` records the generation before traversal and only
    /// publishes a new snapshot if the token is unchanged when the walk
    /// completes.
    pub async fn invalidate(&self) {
        let mut guard = self.0.lock().await;
        guard.generation = guard.generation.saturating_add(1);
        guard.cache = None;
    }
}
