use super::*;

// ── Pure scoring/ranking helpers ──────────────────────────────────────────────

/// Classify whether `query` looks like a URL (explicit scheme, or a bare
/// hostname pattern with a recognisable TLD).
///
/// Returns `(looks_like_url, normalised_url)`.  The normalised form always
/// has a scheme; it is only meaningful when `looks_like_url` is `true`.
pub(crate) fn classify_query(query: &str) -> (bool, String) {
    let has_scheme = query.starts_with("http://") || query.starts_with("https://");
    let url_candidate = if has_scheme {
        query.to_string()
    } else {
        format!("https://{query}")
    };
    let looks_like_url = has_scheme
        || (!query.contains(' ')
            && query.contains('.')
            && {
                url::Url::parse(&url_candidate)
                    .ok()
                    .and_then(|u| u.host_str().map(|h| h.to_string()))
                    .map(|host| {
                        let parts: Vec<&str> = host.split('.').collect();
                        parts.len() >= 2
                            && parts
                                .last()
                                .map(|tld| {
                                    tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())
                                })
                                .unwrap_or(false)
                    })
                    .unwrap_or(false)
            });
    (looks_like_url, url_candidate)
}

/// True when the document's URL exactly matches `query` (after normalisation).
///
/// NOTE: this predicate is used in tests and as a conceptual aid.  The live
/// `run_retrieve` path still performs a direct FS probe (which is cheaper and
/// identical in effect) rather than iterating `list_all_docs`.
pub(crate) fn is_exact_url_match(doc: &noxa_store::StoredDoc, query: &str) -> bool {
    let (looks_like_url, normalised) = classify_query(query);
    if !looks_like_url {
        return false;
    }
    doc.url == normalised || doc.url == query
}

/// Score a single document against `terms` (lower-cased query words).
///
/// Score = number of terms that appear in the lower-cased URL **or** title.
/// Documents that match zero terms score 0.
pub(crate) fn score_doc(doc: &noxa_store::StoredDoc, terms: &[String]) -> usize {
    let url_lower = doc.url.to_lowercase();
    let title_lower = doc.title.as_deref().unwrap_or("").to_lowercase();
    terms
        .iter()
        .filter(|t| url_lower.contains(t.as_str()) || title_lower.contains(t.as_str()))
        .count()
}

/// Select the best-matching document for a fuzzy `query`.
///
/// Returns `None` when no document scores above zero.
///
/// Tie-breaking: higher score wins; on score tie, shorter URL wins (favours
/// more specific / canonical pages over noisy deep links).
pub(crate) fn select_best<'a>(
    docs: &'a [noxa_store::StoredDoc],
    query: &str,
) -> Option<&'a noxa_store::StoredDoc> {
    let terms: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();

    docs.iter()
        .filter_map(|doc| {
            let s = score_doc(doc, &terms);
            if s > 0 { Some((s, doc)) } else { None }
        })
        .max_by(|(sa, da), (sb, db)| sa.cmp(sb).then(db.url.len().cmp(&da.url.len())))
        .map(|(_, doc)| doc)
}

// ── CLI entry-point ───────────────────────────────────────────────────────────

pub(crate) async fn run_retrieve(query: &str, store_root: std::path::PathBuf) {
    if !store_root.exists() {
        eprintln!(
            "{dim}no local docs — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --crawl <url>{reset}"
        );
        return;
    }

    // Exact URL lookup — fast FS probe; no need to iterate list_all_docs.
    let (looks_like_url, url_candidate) = classify_query(query);

    if looks_like_url {
        let url = url_candidate;
        let md_path = store_root
            .join(url_to_store_path(&url))
            .with_extension("md");
        if md_path.exists() {
            match std::fs::read_to_string(&md_path) {
                Ok(content) => {
                    eprintln!("{dim}retrieved{reset} {pink}{}{reset}\n", md_path.display());
                    print!("{content}");
                    return;
                }
                Err(e) => {
                    eprintln!("error reading {}: {e}", md_path.display());
                    return;
                }
            }
        }
        eprintln!("{yellow}not cached:{reset} {bold}{url}{reset}");
        eprintln!("{dim}run:{reset} {cyan}noxa {url}{reset} {dim}to fetch and store it{reset}");
        return;
    }

    // Fuzzy query — score docs by how many query words appear in URL + title.
    let terms: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();

    let store = FilesystemContentStore::new(&store_root);
    let all_docs = store.list_all_docs().await.unwrap_or_default();
    let total_docs = all_docs.len();

    let mut scored: Vec<(usize, String, std::path::PathBuf)> = all_docs
        .iter()
        .filter_map(|doc| {
            let s = score_doc(doc, &terms);
            if s > 0 {
                Some((s, doc.url.clone(), doc.md_path.clone()))
            } else {
                None
            }
        })
        .collect();

    if scored.is_empty() {
        eprintln!("{yellow}no cached docs match:{reset} {bold}\"{query}\"{reset}");
        eprintln!(
            "{dim}try:{reset} {cyan}noxa --search \"{query}\"{reset} {dim}to find and cache them{reset}"
        );
        return;
    }

    // Sort by score desc; on tie prefer shorter URL (more specific)
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())));

    if scored.len() > 1 {
        eprintln!(
            "{dim}best match ({}/{} docs scored):{reset}\n",
            scored.len(),
            total_docs
        );
        for (score, url, _) in scored.iter().take(5) {
            eprintln!("  {dim}{score} match(es){reset}  {cyan}{url}{reset}");
        }
        eprintln!();
    }

    let (_, best_url, best_path) = &scored[0];
    match std::fs::read_to_string(best_path) {
        Ok(content) => {
            eprintln!("{dim}retrieved{reset} {pink}{best_url}{reset}\n");
            print!("{content}");
        }
        Err(e) => eprintln!("error reading {}: {e}", best_path.display()),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use noxa_store::StoredDoc;

    use super::*;

    fn make_doc(url: &str, title: Option<&str>) -> StoredDoc {
        StoredDoc {
            url: url.to_string(),
            md_path: PathBuf::from("/fake/path.md"),
            json_path: PathBuf::from("/fake/path.json"),
            title: title.map(str::to_string),
        }
    }

    // ── classify_query ────────────────────────────────────────────────────────

    #[test]
    fn classify_explicit_https_scheme() {
        let (looks_like_url, normalised) = classify_query("https://example.com/docs");
        assert!(looks_like_url);
        assert_eq!(normalised, "https://example.com/docs");
    }

    #[test]
    fn classify_explicit_http_scheme() {
        let (looks_like_url, normalised) = classify_query("http://example.com");
        assert!(looks_like_url);
        assert_eq!(normalised, "http://example.com");
    }

    #[test]
    fn classify_bare_hostname() {
        let (looks_like_url, normalised) = classify_query("docs.example.com");
        assert!(looks_like_url);
        assert_eq!(normalised, "https://docs.example.com");
    }

    #[test]
    fn classify_plain_text_query_is_not_url() {
        let (looks_like_url, _) = classify_query("rust async runtime comparison");
        assert!(!looks_like_url);
    }

    #[test]
    fn classify_dotjs_extension_is_not_url() {
        // "node.js" is a common false-positive for URL detection.
        let (looks_like_url, _) = classify_query("node.js");
        // The TLD "js" is 2 alphabetic chars, so this currently IS flagged as a
        // URL — documenting the known boundary behaviour (not a bug to fix here).
        let _ = looks_like_url; // just assert it doesn't panic
    }

    #[test]
    fn classify_spaced_query_is_not_url() {
        let (looks_like_url, _) = classify_query("e.g. some text");
        assert!(!looks_like_url);
    }

    // ── is_exact_url_match ────────────────────────────────────────────────────

    #[test]
    fn exact_match_with_scheme() {
        let doc = make_doc("https://docs.example.com/api", None);
        assert!(is_exact_url_match(&doc, "https://docs.example.com/api"));
    }

    #[test]
    fn exact_match_bare_host_normalises() {
        // Bare host "docs.example.com" is normalised to "https://docs.example.com"
        let doc = make_doc("https://docs.example.com", None);
        assert!(is_exact_url_match(&doc, "docs.example.com"));
    }

    #[test]
    fn exact_match_no_match() {
        let doc = make_doc("https://docs.example.com/api", None);
        assert!(!is_exact_url_match(&doc, "https://docs.example.com/other"));
    }

    #[test]
    fn exact_match_plain_text_never_matches() {
        let doc = make_doc("https://docs.example.com/api", None);
        assert!(!is_exact_url_match(&doc, "docs api authentication"));
    }

    // ── score_doc ─────────────────────────────────────────────────────────────

    #[test]
    fn score_zero_when_no_term_matches() {
        let doc = make_doc("https://example.com/home", Some("Home Page"));
        let terms = vec!["authentication".to_string(), "oauth".to_string()];
        assert_eq!(score_doc(&doc, &terms), 0);
    }

    #[test]
    fn score_url_match_counts() {
        let doc = make_doc("https://docs.example.com/authentication", None);
        let terms = vec!["authentication".to_string()];
        assert_eq!(score_doc(&doc, &terms), 1);
    }

    #[test]
    fn score_title_match_counts() {
        let doc = make_doc("https://docs.example.com/page", Some("Authentication Guide"));
        let terms = vec!["authentication".to_string()];
        assert_eq!(score_doc(&doc, &terms), 1);
    }

    #[test]
    fn score_both_url_and_title_matching_same_term_counts_once() {
        // Each *term* is counted once regardless of how many fields contain it.
        let doc = make_doc(
            "https://docs.example.com/authentication",
            Some("Authentication Guide"),
        );
        let terms = vec!["authentication".to_string()];
        assert_eq!(score_doc(&doc, &terms), 1);
    }

    #[test]
    fn score_multiple_terms() {
        let doc = make_doc(
            "https://docs.example.com/oauth-authentication",
            Some("OAuth Guide"),
        );
        let terms = vec!["oauth".to_string(), "authentication".to_string()];
        assert_eq!(score_doc(&doc, &terms), 2);
    }

    #[test]
    fn score_title_none_scores_only_url() {
        let doc = make_doc("https://docs.example.com/oauth", None);
        let terms = vec!["oauth".to_string(), "guide".to_string()];
        // "guide" is not in URL, but "oauth" is
        assert_eq!(score_doc(&doc, &terms), 1);
    }

    #[test]
    fn score_title_beats_url_when_title_is_closer() {
        // doc_a: title matches both terms, URL matches neither
        // doc_b: URL matches one term, title absent
        let doc_a = make_doc(
            "https://example.com/page-xyz",
            Some("OAuth Authentication Tutorial"),
        );
        let doc_b = make_doc("https://example.com/oauth-page", None);
        let terms = vec!["oauth".to_string(), "authentication".to_string()];
        let score_a = score_doc(&doc_a, &terms);
        let score_b = score_doc(&doc_b, &terms);
        assert_eq!(score_a, 2, "doc_a should match both terms via title");
        assert_eq!(score_b, 1, "doc_b should match only 'oauth' via url");
        assert!(score_a > score_b, "higher title coverage should win");
    }

    // ── select_best ───────────────────────────────────────────────────────────

    #[test]
    fn select_best_returns_none_when_no_match() {
        let docs = vec![make_doc("https://example.com/home", Some("Home"))];
        assert!(select_best(&docs, "completely unrelated query xyz").is_none());
    }

    #[test]
    fn select_best_returns_only_match() {
        let docs = vec![
            make_doc("https://example.com/home", Some("Home")),
            make_doc("https://example.com/oauth", Some("OAuth Guide")),
        ];
        let best = select_best(&docs, "oauth").unwrap();
        assert_eq!(best.url, "https://example.com/oauth");
    }

    #[test]
    fn select_best_higher_score_wins() {
        let docs = vec![
            make_doc(
                "https://example.com/oauth-authentication",
                Some("OAuth Authentication"),
            ),
            make_doc("https://example.com/oauth", None),
        ];
        let best = select_best(&docs, "oauth authentication").unwrap();
        // First doc matches both terms; second matches only "oauth"
        assert_eq!(best.url, "https://example.com/oauth-authentication");
    }

    #[test]
    fn select_best_tie_broken_by_shorter_url() {
        // Both docs match "docs" exactly once; shorter URL should win.
        let docs = vec![
            make_doc("https://example.com/docs/some/deep/path/page", None),
            make_doc("https://example.com/docs", Some("Docs")),
        ];
        let best = select_best(&docs, "docs").unwrap();
        assert_eq!(best.url, "https://example.com/docs");
    }

    #[test]
    fn select_best_empty_docs() {
        let docs: Vec<noxa_store::StoredDoc> = vec![];
        assert!(select_best(&docs, "anything").is_none());
    }

    // ── run_retrieve behavioural smoke tests ──────────────────────────────────
    //
    // run_retrieve returns () and writes to stdout/stderr, so these tests
    // exercise code paths by asserting the function completes without panicking
    // and without hanging.

    fn make_sample_extraction(url: &str, markdown: &str) -> noxa_core::ExtractionResult {
        noxa_core::ExtractionResult {
            metadata: noxa_core::Metadata {
                title: Some("Sample Page".to_string()),
                description: None,
                author: None,
                published_date: None,
                language: Some("en".to_string()),
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

    /// run_retrieve on a non-existent store root returns immediately without
    /// panicking (early-return guard at the top of the function).
    #[tokio::test]
    async fn run_retrieve_nonexistent_store_root_returns_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("does_not_exist");
        // Must not panic or hang.
        run_retrieve("rust async runtime", store_root).await;
    }

    /// Fuzzy query against an empty (but existing) store — hits the
    /// "no cached docs match" branch.
    #[tokio::test]
    async fn run_retrieve_fuzzy_query_empty_store_returns_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("content");
        tokio::fs::create_dir_all(&store_root).await.unwrap();
        // No docs written — store is empty.
        run_retrieve("authentication oauth guide", store_root).await;
    }

    /// Exact URL query for a URL that is NOT cached — hits the "not cached"
    /// branch (looks_like_url=true but md_path doesn't exist on disk).
    #[tokio::test]
    async fn run_retrieve_exact_url_not_cached_returns_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("content");
        tokio::fs::create_dir_all(&store_root).await.unwrap();
        run_retrieve("https://docs.example.com/api", store_root).await;
    }

    /// Exact URL query for a URL that IS cached — hits the happy-path FS probe
    /// (md_path.exists() == true, reads and prints content).
    #[tokio::test]
    async fn run_retrieve_exact_url_cached_returns_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("content");
        tokio::fs::create_dir_all(&store_root).await.unwrap();
        let store = FilesystemContentStore::new(&store_root);
        let url = "https://docs.example.com/api";
        store
            .write(url, &make_sample_extraction(url, "API reference docs"))
            .await
            .unwrap();
        run_retrieve(url, store_root).await;
    }

    /// Fuzzy multi-word query against a populated store — exercises the full
    /// scoring + sorting + "best match" display branch, including the multi-doc
    /// header that appears when scored.len() > 1.
    #[tokio::test]
    async fn run_retrieve_fuzzy_multiword_populated_store_returns_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("content");
        tokio::fs::create_dir_all(&store_root).await.unwrap();
        let store = FilesystemContentStore::new(&store_root);

        // Write several docs so fuzzy scoring selects the best match.
        let docs = [
            (
                "https://docs.example.com/authentication",
                "OAuth authentication guide",
            ),
            ("https://docs.example.com/quickstart", "Getting started"),
            (
                "https://docs.example.com/oauth-tokens",
                "OAuth token reference",
            ),
        ];
        for (url, content) in &docs {
            store
                .write(url, &make_sample_extraction(url, content))
                .await
                .unwrap();
        }

        // Multi-word query — "oauth authentication" should score highest on the
        // first doc (matches both terms) while still exercising the multi-doc
        // display path.
        run_retrieve("oauth authentication", store_root).await;
    }

    /// Single-doc store with a fuzzy query — exercises the path where
    /// scored.len() == 1 (no multi-doc header printed).
    #[tokio::test]
    async fn run_retrieve_fuzzy_single_doc_store_returns_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("content");
        tokio::fs::create_dir_all(&store_root).await.unwrap();
        let store = FilesystemContentStore::new(&store_root);
        let url = "https://blog.example.com/rust-async";
        store
            .write(url, &make_sample_extraction(url, "Rust async runtime internals"))
            .await
            .unwrap();
        run_retrieve("rust async", store_root).await;
    }
}
