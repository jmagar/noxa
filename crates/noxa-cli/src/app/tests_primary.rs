use super::*;

mod tests {
    use super::*;

    fn sample_extraction(url: &str, markdown: &str) -> ExtractionResult {
        ExtractionResult {
            metadata: Metadata {
                title: Some("Example".to_string()),
                description: None,
                author: None,
                published_date: None,
                language: Some("en".to_string()),
                url: Some(url.to_string()),
                site_name: Some("Example".to_string()),
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
                links: Vec::new(),
                images: Vec::new(),
                code_blocks: Vec::new(),
                raw_html: None,
            },
            domain_data: None,
            structured_data: Vec::new(),
        }
    }

    #[test]
    fn url_to_filename_root() {
        assert_eq!(
            url_to_filename("https://example.com/", &OutputFormat::Markdown),
            "example_com/index.md"
        );
        assert_eq!(
            url_to_filename("https://example.com", &OutputFormat::Markdown),
            "example_com/index.md"
        );
    }

    #[test]
    fn url_to_filename_path() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Markdown),
            "example_com/docs/api.md"
        );
    }

    #[test]
    fn url_to_filename_trailing_slash() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api/", &OutputFormat::Markdown),
            "example_com/docs/api.md"
        );
    }

    #[test]
    fn url_to_filename_nested_path() {
        assert_eq!(
            url_to_filename("https://example.com/blog/my-post", &OutputFormat::Markdown),
            "example_com/blog/my-post.md"
        );
    }

    #[test]
    fn url_to_filename_query_params() {
        assert_eq!(
            url_to_filename("https://example.com/p?id=123", &OutputFormat::Markdown),
            "example_com/p_id_123.md"
        );
    }

    #[test]
    fn url_to_filename_json_format() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Json),
            "example_com/docs/api.json"
        );
    }

    #[test]
    fn url_to_filename_text_format() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Text),
            "example_com/docs/api.txt"
        );
    }

    #[test]
    fn url_to_filename_llm_format() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Llm),
            "example_com/docs/api.md"
        );
    }

    #[test]
    fn url_to_filename_html_format() {
        assert_eq!(
            url_to_filename("https://example.com/docs/api", &OutputFormat::Html),
            "example_com/docs/api.html"
        );
    }

    #[test]
    fn url_to_filename_special_chars() {
        // Spaces and special chars get replaced with underscores
        assert_eq!(
            url_to_filename(
                "https://example.com/path%20with%20spaces",
                &OutputFormat::Markdown
            ),
            "example_com/path_20with_20spaces.md"
        );
    }

    #[test]
    fn write_to_file_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().to_path_buf();
        write_to_file(&dir, "nested/deep/file.md", "hello").unwrap();
        let content = std::fs::read_to_string(dir.join("nested/deep/file.md")).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_write_to_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().to_path_buf();
        std::fs::create_dir_all(&dir).unwrap();
        assert!(write_to_file(&dir, "../escaped.md", "x").is_err());
        assert!(write_to_file(&dir, "/abs.md", "x").is_err());
        assert!(write_to_file(&dir, "..\\windows.md", "x").is_err());
        assert!(write_to_file(&dir, "null\0byte.md", "x").is_err());
    }

    #[test]
    fn test_write_to_file_allows_nested() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().to_path_buf();
        std::fs::create_dir_all(&dir).unwrap();
        assert!(write_to_file(&dir, "sub/file.md", "hello").is_ok());
    }

    #[test]
    fn test_content_store_root_no_output_dir() {
        let root = content_store_root(None);
        assert!(root.ends_with(".noxa/content"));
    }

    #[test]
    fn test_content_store_root_with_output_dir() {
        let dir = std::path::PathBuf::from("/tmp/mybase");
        assert_eq!(content_store_root(Some(&dir)), dir.join(".noxa/content"));
    }

    #[test]
    fn test_default_search_dir_under_noxa() {
        let d = default_search_dir();
        assert!(d.to_string_lossy().contains(".noxa"));
        assert!(d.to_string_lossy().contains("search"));
    }

    #[test]
    fn test_url_to_filename_flat_for_search() {
        let raw = url_to_filename("https://example.com/blog/post", &OutputFormat::Markdown);
        let flat = raw.replace('/', "_");
        assert!(!flat.contains('/'));
        assert!(flat.ends_with(".md"));
    }

    #[tokio::test]
    async fn validate_public_url_rejects_ipv6_private_ranges() {
        assert!(validate_url("http://[fe80::1]/").await.is_err());
        assert!(validate_url("http://[fc00::1]/").await.is_err());
    }

    #[test]
    fn validate_operator_url_allows_localhost() {
        assert!(validate_operator_url("http://127.0.0.1:8080").is_ok());
        assert!(validate_operator_url("https://localhost/search").is_ok());
    }

    #[test]
    fn search_scrape_concurrency_is_clamped() {
        assert_eq!(clamp_search_scrape_concurrency(0), 1);
        assert_eq!(clamp_search_scrape_concurrency(50), 20);
        assert_eq!(clamp_search_scrape_concurrency(4), 4);
    }

    fn sample_crawl_status(phase: CrawlStatusPhase) -> CrawlStatusRecord {
        CrawlStatusRecord {
            version: 1,
            url: "https://code.claude.com".to_string(),
            pid: 4242,
            phase,
            pages_done: 7,
            pages_ok: 6,
            pages_errors: 1,
            max_pages: 20,
            last_url: Some("https://code.claude.com/docs".to_string()),
            elapsed_secs: 12.3,
            docs_dir: "/tmp/docs".to_string(),
            excluded: 2,
            total_words: 1234,
        }
    }

    #[test]
    fn refresh_flag_requires_a_domain_value() {
        let parsed = Cli::try_parse_from(["noxa", "--refresh", "docs.rust-lang.org"]).unwrap();
        assert_eq!(parsed.refresh.as_deref(), Some("docs.rust-lang.org"));

        assert!(Cli::try_parse_from(["noxa", "--refresh"]).is_err());
    }

    #[tokio::test]
    async fn collect_refresh_urls_is_domain_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("content");
        tokio::fs::create_dir_all(&store_root).await.unwrap();
        let store = FilesystemContentStore::new(&store_root);
        store
            .write(
                "https://docs.rust-lang.org/book/",
                &sample_extraction("https://docs.rust-lang.org/book/", "Rust book"),
            )
            .await
            .unwrap();
        store
            .write(
                "https://example.com/",
                &sample_extraction("https://example.com/", "Example"),
            )
            .await
            .unwrap();

        let urls = collect_refresh_urls(&store_root, "docs.rust-lang.org")
            .await
            .unwrap();
        assert_eq!(urls, vec!["https://docs.rust-lang.org/book/".to_string()]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn collect_refresh_urls_skips_symlink_escapes() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("content");
        let outside_dir = dir.path().join("outside");
        tokio::fs::create_dir_all(&store_root).await.unwrap();
        tokio::fs::create_dir_all(&outside_dir).await.unwrap();
        let store = FilesystemContentStore::new(&store_root);
        store
            .write(
                "https://docs.rust-lang.org/book/",
                &sample_extraction("https://docs.rust-lang.org/book/", "Rust book"),
            )
            .await
            .unwrap();
        let domain_dir = refresh_domain_dir(&store_root, "docs.rust-lang.org").unwrap();
        tokio::fs::write(
            outside_dir.join("evil.json"),
            serde_json::json!({
                "url": "https://attacker.example/secret",
                "current": { "metadata": { "url": "https://attacker.example/secret" } }
            })
            .to_string(),
        )
        .await
        .unwrap();

        symlink(&outside_dir, domain_dir.join("escape")).unwrap();

        let urls = collect_refresh_urls(&store_root, "docs.rust-lang.org")
            .await
            .unwrap();
        assert_eq!(urls, vec!["https://docs.rust-lang.org/book/".to_string()]);
    }

    #[test]
    fn crawl_status_lookup_normalizes_domain_scheme_and_www() {
        let home = tempfile::tempdir().unwrap();
        let expected = home
            .path()
            .join(".noxa/crawls")
            .join("code_claude_com.json");

        assert_eq!(
            crawl_status_path_for_home(home.path(), "code.claude.com"),
            expected
        );
        assert_eq!(
            crawl_status_path_for_home(home.path(), "https://code.claude.com"),
            expected
        );
        assert_eq!(
            crawl_status_path_for_home(home.path(), "https://www.code.claude.com/docs"),
            expected
        );
    }

    #[test]
    fn crawl_status_record_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let expected = sample_crawl_status(CrawlStatusPhase::Running);

        write_crawl_status_sync(&path, &expected).unwrap();

        let actual = read_crawl_status(&path).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn crawl_status_reader_accepts_legacy_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        std::fs::write(
            &path,
            serde_json::json!({
                "url": "https://code.claude.com",
                "pid": 99,
                "pages_done": 3,
                "pages_ok": 3,
                "pages_errors": 0,
                "max_pages": 12,
                "last_url": "https://code.claude.com/docs",
                "done": false,
                "elapsed_secs": 4.2,
                "docs_dir": "/tmp/docs",
                "excluded": 1,
                "total_words": 900,
            })
            .to_string(),
        )
        .unwrap();

        let actual = read_crawl_status(&path).unwrap();
        assert_eq!(actual.phase, CrawlStatusPhase::Running);
        assert_eq!(actual.pid, 99);
        assert_eq!(actual.pages_done, 3);
        assert_eq!(actual.docs_dir, "/tmp/docs");
    }

    #[test]
    fn crawl_status_classifier_covers_every_state() {
        let running = sample_crawl_status(CrawlStatusPhase::Running);
        let done = sample_crawl_status(CrawlStatusPhase::Done);

        assert_eq!(
            classify_crawl_status(None, false),
            CrawlStatusState::NeverStarted
        );
        assert_eq!(
            classify_crawl_status(Some(&running), true),
            CrawlStatusState::Running
        );
        assert_eq!(
            classify_crawl_status(Some(&running), false),
            CrawlStatusState::Stale
        );
        assert_eq!(
            classify_crawl_status(Some(&done), false),
            CrawlStatusState::Done
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_liveness_prefers_proc_when_available() {
        let dir = tempfile::tempdir().unwrap();
        let proc_root = dir.path();
        std::fs::create_dir_all(proc_root.join("777")).unwrap();

        let called = std::cell::Cell::new(false);
        let running = is_pid_running_with(777, Some(proc_root), |_pid| {
            called.set(true);
            Ok(())
        });

        assert!(running);
        assert!(!called.get());
    }

    #[cfg(unix)]
    #[test]
    fn liveness_falls_back_to_signal_probe_without_proc() {
        let running = is_pid_running_with(888, None, |pid| {
            assert_eq!(pid, 888);
            Ok(())
        });
        let stale = is_pid_running_with(999, None, |_pid| {
            Err(std::io::Error::from_raw_os_error(libc::ESRCH))
        });
        let protected = is_pid_running_with(1000, None, |_pid| {
            Err(std::io::Error::from_raw_os_error(libc::EPERM))
        });

        assert!(running);
        assert!(!stale);
        assert!(protected);
    }

    #[test]
    fn initial_background_status_write_is_running_and_log_path_is_preserved() {
        let home = tempfile::tempdir().unwrap();
        let status_path = crawl_status_path_for_home(home.path(), "https://code.claude.com");

        write_initial_crawl_status(
            &status_path,
            "https://code.claude.com",
            321,
            50,
            "/tmp/docs",
        )
        .unwrap();

        let actual = read_crawl_status(&status_path).unwrap();
        assert_eq!(actual.phase, CrawlStatusPhase::Running);
        assert_eq!(actual.pid, 321);
        assert_eq!(
            crawl_log_path_for_home(home.path(), "https://code.claude.com/docs"),
            home.path().join(".noxa/crawls").join("code_claude_com.log")
        );
    }

    #[tokio::test]
    async fn async_crawl_status_write_persists_without_blocking_helper() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let expected = sample_crawl_status(CrawlStatusPhase::Done);

        write_crawl_status_async(path.clone(), expected.clone())
            .await
            .unwrap();

        let actual = read_crawl_status(&path).unwrap();
        assert_eq!(actual, expected);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn on_change_command_writes_payload_and_exits_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("payload.json");
        let payload = r#"{"status":"changed"}"#;
        // Pass the path via env so it is never interpolated unquoted into the shell
        // command string — avoids breakage when TMPDIR contains spaces or special chars.
        std::env::set_var("_NOXA_TEST_OUTPUT", &output_path);
        let cmd = "cat > \"$_NOXA_TEST_OUTPUT\"";

        run_on_change_command(&cmd, payload, std::time::Duration::from_secs(1))
            .await
            .expect("on-change command should succeed");

        let written = tokio::fs::read_to_string(&output_path).await.unwrap();
        assert_eq!(written, payload);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn on_change_command_times_out_and_returns_promptly() {
        let start = std::time::Instant::now();
        let result =
            run_on_change_command("sleep 5", "{}", std::time::Duration::from_millis(50)).await;

        assert!(result.is_err(), "long-running child should time out");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "timeout should bound execution, elapsed={:?}",
            start.elapsed()
        );
    }
}
