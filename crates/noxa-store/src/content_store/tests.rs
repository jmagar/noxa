use std::path::Component;

use crate::content_store::FilesystemContentStore;
use crate::paths::url_to_store_path;
use crate::types::StoreError;

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
}

#[tokio::test]
async fn test_identical_content_not_changed() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let extraction = make_extraction("# Same\n\nContent.");
    store
        .write("https://example.com/same", &extraction)
        .await
        .unwrap();
    let result = store
        .write("https://example.com/same", &extraction)
        .await
        .unwrap();
    assert!(!result.is_new);
    assert!(!result.changed);
}

#[tokio::test]
async fn test_corrupted_prev_json_returns_error() {
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
    let extraction = make_extraction("content");
    let err = store
        .write("https://example.com/corrupt", &extraction)
        .await
        .expect_err("corrupt sidecar should fail closed");
    assert!(matches!(err, StoreError::CorruptSidecar { .. }));
}

#[tokio::test]
async fn test_store_path_stays_within_root() {
    let path = url_to_store_path("https://evil.com/../../../etc/passwd");
    assert!(path.to_string_lossy().starts_with("evil_com/"));
}

#[tokio::test]
async fn test_url_to_store_path_strips_parent_components() {
    let path = url_to_store_path("https://evil.com/a/../../etc/./passwd");
    assert!(!path.components().any(|c| matches!(c, Component::ParentDir)));
    assert!(!path.components().any(|c| matches!(c, Component::CurDir)));
}

#[tokio::test]
async fn test_url_to_store_path_sanitizes_ipv6_host_and_path() {
    let path = url_to_store_path("https://[fe80::1]/bad:path/segment");
    let rendered = path.to_string_lossy();
    assert!(rendered.starts_with("fe80__1/"));
    assert!(!rendered.contains(':'));
    assert!(!rendered.contains('['));
    assert!(!rendered.contains(']'));
}

#[test]
fn test_url_hash_matches_fnv1a() {
    let path = url_to_store_path("https://example.com/page?q=test");
    assert!(path.to_string_lossy().contains('_'));
}

#[tokio::test]
async fn test_read_returns_none_for_unknown_url() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let result = store
        .read("https://example.com/never-written")
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_read_returns_written_extraction() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let extraction = make_extraction("# Hello\n\nReadable content.");
    store
        .write("https://example.com/readable", &extraction)
        .await
        .unwrap();
    let read_back = store
        .read("https://example.com/readable")
        .await
        .unwrap()
        .expect("should be Some after write");
    assert_eq!(read_back.content.markdown, extraction.content.markdown);
}

#[tokio::test]
async fn test_raw_html_stripped_before_write() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let mut extraction = make_extraction("# Secret\n\nContent.");
    extraction.content.raw_html = Some("<script>alert('xss')</script>".to_string());
    store
        .write("https://example.com/xss", &extraction)
        .await
        .unwrap();
    let read_back = store
        .read("https://example.com/xss")
        .await
        .unwrap()
        .unwrap();
    assert!(read_back.content.raw_html.is_none());
}

#[tokio::test]
async fn test_metadata_url_query_params_stripped() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let mut extraction = make_extraction("# Secret\n\nContent.");
    extraction.metadata.url =
        Some("https://api.example.com/data?token=supersecret&foo=bar".to_string());
    store
        .write("https://example.com/secret-page", &extraction)
        .await
        .unwrap();
    let read_back = store
        .read("https://example.com/secret-page")
        .await
        .unwrap()
        .unwrap();
    let stored_url = read_back.metadata.url.unwrap();
    assert!(!stored_url.contains("token="));
    assert!(!stored_url.contains("supersecret"));
}

#[tokio::test]
async fn test_max_content_bytes_guard_skips_oversized() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = FilesystemContentStore::new(dir.path());
    store.max_content_bytes = Some(10);
    let extraction = make_extraction("# Big\n\nThis content is definitely more than 10 bytes.");
    let result = store
        .write("https://example.com/big", &extraction)
        .await
        .unwrap();
    assert!(!result.is_new);
    assert!(!result.changed);
}

#[tokio::test]
async fn test_sidecar_first_write_has_one_changelog_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let extraction = make_extraction("# Hello\n\nFirst content.");
    store
        .write("https://example.com/sidecar-first", &extraction)
        .await
        .unwrap();
    let sidecar = store
        .read_sidecar("https://example.com/sidecar-first")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sidecar.schema_version, 1);
    assert_eq!(sidecar.fetch_count, 1);
    assert_eq!(sidecar.changelog.len(), 1);
    assert!(sidecar.changelog[0].diff.is_none());
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
        .unwrap();
    assert_eq!(sidecar.fetch_count, 2);
    assert_eq!(sidecar.changelog.len(), 2);
    let entry = &sidecar.changelog[1];
    assert!(entry.diff.is_some());
    assert_eq!(
        entry.diff.as_ref().unwrap().status,
        noxa_core::ChangeStatus::Changed
    );
}

#[tokio::test]
async fn test_sidecar_identical_refetch_no_new_changelog_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let extraction = make_extraction("# Same\n\nContent.");
    store
        .write("https://example.com/sidecar-same", &extraction)
        .await
        .unwrap();
    store
        .write("https://example.com/sidecar-same", &extraction)
        .await
        .unwrap();
    let sidecar = store
        .read_sidecar("https://example.com/sidecar-same")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sidecar.fetch_count, 2);
    assert_eq!(sidecar.changelog.len(), 1);
}

#[tokio::test]
async fn test_sidecar_diff_field_in_store_result() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let first = make_extraction("# Hello\n\nOriginal content.");
    let first_result = store
        .write("https://example.com/sidecar-diff", &first)
        .await
        .unwrap();
    assert!(first_result.diff.is_none());
    let second = make_extraction("# Hello\n\nChanged content.");
    let second_result = store
        .write("https://example.com/sidecar-diff", &second)
        .await
        .unwrap();
    assert!(second_result.diff.is_some());
}

#[tokio::test]
async fn test_legacy_migration_on_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let extraction = make_extraction("# Legacy\n\nOld format content.");
    let rel = url_to_store_path("https://example.com/legacy");
    let json_path = dir.path().join(&rel).with_extension("json");
    tokio::fs::create_dir_all(json_path.parent().unwrap())
        .await
        .unwrap();
    let raw = serde_json::to_vec(&extraction).unwrap();
    tokio::fs::write(&json_path, &raw).await.unwrap();

    let result = store
        .read("https://example.com/legacy")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.content.markdown, extraction.content.markdown);

    let sidecar = store
        .read_sidecar("https://example.com/legacy")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sidecar.schema_version, 1);
    assert_eq!(sidecar.fetch_count, 1);
    assert_eq!(sidecar.changelog.len(), 1);
    assert!(sidecar.changelog[0].diff.is_none());
}

#[tokio::test]
async fn test_sidecar_current_matches_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let first = make_extraction("# Version 1");
    store
        .write("https://example.com/cur", &first)
        .await
        .unwrap();
    let second = make_extraction("# Version 2\n\nMore words here.");
    store
        .write("https://example.com/cur", &second)
        .await
        .unwrap();
    let read_result = store
        .read("https://example.com/cur")
        .await
        .unwrap()
        .unwrap();
    let sidecar = store
        .read_sidecar("https://example.com/cur")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        read_result.content.markdown,
        sidecar.current.content.markdown
    );
    assert_eq!(read_result.content.markdown, second.content.markdown);
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
    assert_eq!(a.root(), b.root());
    assert_eq!(a.max_content_bytes, b.max_content_bytes);
}

#[tokio::test]
async fn test_atomic_write_no_tmp_files_after_completion() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let extraction = make_extraction("# Atomic\n\nContent.");
    store
        .write("https://example.com/atomic", &extraction)
        .await
        .unwrap();

    let domain_dir = dir.path().join("example_com");
    let mut entries = tokio::fs::read_dir(&domain_dir).await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        let name = entry.file_name();
        let rendered = name.to_string_lossy();
        assert!(
            !rendered.ends_with(".tmp"),
            "tmp file left behind: {rendered}"
        );
    }
}
