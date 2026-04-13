//! Canonical content store for search snapshots.
//!
//! The `.json` sidecar next to each `.md` now uses a versioned envelope format
//! (`Sidecar`) that keeps the full `ExtractionResult` in `current` plus a
//! `changelog` of every content change over time.  Old sidecars (raw
//! `ExtractionResult` JSON without a `schema_version` key) are migrated
//! transparently on first read.
use std::path::{Component, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Map a URL to a relative store path without extension.
pub fn url_to_store_path(url: &str) -> PathBuf {
    let parsed = match url::Url::parse(url) {
        Ok(url) => url,
        Err(_) => return PathBuf::from("unknown"),
    };

    let host = parsed.host_str().unwrap_or("unknown");
    let clean_host = sanitize_component(host.strip_prefix("www.").unwrap_or(host));

    let segments: Vec<String> = parsed
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        .map(sanitize_component)
        .collect();

    let path_part = if segments.is_empty() {
        "index".to_string()
    } else {
        segments.join("/")
    };

    let mut rel = format!("{clean_host}/{path_part}");
    if rel.len() > 240 {
        rel.truncate(240);
    }
    if parsed.query().is_some() {
        rel.push('_');
        rel.push_str(&format!("{:08x}", url_hash(url)));
    }

    PathBuf::from(rel)
}

fn url_hash(url: &str) -> u32 {
    url.bytes().fold(2166136261_u32, |acc, b| {
        (acc ^ (b as u32)).wrapping_mul(16777619)
    })
}

fn sanitize_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "index".to_string()
    } else {
        trimmed.to_string()
    }
}

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
// StoreResult
// ---------------------------------------------------------------------------

pub struct StoreResult {
    pub md_path: PathBuf,
    pub json_path: PathBuf,
    pub is_new: bool,
    pub changed: bool,
    pub word_count_delta: i64,
    /// The diff computed against the previous version, if any.
    pub diff: Option<noxa_core::ContentDiff>,
}

// ---------------------------------------------------------------------------
// ContentStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ContentStore {
    root: PathBuf,
    /// Maximum combined byte size of markdown + plain_text before a document is
    /// skipped (not written). Default: 2 MiB. `None` disables the guard.
    pub max_content_bytes: Option<usize>,
}

impl ContentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            max_content_bytes: Some(2 * 1024 * 1024),
        }
    }

    pub fn open() -> Self {
        let root = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".noxa")
            .join("content");
        Self {
            root,
            max_content_bytes: Some(2 * 1024 * 1024),
        }
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

        // Symlink protection: canonicalize the parent directory (which must
        // already exist or be created) to resolve any symlinks, then verify
        // that the final path still resides under the canonical root. We
        // canonicalize the parent rather than the full path because the file
        // itself may not exist yet at resolution time.
        let canonical_root = std::fs::canonicalize(&self.root).map_err(|e| {
            format!("store: cannot canonicalize root {}: {e}", self.root.display())
        })?;
        let parent = base.parent().unwrap_or(&base);
        // The parent may not exist yet (first write for this host). Use
        // the deepest ancestor that does exist to canonicalize, then
        // reconstruct the suffix.
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

        Ok(base)
    }

    // -----------------------------------------------------------------------
    // read() — returns the current ExtractionResult, transparent to format
    // -----------------------------------------------------------------------

    pub async fn read(
        &self,
        url: &str,
    ) -> Result<Option<noxa_core::ExtractionResult>, String> {
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
                    let parsed = tokio::task::spawn_blocking(move || {
                        parse_sidecar_or_migrate(&contents, mtime).ok()
                    })
                    .await
                    .unwrap_or(None);
                    parsed
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(format!("store: read previous: {e}")),
            };

        // ---- Strip query params from metadata.url before persisting --------
        let mut to_store = extraction.clone();
        if let Some(ref url_str) = to_store.metadata.url {
            if let Ok(mut u) = url::Url::parse(url_str) {
                u.set_query(None);
                to_store.metadata.url = Some(u.to_string());
            }
        }

        // ---- Build the updated sidecar and decide what changed -------------
        let (sidecar, is_new, changed, word_count_delta, diff_result) =
            if let Some(mut existing) = existing_sidecar {
                // Compare against previous current content.
                let content_diff = noxa_core::diff::diff(&existing.current, &to_store);
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
                }

                let diff_opt = if changed { Some(content_diff) } else { None };
                (existing, false, changed, wc_delta, diff_opt)
            } else {
                // First write — create new sidecar.
                let sidecar = Sidecar {
                    schema_version: 1,
                    url: url.to_string(),
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
        let markdown_bytes = to_store.content.markdown.as_bytes().to_vec();
        let json_bytes = tokio::task::spawn_blocking(move || {
            serde_json::to_vec(&sidecar).map_err(|e| format!("store: serialize: {e}"))
        })
        .await
        .map_err(|e| format!("store: serialize join: {e}"))??;

        // ---- Atomic writes -------------------------------------------------------
        let rand_suffix = {
            use rand::Rng;
            format!("{:016x}", rand::thread_rng().r#gen::<u64>())
        };

        // Always write the JSON sidecar (last_fetched / fetch_count always update).
        let tmp_json = json_path.with_extension(format!("json.{rand_suffix}.tmp"));
        tokio::fs::write(&tmp_json, &json_bytes)
            .await
            .map_err(|e| format!("store: write json.tmp: {e}"))?;
        tokio::fs::rename(&tmp_json, &json_path)
            .await
            .map_err(|e| format!("store: rename json: {e}"))?;

        // Only rewrite the .md when content changed or it's the first write.
        if write_md {
            let tmp_md = md_path.with_extension(format!("md.{rand_suffix}.tmp"));
            tokio::fs::write(&tmp_md, &markdown_bytes)
                .await
                .map_err(|e| format!("store: write md.tmp: {e}"))?;
            tokio::fs::rename(&tmp_md, &md_path)
                .await
                .map_err(|e| format!("store: rename md: {e}"))?;
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
/// wrapping it with `mtime` as `first_seen`.
fn parse_sidecar_or_legacy(
    contents: &str,
) -> Result<Sidecar, serde_json::Error> {
    // New format has `schema_version` key.
    if let Ok(sidecar) = serde_json::from_str::<Sidecar>(contents) {
        return Ok(sidecar);
    }
    // Legacy: raw ExtractionResult — no mtime available here.
    let extraction = serde_json::from_str::<noxa_core::ExtractionResult>(contents)?;
    let now = Utc::now();
    Ok(Sidecar {
        schema_version: 1,
        url: extraction.metadata.url.clone().unwrap_or_default(),
        first_seen: now,
        last_fetched: now,
        fetch_count: 1,
        changelog: vec![],
        current: extraction,
    })
}

/// Like `parse_sidecar_or_legacy` but uses `mtime` for `first_seen` when
/// migrating an old-format file.
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
        changelog: vec![],
        current: extraction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Component;

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
                technologies: Vec::new(),
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
    async fn test_url_to_store_path_root() {
        let p = url_to_store_path("https://example.com/");
        assert_eq!(p, std::path::PathBuf::from("example_com/index"));
    }

    #[tokio::test]
    async fn test_url_to_store_path_strips_www() {
        let p = url_to_store_path("https://www.rust-lang.org/learn");
        assert_eq!(p, std::path::PathBuf::from("rust-lang_org/learn"));
    }

    #[tokio::test]
    async fn test_url_to_store_path_query_discriminates() {
        let p1 = url_to_store_path("https://example.com/search?q=rust");
        let p2 = url_to_store_path("https://example.com/search?q=go");
        assert_ne!(p1, p2);
        let p1_str = p1.to_string_lossy();
        assert!(p1_str.starts_with("example_com/search_"));
    }

    #[tokio::test]
    async fn test_first_write_is_new() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
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
        let store = ContentStore::new(dir.path());
        let first = make_extraction("# Hello\n\nFirst content.");
        store
            .write("https://example.com/page", &first)
            .await
            .unwrap();
        let second = make_extraction("# Hello\n\nUpdated content.");
        let result = store
            .write("https://example.com/page", &second)
            .await
            .unwrap();
        assert!(!result.is_new);
        assert!(result.changed);
        assert!(result.word_count_delta != 0 || result.changed);
    }

    #[tokio::test]
    async fn test_identical_content_not_changed() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
        let e = make_extraction("# Same\n\nContent.");
        store.write("https://example.com/same", &e).await.unwrap();
        let result = store.write("https://example.com/same", &e).await.unwrap();
        assert!(!result.is_new);
        assert!(!result.changed);
    }

    #[tokio::test]
    async fn test_corrupted_prev_json_treated_as_new() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
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
        assert_eq!(url_hash("hello"), 0x4f9f2cab);
    }

    // --- tests for read(), Clone, atomic writes, query-param stripping ---

    #[tokio::test]
    async fn test_read_returns_none_for_unknown_url() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
        let result = store.read("https://example.com/never-written").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_read_returns_written_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
        let e = make_extraction("# Hello\n\nReadable content.");
        store.write("https://example.com/readable", &e).await.unwrap();
        let read_back = store
            .read("https://example.com/readable")
            .await
            .unwrap()
            .expect("should be Some after write");
        assert_eq!(read_back.content.markdown, e.content.markdown);
    }

    #[tokio::test]
    async fn test_read_corrupted_json_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
        let rel = url_to_store_path("https://example.com/bad-json");
        let json_path = dir.path().join(&rel).with_extension("json");
        tokio::fs::create_dir_all(json_path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&json_path, b"{{not valid json{{")
            .await
            .unwrap();
        let result = store.read("https://example.com/bad-json").await;
        assert!(result.is_err(), "corrupted JSON should return Err");
    }

    #[tokio::test]
    async fn test_read_path_escape_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
        let result = store
            .read("https://evil.com/../../etc/passwd")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_clone_produces_same_root() {
        let dir = tempfile::tempdir().unwrap();
        let a = ContentStore::new(dir.path());
        let b = a.clone();
        assert_eq!(a.root, b.root);
        assert_eq!(a.max_content_bytes, b.max_content_bytes);
    }

    #[tokio::test]
    async fn test_atomic_write_no_tmp_files_after_completion() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
        let e = make_extraction("# Atomic\n\nContent.");
        store
            .write("https://example.com/atomic", &e)
            .await
            .unwrap();
        // No .tmp files should remain
        let rel = url_to_store_path("https://example.com/atomic");
        let base = dir.path().join(&rel);
        assert!(!base.with_extension("md.tmp").exists());
        assert!(!base.with_extension("json.tmp").exists());
    }

    #[tokio::test]
    async fn test_metadata_url_query_params_stripped() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
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
        assert!(
            !stored_url.contains("token="),
            "query params must be stripped before storage"
        );
        assert!(
            !stored_url.contains("supersecret"),
            "secret value must not appear in stored URL"
        );
    }

    #[tokio::test]
    async fn test_max_content_bytes_guard_skips_oversized() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ContentStore::new(dir.path());
        store.max_content_bytes = Some(10); // tiny limit
        let e = make_extraction("# Big\n\nThis content is definitely more than 10 bytes.");
        let result = store
            .write("https://example.com/big", &e)
            .await
            .unwrap();
        // Document is skipped — is_new=false, changed=false
        assert!(!result.is_new);
        assert!(!result.changed);
        // File should NOT have been written
        let rel = url_to_store_path("https://example.com/big");
        assert!(!dir.path().join(&rel).with_extension("md").exists());
    }

    // -----------------------------------------------------------------------
    // New tests for the versioned sidecar format
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_sidecar_first_write_has_one_changelog_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
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
        let store = ContentStore::new(dir.path());
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
        let store = ContentStore::new(dir.path());
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
        let store = ContentStore::new(dir.path());
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
        let store = ContentStore::new(dir.path());
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
    }

    #[tokio::test]
    async fn test_sidecar_current_matches_read() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path());
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
}
