//! Canonical content store for search snapshots.
use std::path::{Component, PathBuf};

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

pub struct StoreResult {
    pub md_path: PathBuf,
    pub json_path: PathBuf,
    pub is_new: bool,
    pub changed: bool,
    pub word_count_delta: i64,
}

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
        Ok(base)
    }

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
        }

        // Size guard — skip oversized documents rather than filling disk.
        let estimated =
            extraction.content.markdown.len() + extraction.content.plain_text.len();
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
                    let parsed = tokio::task::spawn_blocking(move || {
                        serde_json::from_str::<noxa_core::ExtractionResult>(&contents).ok()
                    })
                    .await
                    .unwrap_or(None);
                    parsed
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

        // Strip query params from metadata.url before persisting — prevents
        // leaking auth tokens / API keys that may appear in query strings.
        let mut to_store = extraction.clone();
        if let Some(ref url_str) = to_store.metadata.url {
            if let Ok(mut u) = url::Url::parse(url_str) {
                u.set_query(None);
                to_store.metadata.url = Some(u.to_string());
            }
        }

        let markdown_bytes = to_store.content.markdown.as_bytes().to_vec();
        let json_bytes = tokio::task::spawn_blocking(move || {
            serde_json::to_vec(&to_store).map_err(|e| format!("store: serialize: {e}"))
        })
        .await
        .map_err(|e| format!("store: serialize join: {e}"))??;

        // Atomic writes: write to .tmp then rename (POSIX rename is atomic on
        // same filesystem — eliminates the corruption window between two writes).
        let tmp_md = md_path.with_extension("md.tmp");
        let tmp_json = json_path.with_extension("json.tmp");

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

        Ok(StoreResult {
            md_path,
            json_path,
            is_new,
            changed,
            word_count_delta,
        })
    }
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

    // --- new tests for read(), Clone, atomic writes, query-param stripping ---

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
        // resolve_path returns Err for path escapes; read() maps that to Ok(None)
        let result = store
            .read("https://evil.com/../../etc/passwd")
            .await
            .unwrap();
        // url_to_store_path already sanitizes this to a safe path, so Ok(None) expected
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
}
