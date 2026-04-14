//! Per-URL content store: manages `.md` and `.json` sidecar files under a configurable root.
use std::path::{Component, PathBuf};

use crate::paths::{content_store_root, url_to_store_path};
use crate::types::StoreResult;

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
    /// Maximum combined byte size of markdown + plain_text before a document is
    /// skipped (not written). Default: 2 MiB. `None` disables the guard.
    pub max_content_bytes: Option<usize>,
}

impl FilesystemContentStore {
    /// Create a store at an explicit root path.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            max_content_bytes: Some(2 * 1024 * 1024),
        }
    }

    /// Create a store at the default location (`~/.noxa/content/`).
    ///
    /// Returns `Err` if the home directory cannot be determined.
    pub fn open() -> Result<Self, String> {
        let root = content_store_root(None)?;
        Ok(Self {
            root,
            max_content_bytes: Some(2 * 1024 * 1024),
        })
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
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
        // that the final path still resides under the canonical root.
        let canonical_root = std::fs::canonicalize(&self.root).map_err(|e| {
            format!("store: cannot canonicalize root {}: {e}", self.root.display())
        })?;
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

        Ok(base)
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
                    serde_json::from_str::<noxa_core::ExtractionResult>(&contents)
                })
                .await
                .map_err(|e| format!("store: read join: {e}"))?;
                result
                    .map(Some)
                    .map_err(|e| format!("store: deserialize: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("store: read: {e}")),
        }
    }

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
                });
            }
        }

        let previous: Option<noxa_core::ExtractionResult> =
            match tokio::fs::read_to_string(&json_path).await {
                Ok(contents) => {
                    tokio::task::spawn_blocking(move || {
                        serde_json::from_str::<noxa_core::ExtractionResult>(&contents).ok()
                    })
                    .await
                    .unwrap_or(None)
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(format!("store: read previous: {e}")),
            };

        let is_new = previous.is_none();
        let changed;
        let word_count_delta;

        if let Some(ref prev) = previous {
            changed = prev.content.markdown != extraction.content.markdown;
            word_count_delta =
                extraction.metadata.word_count as i64 - prev.metadata.word_count as i64;
        } else {
            changed = false;
            word_count_delta = 0;
        }

        // Security stripping — applied before any serialization.
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

        let markdown_bytes = to_store.content.markdown.as_bytes().to_vec();
        let json_bytes = tokio::task::spawn_blocking(move || {
            serde_json::to_vec(&to_store).map_err(|e| format!("store: serialize: {e}"))
        })
        .await
        .map_err(|e| format!("store: serialize join: {e}"))??;

        // Atomic writes: write to .tmp then rename.
        // Random suffix prevents races between concurrent writes for the same URL.
        let rand_suffix = {
            use rand::Rng;
            format!("{:016x}", rand::thread_rng().r#gen::<u64>())
        };
        let tmp_md = md_path.with_extension(format!("md.{rand_suffix}.tmp"));
        let tmp_json = json_path.with_extension(format!("json.{rand_suffix}.tmp"));

        tokio::fs::write(&tmp_md, &markdown_bytes)
            .await
            .map_err(|e| format!("store: write md.tmp: {e}"))?;
        tokio::fs::write(&tmp_json, &json_bytes)
            .await
            .map_err(|e| format!("store: write json.tmp: {e}"))?;
        tokio::fs::rename(&tmp_md, &md_path)
            .await
            .map_err(|e| format!("store: rename md: {e}"))?;
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
        })
    }
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
}
