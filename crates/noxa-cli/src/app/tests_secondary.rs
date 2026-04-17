use super::*;

mod enum_deserialize_tests {
    use super::*;

    #[test]
    fn test_output_format_deserialize() {
        let f: OutputFormat = serde_json::from_str("\"llm\"").unwrap();
        assert!(matches!(f, OutputFormat::Llm));
        let f: OutputFormat = serde_json::from_str("\"markdown\"").unwrap();
        assert!(matches!(f, OutputFormat::Markdown));
    }

    #[test]
    fn test_browser_deserialize() {
        let b: Browser = serde_json::from_str("\"firefox\"").unwrap();
        assert!(matches!(b, Browser::Firefox));
    }

    #[test]
    fn test_pdf_mode_deserialize() {
        let p: PdfModeArg = serde_json::from_str("\"fast\"").unwrap();
        assert!(matches!(p, PdfModeArg::Fast));
    }

    // --- validate_url tests ---

    #[tokio::test]
    async fn validate_rejects_loopback() {
        assert!(validate_url("http://127.0.0.1/secret").await.is_err());
        assert!(validate_url("http://127.0.0.1:8080/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_localhost() {
        assert!(validate_url("http://localhost/secret").await.is_err());
        assert!(validate_url("http://localhost:8080/secret").await.is_err());
        assert!(validate_url("http://foo.localhost/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_rfc1918() {
        assert!(validate_url("http://10.0.0.1/").await.is_err());
        assert!(validate_url("http://172.16.0.1/").await.is_err());
        assert!(validate_url("http://172.31.255.255/").await.is_err());
        assert!(validate_url("http://192.168.1.1/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_link_local() {
        assert!(
            validate_url("http://169.254.169.254/latest/meta-data/")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn validate_rejects_tailscale() {
        assert!(validate_url("http://100.100.1.1/").await.is_err());
        assert!(validate_url("http://100.127.255.255/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_loopback() {
        assert!(validate_url("http://[::1]/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_link_local() {
        assert!(validate_url("http://[fe80::1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_ula() {
        assert!(validate_url("http://[fd00::1]/").await.is_err());
        assert!(validate_url("http://[fc00::1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv4_mapped_ipv6() {
        assert!(validate_url("http://[::ffff:127.0.0.1]/").await.is_err());
        assert!(
            validate_url("http://[::ffff:169.254.169.254]/latest/meta-data/")
                .await
                .is_err()
        );
        assert!(validate_url("http://[::ffff:10.0.0.1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_accepts_public_ip() {
        assert!(validate_url("http://8.8.8.8/").await.is_ok());
        assert!(validate_url("http://1.1.1.1/").await.is_ok());
    }

    // --- write_to_file traversal tests ---

    #[test]
    fn test_write_to_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        assert!(write_to_file(path, "../escape.md", "x").is_err());
        assert!(write_to_file(path, "..\\windows\\evil", "x").is_err());
        assert!(write_to_file(path, "foo\0bar", "x").is_err());
        // Absolute-path rejection is platform-specific: /etc/passwd is only
        // absolute on Unix; on Windows Path::is_absolute() requires a drive letter.
        #[cfg(unix)]
        assert!(write_to_file(path, "/etc/passwd", "x").is_err());
        #[cfg(windows)]
        assert!(write_to_file(path, "C:\\Windows\\System32\\drivers\\etc\\hosts", "x").is_err());
    }

    #[test]
    fn test_write_to_file_allows_nested() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        assert!(write_to_file(path, "sub/file.md", "hello").is_ok());
        assert!(path.join("sub/file.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_write_to_file_rejects_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let outside = tempfile::tempdir().unwrap();
        // Create a symlink inside `dir` pointing outside.
        let link = path.join("link");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        // Attempting to write through the symlink should be rejected.
        assert!(write_to_file(path, "link/escape.md", "x").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_write_to_file_rejects_leaf_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("secret.txt");
        // Create a leaf symlink inside `dir` pointing to a file outside.
        let link = path.join("output.md");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        // Writing to the leaf symlink should be rejected.
        assert!(write_to_file(path, "output.md", "x").is_err());
    }

    #[test]
    fn content_store_root_uses_explicit_output_dir() {
        let path = std::path::PathBuf::from("custom-output");
        assert_eq!(
            content_store_root(Some(path.as_path())),
            path.join(".noxa/content")
        );
    }

    #[test]
    fn content_store_root_defaults_to_noxa_content() {
        let result = content_store_root(None);
        // The default path should end with .noxa/content regardless of home dir
        assert!(
            result.ends_with(".noxa/content"),
            "expected path ending with .noxa/content, got: {}",
            result.display()
        );
    }
}
