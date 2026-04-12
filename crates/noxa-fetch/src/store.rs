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

pub struct ContentStore {
    root: PathBuf,
}

impl ContentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn open() -> Self {
        let root = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".noxa")
            .join("content");
        Self { root }
    }

    pub async fn write(
        &self,
        url: &str,
        extraction: &noxa_core::ExtractionResult,
    ) -> Result<StoreResult, String> {
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

        let md_path = base.with_extension("md");
        let json_path = base.with_extension("json");

        if let Some(parent) = md_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("store: create_dir: {e}"))?;
        }

        let previous: Option<noxa_core::ExtractionResult> =
            match tokio::fs::read_to_string(&json_path).await {
                Ok(contents) => serde_json::from_str(&contents).ok(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(format!("store: read previous: {e}")),
            };

        let is_new = previous.is_none();
        let mut changed = false;
        let mut word_count_delta = 0_i64;

        if let Some(ref prev) = previous {
            let diff = noxa_core::diff::diff(prev, extraction);
            if diff.status != noxa_core::ChangeStatus::Same {
                changed = true;
                word_count_delta = diff.word_count_delta;
            }
        }

        tokio::fs::write(&md_path, extraction.content.markdown.as_bytes())
            .await
            .map_err(|e| format!("store: write md: {e}"))?;

        let json =
            serde_json::to_string(extraction).map_err(|e| format!("store: serialize: {e}"))?;
        tokio::fs::write(&json_path, json.as_bytes())
            .await
            .map_err(|e| format!("store: write json: {e}"))?;

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
}
