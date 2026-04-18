//! Store-native enumeration: list domains, list docs per domain, iterate all sidecars.
//!
//! This module consolidates filesystem traversal, sidecar parsing, and legacy-envelope
//! compatibility that previously lived in the CLI layer.

use std::path::{Path, PathBuf};

use chrono::DateTime;

use crate::content_store::migrate::parse_sidecar_or_migrate;
use crate::content_store::FilesystemContentStore;
use crate::paths::sanitize_component;
use crate::types::StoreError;

// ── Public types ─────────────────────────────────────────────────────────────

/// One document entry returned by enumeration APIs.
#[derive(Debug, Clone)]
pub struct StoredDoc {
    /// The URL of the document (from sidecar or reconstructed from path).
    pub url: String,
    /// Absolute path to the `.md` file.
    pub md_path: PathBuf,
    /// Absolute path to the `.json` sidecar.
    pub json_path: PathBuf,
    /// Title extracted from the sidecar, if available.
    pub title: Option<String>,
}

/// One domain entry returned by [`FilesystemContentStore::list_domains`].
#[derive(Debug, Clone)]
pub struct DomainEntry {
    /// Sanitized directory name (e.g. `"docs_example_com"`).
    pub name: String,
    /// Number of `.md` files under this domain directory.
    pub doc_count: usize,
}

// ── FilesystemContentStore impl ──────────────────────────────────────────────

impl FilesystemContentStore {
    /// Return all domain directories under the store root, sorted alphabetically,
    /// together with their `.md` doc counts.
    ///
    /// Returns an empty `Vec` (not an error) when the root does not exist yet.
    pub async fn list_domains(&self) -> Result<Vec<DomainEntry>, StoreError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut read_dir = match tokio::fs::read_dir(&self.root).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut entries: Vec<DomainEntry> = Vec::new();
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let ft = match tokio::fs::symlink_metadata(&path).await {
                Ok(m) => m.file_type(),
                Err(_) => continue,
            };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let doc_count = count_md_files_sync(&path);
            entries.push(DomainEntry { name, doc_count });
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    /// Return all documents stored under the given domain, sorted by path.
    ///
    /// `domain` can be a raw display form (`"docs.example.com"` or
    /// `"www.docs.example.com"`); it is sanitized internally to match the
    /// on-disk directory name.
    ///
    /// Returns an empty `Vec` when the domain directory does not exist.
    /// Corrupt sidecars are skipped with a warning (matching pre-existing CLI
    /// behaviour) rather than propagating an error.
    pub async fn list_docs(&self, domain: &str) -> Result<Vec<StoredDoc>, StoreError> {
        let dir = match self.domain_dir(domain) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let canonical_root = self.get_canonical_root()?;
        let mut docs = Vec::new();
        collect_docs_async(&dir, &self.root, &canonical_root, &mut docs).await;
        docs.sort_by(|a, b| a.md_path.cmp(&b.md_path));
        Ok(docs)
    }

    /// Return all documents in the entire store, sorted by path.
    ///
    /// Corrupt sidecars are skipped with a warning.
    pub async fn list_all_docs(&self) -> Result<Vec<StoredDoc>, StoreError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let canonical_root = self.get_canonical_root()?;
        let mut read_dir = match tokio::fs::read_dir(&self.root).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut docs = Vec::new();
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let ft = match tokio::fs::symlink_metadata(&path).await {
                Ok(m) => m.file_type(),
                Err(_) => continue,
            };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            collect_docs_async(&path, &self.root, &canonical_root, &mut docs).await;
        }

        docs.sort_by(|a, b| a.md_path.cmp(&b.md_path));
        Ok(docs)
    }

    /// Collect all URLs stored under `domain`, used by the refresh workflow.
    ///
    /// This replaces the CLI-level `collect_refresh_urls` function with an
    /// implementation that lives inside the storage boundary, applies the same
    /// symlink-escape check, and uses typed sidecar parsing.
    pub async fn list_domain_urls(&self, domain: &str) -> Result<Vec<String>, StoreError> {
        let dir = match self.domain_dir(domain) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let canonical_root = self.get_canonical_root()?;
        let mut urls: Vec<String> = Vec::new();
        collect_urls_async(&dir, &canonical_root, &mut urls).await;
        urls.sort();
        urls.dedup();
        Ok(urls)
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Resolve the on-disk directory for `domain`, sanitizing input.
    /// Returns `None` if the domain component is empty after sanitization.
    fn domain_dir(&self, domain: &str) -> Option<PathBuf> {
        let raw = domain.trim();
        // Strip leading "www." before sanitizing (mirrors CLI behaviour).
        let raw = raw.strip_prefix("www.").unwrap_or(raw);
        let component = sanitize_component(raw);
        if component.is_empty() || component == "index" {
            return None;
        }
        Some(self.root.join(component))
    }
}

// ── Async filesystem helpers ──────────────────────────────────────────────────

/// Recursively walk `dir` and collect `StoredDoc` entries for every `.md` file.
///
/// Symlinks that would escape `canonical_root` are silently skipped.
/// Corrupt/missing sidecars fall back to URL reconstruction from path.
async fn collect_docs_async(
    dir: &Path,
    store_root: &Path,
    canonical_root: &Path,
    out: &mut Vec<StoredDoc>,
) {
    // Safety: reject any traversal that escapes the canonical root.
    match tokio::fs::canonicalize(dir).await {
        Ok(canonical_dir) if canonical_dir.starts_with(canonical_root) => {}
        _ => return, // escape or I/O error → skip
    }

    let mut read_dir = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut paths: Vec<PathBuf> = Vec::new();
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        paths.push(entry.path());
    }
    paths.sort();

    for path in paths {
        let ft = match tokio::fs::symlink_metadata(&path).await {
            Ok(m) => m.file_type(),
            Err(_) => continue,
        };

        if ft.is_symlink() {
            // Do not follow symlinks — they can escape the root.
            continue;
        }

        if ft.is_dir() {
            // Use Box::pin to allow async recursion.
            Box::pin(collect_docs_async(&path, store_root, canonical_root, out)).await;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let json_path = path.with_extension("json");
        let (url, title) = parse_sidecar_for_doc(&json_path).await;
        let url = url.or_else(|| reconstruct_url_from_store_path(&path, store_root));
        if let Some(url) = url {
            out.push(StoredDoc {
                url,
                md_path: path,
                json_path,
                title,
            });
        }
    }
}

/// Recursively collect URLs from `.json` sidecars under `dir`.
///
/// Symlinks that would escape `canonical_root` are silently skipped.
async fn collect_urls_async(dir: &Path, canonical_root: &Path, out: &mut Vec<String>) {
    // Safety: reject traversal escaping the canonical root.
    match tokio::fs::canonicalize(dir).await {
        Ok(canonical_dir) if canonical_dir.starts_with(canonical_root) => {}
        _ => return,
    }

    let mut read_dir = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut paths: Vec<PathBuf> = Vec::new();
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        paths.push(entry.path());
    }
    paths.sort();

    for path in paths {
        let ft = match tokio::fs::symlink_metadata(&path).await {
            Ok(m) => m.file_type(),
            Err(_) => continue,
        };

        if ft.is_symlink() {
            continue;
        }

        if ft.is_dir() {
            Box::pin(collect_urls_async(&path, canonical_root, out)).await;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        match parse_url_from_sidecar_file(&path).await {
            Some(url) => out.push(url),
            None => {}
        }
    }
}

// ── Sidecar parsing helpers ───────────────────────────────────────────────────

/// Parse a sidecar file and return `(url, title)`.
///
/// Uses the typed `Sidecar` / legacy-migration path; falls back to `(None, None)`
/// on missing or corrupt sidecars.
async fn parse_sidecar_for_doc(json_path: &Path) -> (Option<String>, Option<String>) {
    let contents = match tokio::fs::read_to_string(json_path).await {
        Ok(s) => s,
        Err(_) => return (None, None),
    };
    let mtime = tokio::fs::metadata(json_path)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .map(DateTime::from)
        .unwrap_or_else(chrono::Utc::now);

    match tokio::task::spawn_blocking(move || parse_sidecar_or_migrate(&contents, mtime)).await {
        Ok(Ok(sidecar)) => {
            let url = if sidecar.url.is_empty() {
                sidecar.current.metadata.url.clone()
            } else {
                Some(sidecar.url)
            };
            let title = sidecar.current.metadata.title.clone();
            (url, title)
        }
        _ => (None, None),
    }
}

/// Parse only the URL from a sidecar file (used by URL enumeration for refresh).
async fn parse_url_from_sidecar_file(json_path: &Path) -> Option<String> {
    let (url, _title) = parse_sidecar_for_doc(json_path).await;
    url
}

// ── Sync helpers (called from async context via blocking) ─────────────────────

/// Count `.md` files under `dir` recursively (synchronous, used for domain listing).
fn count_md_files_sync(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                count_md_files_sync(&p)
            } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
                1
            } else {
                0
            }
        })
        .sum()
}

/// Reconstruct a URL from its store path when the sidecar is missing or corrupt.
///
/// This is a best-effort fallback: it reverses the sanitized path back to a
/// plausible `https://` URL.
pub(super) fn reconstruct_url_from_store_path(path: &Path, store_root: &Path) -> Option<String> {
    let rel = path.strip_prefix(store_root).ok()?;
    let mut components = rel.components();
    let domain = components.next()?.as_os_str().to_str()?.replace('_', ".");
    let stem = rel.with_extension("");
    let mut segments = stem
        .components()
        .skip(1)
        .filter_map(|part| part.as_os_str().to_str())
        .collect::<Vec<_>>();
    if segments.last().copied() == Some("index") {
        segments.pop();
    }
    let mut url = format!("https://{domain}");
    if !segments.is_empty() {
        url.push('/');
        url.push_str(&segments.join("/"));
    }
    Some(url)
}
