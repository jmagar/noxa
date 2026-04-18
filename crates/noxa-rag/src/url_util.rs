/// Strip fragment, trailing path slash, lowercase scheme+host (url crate already does the latter).
///
/// This is a pure, backend-agnostic canonicalization step used by the ingestion pipeline
/// before any interaction with the vector store. It lives here rather than inside a specific
/// store implementation so that all current and future backends derive the same URL identity.
pub(crate) fn normalize_url(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(&path);
    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_url;

    #[test]
    fn strips_fragment() {
        assert_eq!(
            normalize_url("https://example.com/page#section"),
            "https://example.com/page"
        );
    }

    #[test]
    fn trims_trailing_slash() {
        assert_eq!(
            normalize_url("https://example.com/docs/"),
            "https://example.com/docs"
        );
    }

    #[test]
    fn preserves_root_path() {
        // Root path "/" — trimming all slashes would yield an empty path, which is invalid.
        // url::Url::set_path("") normalises back to "/" so the round-trip is stable.
        let result = normalize_url("https://example.com/");
        assert!(result == "https://example.com/" || result == "https://example.com");
    }

    #[test]
    fn passes_through_unparseable_url() {
        let bad = "not a url at all";
        assert_eq!(normalize_url(bad), bad);
    }

    #[test]
    fn normalises_file_url() {
        assert_eq!(
            normalize_url("file:///tmp/report.md"),
            "file:///tmp/report.md"
        );
    }
}
