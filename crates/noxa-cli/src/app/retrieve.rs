use super::*;

pub(crate) fn run_retrieve(query: &str, store_root: std::path::PathBuf) {
    if !store_root.exists() {
        eprintln!(
            "{dim}no local docs — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --crawl <url>{reset}"
        );
        return;
    }

    // Exact URL lookup
    let looks_like_url = query.starts_with("http://")
        || query.starts_with("https://")
        || (!query.contains(' ') && query.contains('.'));

    if looks_like_url {
        let url = if query.starts_with("http") {
            query.to_string()
        } else {
            format!("https://{query}")
        };
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

    // Fuzzy query — score docs by how many query words appear in URL + title
    let terms: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();

    let mut scored: Vec<(usize, String, std::path::PathBuf)> = Vec::new();

    // Walk and score inline to avoid a second pass
    let mut all_docs: Vec<(String, std::path::PathBuf)> = Vec::new();
    collect_docs(&store_root, &store_root, &mut all_docs);

    let total_docs = all_docs.len();

    for (url, path) in all_docs {
        let url_lower = url.to_lowercase();
        // Also pull title from JSON sidecar if present.
        // Support both new Sidecar envelope and legacy raw ExtractionResult.
        let title_lower = path
            .with_extension("json")
            .pipe(|jp| std::fs::read_to_string(jp).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| {
                // New sidecar: nested under current.metadata.title.
                if let Some(t) = v["current"]["metadata"]["title"].as_str() {
                    return Some(t.to_lowercase());
                }
                // Legacy format: metadata.title at top level.
                v["metadata"]["title"].as_str().map(|t| t.to_lowercase())
            })
            .unwrap_or_default();
        let score = terms
            .iter()
            .filter(|t| url_lower.contains(t.as_str()) || title_lower.contains(t.as_str()))
            .count();
        if score > 0 {
            scored.push((score, url, path));
        }
    }

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

// Helper to pipe a value through a function (avoids temp var for path transform)
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}
impl<T> Pipe for T {}
