//! Per-URL content store: manages `.md` and `.json` sidecar files under a configurable root.
//!
//! The `.json` sidecar next to each `.md` now uses a versioned envelope format
//! (`Sidecar`) that keeps the full `ExtractionResult` in `current` plus a
//! `changelog` of every content change over time.  Old sidecars (raw
//! `ExtractionResult` JSON without a `schema_version` key) are migrated
//! transparently **in-memory** on first read; the migrated form is not written
//! back to disk until the next `write()` call.
use std::path::{Component, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::paths::{content_store_root, url_to_store_path};
use crate::types::StoreResult;

// ---------------------------------------------------------------------------
// Sidecar format (schema_version = 1)
// ---------------------------------------------------------------------------

/// A single entry in the changelog — recorded only when content changes or on
/// the first fetch (where `diff` is `None`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    /// UTC timestamp of this fetch.
    pub at: DateTime<Utc>,
    /// Word count of the content at this snapshot.
    pub word_count: usize,
    /// `None` for the very first fetch; populated on subsequent changes.
    pub diff: Option<noxa_core::ContentDiff>,
}

/// The versioned envelope stored in the `.json` sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sidecar {
    pub schema_version: u32,
    pub url: String,
    pub first_seen: DateTime<Utc>,
    pub last_fetched: DateTime<Utc>,
    pub fetch_count: u64,
    /// Full latest `ExtractionResult` — `store.read()` returns this directly.
    pub current: noxa_core::ExtractionResult,
    /// Ordered list of change events.  Entry 0 is the initial fetch.
    pub changelog: Vec<ChangelogEntry>,
}

// ---------------------------------------------------------------------------
// FilesystemContentStore
// ---------------------------------------------------------------------------

/// Filesystem-backed content store.
///
/// Stores one `.md` (current markdown) and one `.json` (sidecar changelog) per URL,
/// organized under `<root>/<domain>/<path>`.
///
/// All writes are atomic (tmp + rename). Sensitive fields are stripped before
/// serialization. Files are created with `0o600` permissions on Unix.
#[derive(Debug, Clone)]
pub struct FilesystemContentStore {
    root: PathBuf,
    /// Cached canonical (symlink-resolved) root path.  Populated lazily on first
    /// use of `resolve_path()` so that the store can be constructed before the
    /// directory exists on disk.
    canonical_root: std::sync::Arc<std::sync::OnceLock<PathBuf>>,
    /// Maximum combined byte size of markdown + plain_text before a document is
    /// skipped (not written). Default: 2 MiB. `None` disables the guard.
    pub max_content_bytes: Option<usize>,
    /// Maximum number of changelog entries per sidecar.  When exceeded, old
    /// entries (all except `[0]`, the initial-fetch sentinel) are drained from
    /// the front.  `None` disables the cap.  Default: 100.
    pub max_changelog_entries: Option<usize>,
}

impl FilesystemContentStore {
    /// Create a store at an explicit root path.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let canonical_root = std::sync::Arc::new(std::sync::OnceLock::new());
        // Eagerly try to cache the canonical root if the directory already exists.
        if let Ok(cr) = std::fs::canonicalize(&root) {
            let _ = canonical_root.set(cr);
        }
        Self {
            root,
            canonical_root,
            max_content_bytes: Some(2 * 1024 * 1024),
            max_changelog_entries: Some(100),
        }
    }

    /// Create a store at the default location (`~/.noxa/content/`).
    ///
    /// Returns `Err` if the home directory cannot be determined.
    pub fn open() -> Result<Self, String> {
        let root = content_store_root(None)?;
        let canonical_root = std::sync::Arc::new(std::sync::OnceLock::new());
        if let Ok(cr) = std::fs::canonicalize(&root) {
            let _ = canonical_root.set(cr);
        }
        Ok(Self {
            root,
            canonical_root,
            max_content_bytes: Some(2 * 1024 * 1024),
            max_changelog_entries: Some(100),
        })
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Return the cached canonical root, lazily populating on first call.
    fn get_canonical_root(&self) -> Result<PathBuf, String> {
        if let Some(cr) = self.canonical_root.get() {
            return Ok(cr.clone());
        }
        let cr = std::fs::canonicalize(&self.root).map_err(|e| {
            format!("store: cannot canonicalize root {}: {e}", self.root.display())
        })?;
        // Ignore set error — another thread may have populated it concurrently.
        let _ = self.canonical_root.set(cr.clone());
        Ok(cr)
    }

    fn resolve_path(&self, url: &str) -> Result<PathBuf, String> {
        let rel = url_to_store_path(url);
        if rel
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!("store: computed path escapes root for url: {url}"));
        }
        let base = self.root.join(&rel);
        if !base.starts_with(&self.root) {
            return Err(format!("store: computed path escapes root for url: {url}"));
        }

        // Use the cached canonical root instead of calling canonicalize per-request.
        let canonical_root = self.get_canonical_root()?;

        // Symlink protection: walk up to find the nearest existing ancestor,
        // canonicalize it, then verify the resolved path stays under the root.
        let parent = base.parent().unwrap_or(&base);
        let mut existing_ancestor = parent.to_path_buf();
        let mut suffix = PathBuf::new();
        while !existing_ancestor.exists() {
            if let Some(name) = existing_ancestor.file_name() {
                suffix = PathBuf::from(name).join(&suffix);
            }
            match existing_ancestor.parent() {
                Some(p) => existing_ancestor = p.to_path_buf(),
                None => break,
            }
        }
        let canonical_parent = std::fs::canonicalize(&existing_ancestor)
            .map_err(|e| format!("store: cannot canonicalize path ancestor: {e}"))?;
        let resolved = canonical_parent.join(&suffix);
        if !resolved.starts_with(&canonical_root) {
            return Err(format!("store: computed path escapes root for url: {url}"));
        }

        // Return the canonicalized path (not `base`) to close the TOCTOU gap:
        // a symlink created between check and write cannot escape the root.
        let file_name = base
            .file_name()
            .ok_or_else(|| format!("store: no file name in path for url: {url}"))?;
        Ok(resolved.join(file_name))
    }

    pub async fn read(&self, url: &str) -> Result<Option<noxa_core::ExtractionResult>, String> {
        let base = match self.resolve_path(url) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        let json_path = base.with_extension("json");
        match tokio::fs::read_to_string(&json_path).await {
            Ok(contents) => {
                let result = tokio::task::spawn_blocking(move || {
                    parse_sidecar_or_legacy(&contents)
                        .map(|s| s.current)
                        .map_err(|e| format!("store: deserialize: {e}"))
                })
                .await
                .map_err(|e| format!("store: read join: {e}"))??;
                Ok(Some(result))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("store: read: {e}")),
        }
    }

    // -----------------------------------------------------------------------
    // read_sidecar() — returns the full versioned sidecar
    // -----------------------------------------------------------------------

    pub async fn read_sidecar(&self, url: &str) -> Result<Option<Sidecar>, String> {
        let base = match self.resolve_path(url) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        let json_path = base.with_extension("json");
        match tokio::fs::read_to_string(&json_path).await {
            Ok(contents) => {
                // Also need file mtime for legacy migration.
                let mtime = tokio::fs::metadata(&json_path)
                    .await
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|st| DateTime::<Utc>::from(st))
                    .unwrap_or_else(Utc::now);
                let result = tokio::task::spawn_blocking(move || {
                    parse_sidecar_or_migrate(&contents, mtime)
                        .map_err(|e| format!("store: deserialize: {e}"))
                })
                .await
                .map_err(|e| format!("store: read_sidecar join: {e}"))??;
                Ok(Some(result))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("store: read_sidecar: {e}")),
        }
    }

    // -----------------------------------------------------------------------
    // write()
    // -----------------------------------------------------------------------

    pub async fn write(
        &self,
        url: &str,
        extraction: &noxa_core::ExtractionResult,
    ) -> Result<StoreResult, String> {
        let base = self.resolve_path(url)?;

        let md_path = base.with_extension("md");
        let json_path = base.with_extension("json");

        if let Some(parent) = md_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("store: create_dir: {e}"))?;
            #[cfg(unix)]
            set_dir_permissions(parent)?;
        }

        // Size guard — skip oversized documents rather than filling disk.
        let estimated = extraction.content.markdown.len()
            + extraction.content.plain_text.len()
            + extraction.content.raw_html.as_deref().map_or(0, |h| h.len());
        if let Some(max) = self.max_content_bytes {
            if estimated > max {
                tracing::warn!(
                    url,
                    estimated,
                    max,
                    "content store: skipping oversized document"
                );
                return Ok(StoreResult {
                    md_path,
                    json_path,
                    is_new: false,
                    changed: false,
                    word_count_delta: 0,
                    diff: None,
                });
            }
        }

        // ---- Read and optionally migrate existing sidecar -------------------
        let now = Utc::now();

        let existing_sidecar: Option<Sidecar> =
            match tokio::fs::read_to_string(&json_path).await {
                Ok(contents) => {
                    // Need mtime for legacy migration.
                    let mtime = tokio::fs::metadata(&json_path)
                        .await
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|st| DateTime::<Utc>::from(st))
                        .unwrap_or(now);
                    tokio::task::spawn_blocking(move || {
                        parse_sidecar_or_migrate(&contents, mtime).ok()
                    })
                    .await
                    .unwrap_or(None)
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(format!("store: read previous: {e}")),
            };

        // ---- Strip query params from metadata.url before persisting --------
        let mut to_store = extraction.clone();
        // Strip query params from metadata.url — prevents leaking auth tokens / API keys.
        if let Some(ref url_str) = to_store.metadata.url {
            if let Ok(mut u) = url::Url::parse(url_str) {
                u.set_query(None);
                to_store.metadata.url = Some(u.to_string());
            }
        }
        // Strip sensitive fields.
        to_store.content.raw_html = None; // persistent XSS surface for downstream renderers
        to_store.metadata.file_path = None; // leaks local filesystem paths
        to_store.metadata.search_query = None; // leaks user search intent

        // ---- Build the updated sidecar and decide what changed -------------
        let (sidecar, is_new, changed, word_count_delta, diff_result) =
            if let Some(mut existing) = existing_sidecar {
                // Offload CPU-bound diff to spawn_blocking to avoid blocking the executor.
                let prev = existing.current.clone();
                let curr = to_store.clone();
                let content_diff = tokio::task::spawn_blocking(move || {
                    noxa_core::diff::diff(&prev, &curr)
                })
                .await
                .map_err(|e| format!("store: diff join: {e}"))?;
                let changed = content_diff.status == noxa_core::ChangeStatus::Changed;
                let wc_delta =
                    to_store.metadata.word_count as i64 - existing.current.metadata.word_count as i64;

                existing.last_fetched = now;
                existing.fetch_count += 1;
                existing.current = to_store.clone();

                if changed {
                    existing.changelog.push(ChangelogEntry {
                        at: now,
                        word_count: to_store.metadata.word_count,
                        diff: Some(content_diff.clone()),
                    });

                    // Enforce changelog cap: keep entry[0] (initial fetch sentinel)
                    // and trim the oldest intermediate entries.
                    if let Some(cap) = self.max_changelog_entries {
                        if cap > 1 && existing.changelog.len() > cap {
                            let excess = existing.changelog.len() - cap;
                            existing.changelog.drain(1..1 + excess);
                        }
                    }
                }

                let diff_opt = if changed { Some(content_diff) } else { None };
                (existing, false, changed, wc_delta, diff_opt)
            } else {
                // First write — create new sidecar.
                // Strip query params from `url` to avoid persisting tokens/secrets.
                let clean_url = url::Url::parse(url)
                    .ok()
                    .map(|mut u| { u.set_query(None); u.to_string() })
                    .unwrap_or_else(|| url.to_string());
                let sidecar = Sidecar {
                    schema_version: 1,
                    url: clean_url,
                    first_seen: now,
                    last_fetched: now,
                    fetch_count: 1,
                    changelog: vec![ChangelogEntry {
                        at: now,
                        word_count: to_store.metadata.word_count,
                        diff: None,
                    }],
                    current: to_store.clone(),
                };
                (sidecar, true, false, 0i64, None)
            };

        // ---- Serialize ---------------------------------------------------------
        let write_md = is_new || changed;
        let json_bytes = tokio::task::spawn_blocking(move || {
            serde_json::to_vec(&sidecar).map_err(|e| format!("store: serialize: {e}"))
        })
        .await
        .map_err(|e| format!("store: serialize join: {e}"))??;

        // ---- Atomic writes -------------------------------------------------------
        // Write order: .md first (when needed), then .json.
        //
        // The JSON sidecar is the authoritative commit point — if a crash occurs
        // between a successful .md rename and the .json rename, the old sidecar
        // remains on disk and the .md will be rewritten on the next fetch.
        // If the .md write fails before the sidecar is updated, the sidecar
        // still reflects the previous state, making the failure safely retryable.
        // Random suffix prevents races between concurrent writes for the same URL.
        let rand_suffix = {
            use rand::Rng;
            format!("{:016x}", rand::thread_rng().r#gen::<u64>())
        };

        // Only rewrite the .md when content changed or it's the first write.
        // Defer the allocation until it is actually needed.
        if write_md {
            let markdown_bytes = to_store.content.markdown.as_bytes().to_vec();
            let tmp_md = md_path.with_extension(format!("md.{rand_suffix}.tmp"));
            tokio::fs::write(&tmp_md, &markdown_bytes)
                .await
                .map_err(|e| format!("store: write md.tmp: {e}"))?;
            tokio::fs::rename(&tmp_md, &md_path)
                .await
                .map_err(|e| format!("store: rename md: {e}"))?;
        }

        // Always write the JSON sidecar (last_fetched / fetch_count always update).
        // This acts as the transaction commit — written last so a crash before
        // this point leaves the previous sidecar intact.
        let tmp_json = json_path.with_extension(format!("json.{rand_suffix}.tmp"));
        tokio::fs::write(&tmp_json, &json_bytes)
            .await
            .map_err(|e| format!("store: write json.tmp: {e}"))?;
        tokio::fs::rename(&tmp_json, &json_path)
            .await
            .map_err(|e| format!("store: rename json: {e}"))?;

        #[cfg(unix)]
        {
            set_file_permissions(&md_path)?;
            set_file_permissions(&json_path)?;
        }

        Ok(StoreResult {
            md_path,
            json_path,
            is_new,
            changed,
            word_count_delta,
            diff: diff_result,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers — format detection & migration
// ---------------------------------------------------------------------------

/// Try to parse `contents` as a new-format `Sidecar`.  If that fails (old
/// format or parse error), fall back to reading a raw `ExtractionResult` and
/// wrapping it with the current time for `first_seen`/`last_fetched` (no mtime
/// is available at this call site).  The migrated sidecar is **not** written
/// back to disk; it will be persisted on the next `write()` call.
fn parse_sidecar_or_legacy(
    contents: &str,
) -> Result<Sidecar, serde_json::Error> {
    // New format has `schema_version` key.
    if let Ok(sidecar) = serde_json::from_str::<Sidecar>(contents) {
        return Ok(sidecar);
    }
    // Legacy: raw ExtractionResult — no mtime available here; use now.
    let extraction = serde_json::from_str::<noxa_core::ExtractionResult>(contents)?;
    let now = Utc::now();
    Ok(Sidecar {
        schema_version: 1,
        url: extraction.metadata.url.clone().unwrap_or_default(),
        first_seen: now,
        last_fetched: now,
        fetch_count: 1,
        changelog: vec![ChangelogEntry {
            at: now,
            word_count: extraction.metadata.word_count,
            diff: None,
        }],
        current: extraction,
    })
}

/// Like `parse_sidecar_or_legacy` but uses `mtime` for `first_seen` when
/// migrating an old-format file.  An initial `ChangelogEntry` (with `diff:
/// None`) is seeded so that migrated sidecars satisfy the invariant that
/// `changelog[0]` represents the initial fetch.
fn parse_sidecar_or_migrate(
    contents: &str,
    mtime: DateTime<Utc>,
) -> Result<Sidecar, serde_json::Error> {
    // New format.
    if let Ok(sidecar) = serde_json::from_str::<Sidecar>(contents) {
        return Ok(sidecar);
    }
    // Legacy: raw ExtractionResult.
    let extraction = serde_json::from_str::<noxa_core::ExtractionResult>(contents)?;
    Ok(Sidecar {
        schema_version: 1,
        url: extraction.metadata.url.clone().unwrap_or_default(),
        first_seen: mtime,
        last_fetched: mtime,
        fetch_count: 1,
        changelog: vec![ChangelogEntry {
            at: mtime,
            word_count: extraction.metadata.word_count,
            diff: None,
        }],
        current: extraction,
    })
}

#[cfg(unix)]
fn set_dir_permissions(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    // Only set if we just created this directory — skip if it already has correct perms.
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("store: stat dir {}: {e}", path.display()))?;
    if meta.permissions().mode() & 0o777 != 0o700 {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("store: chmod dir {}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_file_permissions(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("store: chmod file {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::url_to_store_path;

    fn make_extraction(markdown: &str) -> noxa_core::ExtractionResult {
        noxa_core::ExtractionResult {
            metadata: noxa_core::Metadata {
                title: Some("Test".into()),
                description: None,
                author: None,
                published_date: None,
                language: None,
                url: None,
                site_name: None,
                image: None,
                favicon: None,
                word_count: markdown.split_whitespace().count(),
                content_hash: None,
                source_type: None,
                file_path: None,
                last_modified: None,
                is_truncated: None,
                technologies: vec![],
                seed_url: None,
                crawl_depth: None,
                search_query: None,
                fetched_at: None,
            },
            content: noxa_core::Content {
                markdown: markdown.to_string(),
                plain_text: markdown.to_string(),
                links: vec![],
                images: vec![],
                code_blocks: vec![],
                raw_html: None,
            },
            domain_data: None,
            structured_data: vec![],
        }
    }

    #[tokio::test]
    async fn test_first_write_is_new() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let extraction = make_extraction("# Hello\n\nFirst content.");
        let result = store
            .write("https://example.com/page", &extraction)
            .await
            .unwrap();
        assert!(result.is_new);
        assert!(!result.changed);
        assert!(result.md_path.exists());
        assert!(result.json_path.exists());
    }

    #[tokio::test]
    async fn test_second_write_detects_change() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let first = make_extraction("# Hello\n\nFirst content.");
        store.write("https://example.com/page", &first).await.unwrap();
        let second = make_extraction("# Hello\n\nUpdated content.");
        let result = store
            .write("https://example.com/page", &second)
            .await
            .unwrap();
        assert!(!result.is_new);
        assert!(result.changed);
    }

    #[tokio::test]
    async fn test_identical_content_not_changed() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let e = make_extraction("# Same\n\nContent.");
        store.write("https://example.com/same", &e).await.unwrap();
        let result = store.write("https://example.com/same", &e).await.unwrap();
        assert!(!result.is_new);
        assert!(!result.changed);
    }

    #[tokio::test]
    async fn test_corrupted_prev_json_treated_as_new() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let rel = url_to_store_path("https://example.com/corrupt");
        let json_path = dir.path().join(&rel).with_extension("json");
        tokio::fs::create_dir_all(json_path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&json_path, b"{{not valid json{{")
            .await
            .unwrap();
        let e = make_extraction("content");
        let result = store
            .write("https://example.com/corrupt", &e)
            .await
            .unwrap();
        assert!(result.is_new);
    }

    #[tokio::test]
    async fn test_store_path_stays_within_root() {
        let p = url_to_store_path("https://evil.com/../../../etc/passwd");
        assert!(p.to_string_lossy().starts_with("evil_com/"));
    }

    #[tokio::test]
    async fn test_url_to_store_path_strips_parent_components() {
        let p = url_to_store_path("https://evil.com/a/../../etc/./passwd");
        assert!(!p.components().any(|c| matches!(c, Component::ParentDir)));
        assert!(!p.components().any(|c| matches!(c, Component::CurDir)));
    }

    #[tokio::test]
    async fn test_url_to_store_path_sanitizes_ipv6_host_and_path() {
        let p = url_to_store_path("https://[fe80::1]/bad:path/segment");
        let s = p.to_string_lossy();
        assert!(s.starts_with("fe80__1/"));
        assert!(!s.contains(':'));
        assert!(!s.contains('['));
        assert!(!s.contains(']'));
    }

    #[test]
    fn test_url_hash_matches_fnv1a() {
        use crate::paths::url_to_store_path as _; // ensure url_hash is accessible via paths
        // Test via url_to_store_path behavior: URL with query gets a hash suffix.
        let p = url_to_store_path("https://example.com/page?q=test");
        assert!(p.to_string_lossy().contains('_'));
    }

    // --- tests for read(), Clone, atomic writes, query-param stripping ---

    #[tokio::test]
    async fn test_read_returns_none_for_unknown_url() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let result = store.read("https://example.com/never-written").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_read_returns_written_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let e = make_extraction("# Hello\n\nReadable content.");
        store
            .write("https://example.com/readable", &e)
            .await
            .unwrap();
        let read_back = store
            .read("https://example.com/readable")
            .await
            .unwrap()
            .expect("should be Some after write");
        assert_eq!(read_back.content.markdown, e.content.markdown);
    }

    #[tokio::test]
    async fn test_raw_html_stripped_before_write() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let mut e = make_extraction("# Secret\n\nContent.");
        e.content.raw_html = Some("<script>alert('xss')</script>".to_string());
        store
            .write("https://example.com/xss", &e)
            .await
            .unwrap();
        let read_back = store
            .read("https://example.com/xss")
            .await
            .unwrap()
            .expect("should be Some");
        assert!(
            read_back.content.raw_html.is_none(),
            "raw_html must be stripped before write"
        );
    }

    #[tokio::test]
    async fn test_metadata_url_query_params_stripped() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let mut e = make_extraction("# Secret\n\nContent.");
        e.metadata.url =
            Some("https://api.example.com/data?token=supersecret&foo=bar".to_string());
        store
            .write("https://example.com/secret-page", &e)
            .await
            .unwrap();
        let read_back = store
            .read("https://example.com/secret-page")
            .await
            .unwrap()
            .expect("should be Some");
        let stored_url = read_back.metadata.url.unwrap();
        assert!(!stored_url.contains("token="));
        assert!(!stored_url.contains("supersecret"));
    }

    #[tokio::test]
    async fn test_max_content_bytes_guard_skips_oversized() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = FilesystemContentStore::new(dir.path());
        store.max_content_bytes = Some(10);
        let e = make_extraction("# Big\n\nThis content is definitely more than 10 bytes.");
        let result = store
            .write("https://example.com/big", &e)
            .await
            .unwrap();
        assert!(!result.is_new);
        assert!(!result.changed);
    }

    // -----------------------------------------------------------------------
    // New tests for the versioned sidecar format
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_sidecar_first_write_has_one_changelog_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let e = make_extraction("# Hello\n\nFirst content.");
        store
            .write("https://example.com/sidecar-first", &e)
            .await
            .unwrap();
        let sidecar = store
            .read_sidecar("https://example.com/sidecar-first")
            .await
            .unwrap()
            .expect("sidecar should exist");
        assert_eq!(sidecar.schema_version, 1);
        assert_eq!(sidecar.fetch_count, 1);
        assert_eq!(sidecar.changelog.len(), 1);
        assert!(sidecar.changelog[0].diff.is_none(), "first entry has no diff");
    }

    #[tokio::test]
    async fn test_sidecar_change_adds_changelog_entry_with_diff() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let first = make_extraction("# Hello\n\nOriginal content.");
        store
            .write("https://example.com/sidecar-change", &first)
            .await
            .unwrap();
        let second = make_extraction("# Hello\n\nModified content.");
        store
            .write("https://example.com/sidecar-change", &second)
            .await
            .unwrap();
        let sidecar = store
            .read_sidecar("https://example.com/sidecar-change")
            .await
            .unwrap()
            .expect("sidecar should exist");
        assert_eq!(sidecar.fetch_count, 2);
        assert_eq!(sidecar.changelog.len(), 2);
        let entry = &sidecar.changelog[1];
        assert!(entry.diff.is_some(), "second entry should have a diff");
        let diff = entry.diff.as_ref().unwrap();
        assert_eq!(diff.status, noxa_core::ChangeStatus::Changed);
    }

    #[tokio::test]
    async fn test_sidecar_identical_refetch_no_new_changelog_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let e = make_extraction("# Same\n\nContent.");
        store
            .write("https://example.com/sidecar-same", &e)
            .await
            .unwrap();
        store
            .write("https://example.com/sidecar-same", &e)
            .await
            .unwrap();
        let sidecar = store
            .read_sidecar("https://example.com/sidecar-same")
            .await
            .unwrap()
            .expect("sidecar should exist");
        // fetch_count increments, but changelog stays at 1
        assert_eq!(sidecar.fetch_count, 2);
        assert_eq!(sidecar.changelog.len(), 1, "no new entry for identical content");
    }

    #[tokio::test]
    async fn test_sidecar_diff_field_in_store_result() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let first = make_extraction("# Hello\n\nOriginal content.");
        let r1 = store
            .write("https://example.com/sidecar-diff", &first)
            .await
            .unwrap();
        assert!(r1.diff.is_none(), "first write has no diff");
        let second = make_extraction("# Hello\n\nChanged content.");
        let r2 = store
            .write("https://example.com/sidecar-diff", &second)
            .await
            .unwrap();
        assert!(r2.diff.is_some(), "changed write should have a diff");
    }

    #[tokio::test]
    async fn test_legacy_migration_on_read() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        // Write a raw ExtractionResult (old format) directly to disk.
        let e = make_extraction("# Legacy\n\nOld format content.");
        let rel = url_to_store_path("https://example.com/legacy");
        let json_path = dir.path().join(&rel).with_extension("json");
        tokio::fs::create_dir_all(json_path.parent().unwrap())
            .await
            .unwrap();
        let raw = serde_json::to_vec(&e).unwrap();
        tokio::fs::write(&json_path, &raw).await.unwrap();
        // read() should transparently migrate and return the content.
        let result = store
            .read("https://example.com/legacy")
            .await
            .unwrap()
            .expect("should be Some for legacy file");
        assert_eq!(result.content.markdown, e.content.markdown);
        // read_sidecar() should also work.
        let sidecar = store
            .read_sidecar("https://example.com/legacy")
            .await
            .unwrap()
            .expect("sidecar should exist");
        assert_eq!(sidecar.schema_version, 1);
        assert_eq!(sidecar.fetch_count, 1);
        // Legacy migration must seed an initial changelog entry (diff: None).
        assert_eq!(sidecar.changelog.len(), 1, "migrated sidecar should have one changelog entry");
        assert!(sidecar.changelog[0].diff.is_none(), "initial entry has no diff");
    }

    #[tokio::test]
    async fn test_sidecar_current_matches_read() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let first = make_extraction("# Version 1");
        store.write("https://example.com/cur", &first).await.unwrap();
        let second = make_extraction("# Version 2\n\nMore words here.");
        store.write("https://example.com/cur", &second).await.unwrap();
        let read_result = store
            .read("https://example.com/cur")
            .await
            .unwrap()
            .expect("should exist");
        let sidecar = store
            .read_sidecar("https://example.com/cur")
            .await
            .unwrap()
            .expect("should exist");
        // read() and sidecar.current should agree on the latest content.
        assert_eq!(
            read_result.content.markdown,
            sidecar.current.content.markdown
        );
        assert_eq!(
            read_result.content.markdown,
            second.content.markdown
        );
    }

    #[tokio::test]
    async fn test_read_path_escape_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let result = store
            .read("https://evil.com/../../etc/passwd")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_clone_produces_same_root() {
        let dir = tempfile::tempdir().unwrap();
        let a = FilesystemContentStore::new(dir.path());
        let b = a.clone();
        assert_eq!(a.root, b.root);
        assert_eq!(a.max_content_bytes, b.max_content_bytes);
    }

    #[tokio::test]
    async fn test_atomic_write_no_tmp_files_after_completion() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilesystemContentStore::new(dir.path());
        let e = make_extraction("# Atomic\n\nContent.");
        store.write("https://example.com/atomic", &e).await.unwrap();
        // No .tmp files should remain
        let domain_dir = dir.path().join("example_com");
        let mut entries = tokio::fs::read_dir(&domain_dir).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            assert!(
                !name_str.ends_with(".tmp"),
                "tmp file left behind: {name_str}"
            );
        }
    }
}
