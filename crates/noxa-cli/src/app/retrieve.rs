use super::*;

pub(crate) async fn run_retrieve(query: &str, store_root: std::path::PathBuf) {
    if !store_root.exists() {
        eprintln!(
            "{dim}no local docs — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --crawl <url>{reset}"
        );
        return;
    }

    // Exact URL lookup.
    // Require an explicit scheme (http:// or https://) OR a parseable URL with a valid
    // host that has at least one dot and a TLD-like final label (2+ alphabetic chars).
    // This avoids treating tokens like "node.js", "e.g.", or "readme.md" as URLs.
    let has_scheme = query.starts_with("http://") || query.starts_with("https://");
    let url_candidate = if has_scheme {
        query.to_string()
    } else {
        format!("https://{query}")
    };
    let looks_like_url = has_scheme || (!query.contains(' ') && query.contains('.') && {
        url::Url::parse(&url_candidate)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .map(|host| {
                let parts: Vec<&str> = host.split('.').collect();
                parts.len() >= 2
                    && parts.last()
                        .map(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
                        .unwrap_or(false)
            })
            .unwrap_or(false)
    });

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
        .into_iter()
        .filter_map(|doc| {
            let url_lower = doc.url.to_lowercase();
            let title_lower = doc.title.as_deref().unwrap_or("").to_lowercase();
            let score = terms
                .iter()
                .filter(|t| url_lower.contains(t.as_str()) || title_lower.contains(t.as_str()))
                .count();
            if score > 0 {
                Some((score, doc.url, doc.md_path))
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
