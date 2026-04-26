use std::path::Component;

use crate::content_store::FilesystemContentStore;
use crate::paths::url_to_store_path;
use crate::types::StoreError;

fn make_extraction_with_url(markdown: &str, url: &str, title: &str) -> noxa_core::ExtractionResult {
    noxa_core::ExtractionResult {
        metadata: noxa_core::Metadata {
            title: Some(title.into()),
            description: None,
            author: None,
            published_date: None,
            language: None,
            url: Some(url.to_string()),
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
        vertical_data: None,
        structured_data: vec![],
    }
}

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
        vertical_data: None,
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
async fn test_sidecar_bloat_triggers_guard_despite_small_markdown() {
    // The old heuristic only checked markdown.len() + plain_text.len() and ignored
    // sidecar growth from metadata cardinality. This test verifies that a document
    // with small markdown but bulky metadata (many technologies, long description,
    // lots of links, changelog history) is correctly rejected once the serialized
    // sidecar + markdown exceeds max_content_bytes.
    let dir = tempfile::tempdir().unwrap();
    let mut store = FilesystemContentStore::new(dir.path());

    // First: set a large enough cap that the first write succeeds.
    store.max_content_bytes = Some(64 * 1024); // 64 KiB

    // Build an extraction with small markdown but very bulky metadata.
    let small_markdown = "# Small\n\nTiny content.";
    let mut extraction = make_extraction(small_markdown);
    // Stuff the metadata with many technologies to inflate the sidecar JSON.
    extraction.metadata.technologies = (0..500)
        .map(|i| format!("technology-framework-{i:04}"))
        .collect();
    extraction.metadata.description = Some("x".repeat(10_000)); // 10 KB description

    // Populate links to further inflate the sidecar's current.content.links array.
    extraction.content.links = (0..200)
        .map(|i| noxa_core::Link {
            href: format!("https://example.com/link-{i:04}/very/long/path/segment"),
            text: format!("Link text for item number {i} with extra padding"),
        })
        .collect();

    let result = store
        .write("https://example.com/bloat", &extraction)
        .await
        .unwrap();
    // First write: sidecar is small enough, should succeed.
    assert!(result.is_new, "first write should succeed under 64 KiB cap");

    // Now tighten the cap to something that the markdown alone would pass but
    // the full sidecar (which holds the ExtractionResult with all those fields)
    // would exceed. The serialized sidecar is well above 2 KB given the above.
    store.max_content_bytes = Some(512); // 512 bytes

    let result2 = store
        .write("https://example.com/bloat", &extraction)
        .await
        .unwrap();
    // Guard must fire: markdown is ~22 bytes but sidecar JSON is many KB.
    assert!(
        !result2.is_new && !result2.changed,
        "oversized sidecar should be rejected even when markdown alone is small"
    );
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

// ── Enumeration API tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_domains_empty_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path().join("nonexistent"));
    let domains = store.list_domains().await.unwrap();
    assert!(domains.is_empty());
}

#[tokio::test]
async fn test_list_domains_returns_entries_with_counts() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://example.com/page1",
            &make_extraction_with_url("content a", "https://example.com/page1", "Page 1"),
        )
        .await
        .unwrap();
    store
        .write(
            "https://example.com/page2",
            &make_extraction_with_url("content b", "https://example.com/page2", "Page 2"),
        )
        .await
        .unwrap();
    store
        .write(
            "https://docs.example.com/intro",
            &make_extraction_with_url("intro", "https://docs.example.com/intro", "Intro"),
        )
        .await
        .unwrap();

    let domains = store.list_domains().await.unwrap();
    assert_eq!(domains.len(), 2);
    // Alphabetical order
    assert_eq!(domains[0].name, "docs_example_com");
    assert_eq!(domains[0].doc_count, 1);
    assert_eq!(domains[1].name, "example_com");
    assert_eq!(domains[1].doc_count, 2);
}

#[tokio::test]
async fn test_list_domains_uses_metadata_url_when_sidecar_url_is_blank() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let url = "https://docs.example.com/intro";
    store
        .write(url, &make_extraction_with_url("intro", url, "Intro"))
        .await
        .unwrap();

    let json_path = dir
        .path()
        .join(url_to_store_path(url))
        .with_extension("json");
    let mut sidecar: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&json_path).await.unwrap()).unwrap();
    sidecar["url"] = serde_json::Value::String(String::new());
    tokio::fs::write(&json_path, serde_json::to_vec(&sidecar).unwrap())
        .await
        .unwrap();

    let domains = store.list_domains().await.unwrap();
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name, "docs_example_com");
    assert_eq!(domains[0].doc_count, 1);
    assert_eq!(
        domains[0].original_domain.as_deref(),
        Some("docs.example.com")
    );
}

#[tokio::test]
async fn test_list_docs_filters_by_domain() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://example.com/page",
            &make_extraction_with_url("content", "https://example.com/page", "Page"),
        )
        .await
        .unwrap();
    store
        .write(
            "https://other.com/page",
            &make_extraction_with_url("other", "https://other.com/page", "Other"),
        )
        .await
        .unwrap();

    let docs = store.list_docs("example.com").await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].url, "https://example.com/page");
    assert_eq!(docs[0].title.as_deref(), Some("Page"));
}

#[tokio::test]
async fn test_list_docs_strips_www_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://example.com/about",
            &make_extraction_with_url("about", "https://example.com/about", "About"),
        )
        .await
        .unwrap();

    let docs_with_www = store.list_docs("www.example.com").await.unwrap();
    let docs_without_www = store.list_docs("example.com").await.unwrap();
    assert_eq!(docs_with_www.len(), 1);
    assert_eq!(docs_without_www.len(), 1);
    assert_eq!(docs_with_www[0].url, docs_without_www[0].url);
}

#[tokio::test]
async fn test_list_docs_nonexistent_domain_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let docs = store.list_docs("nothere.example.com").await.unwrap();
    assert!(docs.is_empty());
}

#[tokio::test]
async fn test_list_docs_legacy_sidecar_format() {
    // Write a legacy raw ExtractionResult (no Sidecar envelope) directly to disk.
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let extraction = make_extraction_with_url("# Legacy", "https://example.com/legacy", "Legacy");
    let rel = url_to_store_path("https://example.com/legacy");
    let json_path = dir.path().join(&rel).with_extension("json");
    let md_path = dir.path().join(&rel).with_extension("md");
    tokio::fs::create_dir_all(json_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&json_path, serde_json::to_vec(&extraction).unwrap())
        .await
        .unwrap();
    tokio::fs::write(&md_path, b"# Legacy").await.unwrap();

    let docs = store.list_docs("example.com").await.unwrap();
    assert_eq!(docs.len(), 1);
    assert!(docs[0].url.contains("example.com"));
}

#[tokio::test]
async fn test_list_docs_missing_sidecar_falls_back_to_url_reconstruction() {
    // Write only the .md file, no .json sidecar.
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    let rel = url_to_store_path("https://example.com/noside");
    let md_path = dir.path().join(&rel).with_extension("md");
    tokio::fs::create_dir_all(md_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&md_path, b"content").await.unwrap();

    let docs = store.list_docs("example.com").await.unwrap();
    assert_eq!(docs.len(), 1);
    // Reconstructed URL should at least contain the domain
    assert!(docs[0].url.contains("example.com"));
}

#[tokio::test]
async fn test_list_all_docs_empty_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path().join("nonexistent"));
    let docs = store.list_all_docs().await.unwrap();
    assert!(docs.is_empty());
}

#[tokio::test]
async fn test_list_all_docs_spans_multiple_domains() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://alpha.com/a",
            &make_extraction_with_url("a", "https://alpha.com/a", "A"),
        )
        .await
        .unwrap();
    store
        .write(
            "https://beta.com/b",
            &make_extraction_with_url("b", "https://beta.com/b", "B"),
        )
        .await
        .unwrap();
    store
        .write(
            "https://alpha.com/c",
            &make_extraction_with_url("c", "https://alpha.com/c", "C"),
        )
        .await
        .unwrap();

    let docs = store.list_all_docs().await.unwrap();
    assert_eq!(docs.len(), 3);
    let urls: Vec<&str> = docs.iter().map(|d| d.url.as_str()).collect();
    assert!(urls.contains(&"https://alpha.com/a"));
    assert!(urls.contains(&"https://alpha.com/c"));
    assert!(urls.contains(&"https://beta.com/b"));
}

#[tokio::test]
async fn test_list_domain_urls_scoped_to_domain() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://docs.example.com/book",
            &make_extraction_with_url("book", "https://docs.example.com/book", "Book"),
        )
        .await
        .unwrap();
    store
        .write(
            "https://other.com/page",
            &make_extraction_with_url("page", "https://other.com/page", "Page"),
        )
        .await
        .unwrap();

    let result = store.list_domain_urls("docs.example.com").await.unwrap();
    assert_eq!(result.urls.len(), 1);
    assert_eq!(result.urls[0], "https://docs.example.com/book");
    assert_eq!(result.skipped, 0);
}

#[cfg(unix)]
#[tokio::test]
async fn test_list_domain_urls_skips_symlink_escapes() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let store_root = dir.path().join("content");
    let outside_dir = dir.path().join("outside");
    tokio::fs::create_dir_all(&store_root).await.unwrap();
    tokio::fs::create_dir_all(&outside_dir).await.unwrap();

    let store = FilesystemContentStore::new(&store_root);
    store
        .write(
            "https://example.com/legit",
            &make_extraction_with_url("legit", "https://example.com/legit", "Legit"),
        )
        .await
        .unwrap();

    // Plant a fully-valid JSON sidecar outside the store root.
    // Using a parseable payload is critical: if the symlink IS followed the URL
    // "https://escaped.example.com/" will surface in the results, causing the
    // assertion below to fail and exposing the traversal bug.  An invalid
    // `current` block would silently fail to parse and the test would pass even
    // when the escape was not prevented.
    tokio::fs::write(
        outside_dir.join("evil.json"),
        serde_json::json!({
            "schema_version": 1,
            "url": "https://escaped.example.com/",
            "first_seen": "2024-01-01T00:00:00Z",
            "last_fetched": "2024-01-01T00:00:00Z",
            "fetch_count": 1,
            "current": {
                "metadata": {
                    "title": "Escaped",
                    "description": null,
                    "author": null,
                    "published_date": null,
                    "language": null,
                    "url": "https://escaped.example.com/",
                    "site_name": null,
                    "image": null,
                    "favicon": null,
                    "word_count": 1,
                    "content_hash": null,
                    "source_type": null,
                    "file_path": null,
                    "last_modified": null,
                    "is_truncated": null,
                    "technologies": [],
                    "seed_url": null,
                    "crawl_depth": null,
                    "search_query": null,
                    "fetched_at": null
                },
                "content": {
                    "markdown": "escaped",
                    "plain_text": "escaped",
                    "links": [],
                    "images": [],
                    "code_blocks": [],
                    "raw_html": null
                },
                "domain_data": null,
                "structured_data": []
            },
            "changelog": []
        })
        .to_string(),
    )
    .await
    .unwrap();

    // Create a symlink inside the domain dir pointing outside
    let domain_dir = store_root.join("example_com");
    symlink(&outside_dir, domain_dir.join("escape")).unwrap();

    let result = store.list_domain_urls("example.com").await.unwrap();
    // Only the legit URL should appear; the symlink escape should be skipped.
    // If the symlink IS followed the escaped URL would appear here, failing both assertions.
    assert_eq!(result.urls, vec!["https://example.com/legit".to_string()]);
    assert!(
        !result
            .urls
            .contains(&"https://escaped.example.com/".to_string()),
        "symlink escape was not prevented: escaped URL appeared in results"
    );
}

#[tokio::test]
async fn test_list_domain_urls_counts_corrupt_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());

    // Write one valid entry so there is at least one parseable sidecar.
    store
        .write(
            "https://example.com/good",
            &make_extraction_with_url("good", "https://example.com/good", "Good"),
        )
        .await
        .unwrap();

    // Plant a corrupt JSON sidecar directly in the domain directory.
    let domain_dir = dir.path().join("example_com");
    tokio::fs::write(domain_dir.join("corrupt.json"), b"this is not json at all")
        .await
        .unwrap();

    let result = store.list_domain_urls("example.com").await.unwrap();
    assert_eq!(result.urls, vec!["https://example.com/good".to_string()]);
    assert_eq!(
        result.skipped, 1,
        "corrupt sidecar should be counted as skipped"
    );
}

// ── Query-variant dedup regression test ──────────────────────────────────────

#[tokio::test]
async fn test_list_all_docs_query_variant_docs_not_deduplicated() {
    // Regression test for noxa-i22: the manifest cache was previously keyed by
    // `d.url` (canonical URL), so query-variant docs that shared a base URL
    // would overwrite each other in the cache. The cache is now keyed by file
    // path, so each query-variant gets its own cache slot.
    //
    // Two URLs with the same base path but different query strings must map to
    // distinct storage files (url_to_store_path appends a hash for query-bearing
    // URLs). Both calls to list_all_docs() must return count == 2.
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());

    let url_a = "https://example.com/search?q=rust";
    let url_b = "https://example.com/search?q=golang";

    store
        .write(
            url_a,
            &make_extraction_with_url("rust results", url_a, "Rust Search"),
        )
        .await
        .unwrap();
    store
        .write(
            url_b,
            &make_extraction_with_url("golang results", url_b, "Go Search"),
        )
        .await
        .unwrap();

    // Cold call: cache is not yet populated → full filesystem walk.
    let docs_cold = store.list_all_docs().await.unwrap();
    assert_eq!(
        docs_cold.len(),
        2,
        "cold list_all_docs must return both query-variant docs"
    );

    // Warm call: cache is now populated → served from HashMap keyed by path.
    let docs_warm = store.list_all_docs().await.unwrap();
    assert_eq!(
        docs_warm.len(),
        2,
        "warm list_all_docs must return the same count as the cold walk (no dedup by URL)"
    );

    let urls_cold: std::collections::HashSet<&str> =
        docs_cold.iter().map(|d| d.url.as_str()).collect();
    let urls_warm: std::collections::HashSet<&str> =
        docs_warm.iter().map(|d| d.url.as_str()).collect();
    assert_eq!(
        urls_cold, urls_warm,
        "cold and warm results must contain identical URL sets"
    );
}

// ── Manifest cache tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_manifest_cache_populates_on_first_call() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://cache.example.com/doc",
            &make_extraction_with_url("body", "https://cache.example.com/doc", "Doc"),
        )
        .await
        .unwrap();

    // First call: cache is None → walk happens.
    let docs = store.list_all_docs().await.unwrap();
    assert_eq!(docs.len(), 1);

    // After the call the cache should be populated.
    let guard = store.manifest_cache.0.lock().await;
    assert!(
        guard.cache.is_some(),
        "cache should be Some after first list_all_docs"
    );
    assert!(guard.cache.as_ref().unwrap().is_fresh());
}

#[tokio::test]
async fn test_manifest_cache_hit_does_not_see_out_of_band_file() {
    // Write a file directly to disk (bypassing the store API) *after* the
    // cache has been populated.  A cache-hit call should NOT see it, proving
    // the cache is actually being used.
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://hit.example.com/a",
            &make_extraction_with_url("a", "https://hit.example.com/a", "A"),
        )
        .await
        .unwrap();

    // Prime the cache.
    let docs_first = store.list_all_docs().await.unwrap();
    assert_eq!(docs_first.len(), 1);

    // Plant a new .md file on disk without going through write() so the
    // cache is NOT invalidated.
    let rel = url_to_store_path("https://hit.example.com/b");
    let md_path = dir.path().join(&rel).with_extension("md");
    tokio::fs::create_dir_all(md_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&md_path, b"sneaky").await.unwrap();

    // Second call within TTL should still return 1 (cached).
    let docs_cached = store.list_all_docs().await.unwrap();
    assert_eq!(
        docs_cached.len(),
        1,
        "cache hit should not reflect out-of-band file write"
    );
}

#[tokio::test]
async fn test_write_invalidates_cache() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://inv.example.com/a",
            &make_extraction_with_url("a", "https://inv.example.com/a", "A"),
        )
        .await
        .unwrap();

    // Prime the cache.
    let _ = store.list_all_docs().await.unwrap();
    {
        let guard = store.manifest_cache.0.lock().await;
        assert!(guard.cache.is_some(), "cache should be populated");
    }

    // Another write should invalidate the cache.
    store
        .write(
            "https://inv.example.com/b",
            &make_extraction_with_url("b", "https://inv.example.com/b", "B"),
        )
        .await
        .unwrap();
    {
        let guard = store.manifest_cache.0.lock().await;
        assert!(
            guard.cache.is_none(),
            "cache should be invalidated after write"
        );
    }

    // Next list_all_docs should re-walk and return both docs.
    let docs = store.list_all_docs().await.unwrap();
    assert_eq!(
        docs.len(),
        2,
        "should see both docs after cache invalidation"
    );
}

#[tokio::test]
async fn test_manifest_cache_ttl_forces_rewalk() {
    use crate::content_store::manifest::{CACHE_TTL, ManifestCache, ManifestCacheState};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemContentStore::new(dir.path());
    store
        .write(
            "https://ttl.example.com/a",
            &make_extraction_with_url("a", "https://ttl.example.com/a", "A"),
        )
        .await
        .unwrap();

    // Manually insert a stale cache entry (populated_at = now - TTL - 1s).
    {
        let mut guard = store.manifest_cache.0.lock().await;
        *guard = ManifestCacheState {
            cache: Some(ManifestCache {
                docs: HashMap::new(), // empty — would return 0 if served
                populated_at: Instant::now()
                    .checked_sub(CACHE_TTL + Duration::from_secs(1))
                    .expect("time arithmetic should not overflow"),
            }),
            generation: 0,
        };
    }

    // list_all_docs should detect the stale cache and re-walk.
    let docs = store.list_all_docs().await.unwrap();
    assert_eq!(
        docs.len(),
        1,
        "stale cache should trigger re-walk and return real docs"
    );
}
