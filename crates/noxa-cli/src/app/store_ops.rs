use super::*;

pub(crate) fn run_list(filter: &str, store_root: std::path::PathBuf) {
    if !store_root.exists() {
        eprintln!(
            "{dim}no local docs yet — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --search \"...\"{reset} {dim}to build your store{reset}"
        );
        return;
    }

    if filter.is_empty() {
        // Top-level: list all domain directories with doc counts
        let mut domains: Vec<(String, usize)> = std::fs::read_dir(&store_root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let count = count_md_files(&e.path());
                (name, count)
            })
            .collect();
        domains.sort_by(|a, b| a.0.cmp(&b.0));

        if domains.is_empty() {
            eprintln!("{dim}no docs stored yet{reset}");
            return;
        }

        let total: usize = domains.iter().map(|(_, c)| c).sum();
        eprintln!("\n{bold}{cyan}stored docs{reset}  {dim}{total} total{reset}\n");
        for (domain, count) in &domains {
            eprintln!("  {bold}{domain}{reset}  {dim}({count}){reset}");
        }
        eprintln!("\n{dim}noxa --list <domain>{reset}  {dim}to see individual docs{reset}\n");
    } else {
        // Domain view: list all docs for matching domain dir, URL → path
        let domain = filter.strip_prefix("www.").unwrap_or(filter);
        let Some(domain_component) = validated_domain_component(domain) else {
            eprintln!("error: invalid domain filter: {filter}");
            return;
        };
        let domain_dir = store_root.join(&domain_component);
        if !domain_dir.exists() {
            // Try sanitized form (dots → underscores)
            eprintln!("{dim}no docs found for{reset} {bold}{filter}{reset}");
            return;
        }
        list_domain_docs(&domain_dir, &store_root, filter);
    }
}

pub(crate) fn count_md_files(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                count_md_files(&p)
            } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
                1
            } else {
                0
            }
        })
        .sum()
}

pub(crate) fn list_domain_docs(dir: &std::path::Path, store_root: &std::path::Path, filter: &str) {
    let mut docs: Vec<(String, std::path::PathBuf)> = Vec::new();
    collect_docs(dir, store_root, &mut docs);
    docs.sort_by(|a, b| a.1.cmp(&b.1));

    if docs.is_empty() {
        eprintln!("{dim}no docs found for {reset}{bold}{filter}{reset}");
        return;
    }

    // Measure URL column width for alignment.
    // Note: uses byte length (url.len()), which is correct for ASCII store keys
    // (normalized URLs are always ASCII). Non-ASCII display widths would require
    // the unicode-width crate but are not needed in practice here.
    let url_width = docs.iter().map(|(url, _)| url.len()).max().unwrap_or(0);

    eprintln!(
        "\n{bold}{cyan}{filter}{reset}  {dim}({} docs){reset}\n",
        docs.len()
    );
    for (url, path) in &docs {
        let rel = path.strip_prefix(store_root).unwrap_or(path);
        eprintln!(
            "  {blue}{url:<url_width$}{reset}  {dim}{}{reset}",
            rel.display()
        );
    }
    eprintln!();
}

#[allow(clippy::only_used_in_recursion)]
pub(crate) fn collect_docs(
    dir: &std::path::Path,
    store_root: &std::path::Path,
    out: &mut Vec<(String, std::path::PathBuf)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_docs(&path, store_root, out);
        } else if path.extension().and_then(|x| x.to_str()) == Some("md") {
            // Read URL from JSON sidecar; fall back to reconstructing from path.
            // Support both the new Sidecar envelope (url at top-level, metadata
            // nested under "current") and the legacy raw ExtractionResult format
            // (metadata at top-level).
            let json_path = path.with_extension("json");
            let url = std::fs::read_to_string(&json_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| {
                    // New sidecar: top-level "url" field.
                    if let Some(u) = v["url"].as_str().filter(|s| !s.is_empty()) {
                        return Some(u.to_string());
                    }
                    // New sidecar: nested under current.metadata.url.
                    if let Some(u) = v["current"]["metadata"]["url"].as_str() {
                        return Some(u.to_string());
                    }
                    // Legacy format: metadata.url at top level.
                    v["metadata"]["url"].as_str().map(|u| u.to_string())
                });
            let url = url.or_else(|| reconstruct_url_from_store_path(&path, store_root));
            if let Some(url) = url {
                out.push((url, path));
            }
        }
    }
}

pub(crate) fn run_grep(pattern: &str, store_root: std::path::PathBuf) {
    if !store_root.exists() {
        eprintln!(
            "{dim}no local docs yet — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --search \"...\"{reset} {dim}to build your store{reset}"
        );
        return;
    }

    eprintln!(
        "\n{bold}{cyan}grep{reset}  {bold}{pattern}{reset}  {dim}{}{reset}\n",
        store_root.display()
    );

    // Try rg first — it's fast and produces great output natively
    let rg_status = std::process::Command::new("rg")
        .args([
            "--color=always",
            "--heading",
            "--line-number",
            "--smart-case",
            pattern,
        ])
        .arg(&store_root)
        .status();

    match rg_status {
        Ok(status) => {
            if status.code() == Some(1) {
                // rg exit 1 = no matches
                eprintln!("{dim}no matches for {reset}{bold}\"{pattern}\"{reset}");
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // rg not installed — fall back to a simple Rust walk
            eprintln!("{dim}rg not found, using built-in search{reset}\n");
            let pattern_lower = pattern.to_lowercase();
            let mut matched_files = 0usize;
            let mut matched_lines = 0usize;
            if let Ok(walker) = std::fs::read_dir(&store_root) {
                grep_dir(
                    walker,
                    &pattern_lower,
                    &store_root,
                    &mut matched_files,
                    &mut matched_lines,
                );
            }
            if matched_files == 0 {
                eprintln!("{dim}no matches for {reset}{bold}\"{pattern}\"{reset}");
            } else {
                eprintln!(
                    "\n{green}{bold}✓{reset} {bold}{matched_lines} match(es){reset} {dim}across {matched_files} file(s){reset}"
                );
            }
        }
        Err(e) => eprintln!("error running rg: {e}"),
    }
}

pub(crate) fn grep_dir(
    entries: std::fs::ReadDir,
    pattern: &str,
    root: &std::path::Path,
    matched_files: &mut usize,
    matched_lines: &mut usize,
) {
    let mut paths: Vec<std::path::PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if let Ok(sub) = std::fs::read_dir(&path) {
                grep_dir(sub, pattern, root, matched_files, matched_lines);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let hits: Vec<(usize, &str)> = content
                .lines()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(pattern))
                .collect();
            if !hits.is_empty() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                eprintln!("{pink}{}{reset}", rel.display());
                for (lineno, line) in &hits {
                    let trimmed = line.trim();
                    let display = truncate_display(trimmed, 120);
                    eprintln!("  {dim}{:>4}{reset}  {bold}{display}{reset}", lineno + 1);
                    *matched_lines += 1;
                }
                eprintln!();
                *matched_files += 1;
            }
        }
    }
}

fn validated_domain_component(filter: &str) -> Option<String> {
    let normalized = filter.trim();
    if normalized.is_empty()
        || normalized.contains('\0')
        || normalized.starts_with(['/', '\\'])
        || normalized
            .split(['/', '\\'])
            .any(|part| part == "." || part == "..")
    {
        return None;
    }
    Some(
        normalized
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect(),
    )
}

fn reconstruct_url_from_store_path(
    path: &std::path::Path,
    store_root: &std::path::Path,
) -> Option<String> {
    let rel = path.strip_prefix(store_root).ok()?;
    let mut components = rel.components();
    let domain = components.next()?.as_os_str().to_str()?.replace('_', ".");
    let stem = rel.with_extension("");
    let mut segments = stem
        .components()
        .skip(1)
        .filter_map(|part| part.as_os_str().to_str())
        .collect::<Vec<_>>();
    if segments.last().copied() == Some("index") {
        segments.pop();
    }
    let mut url = format!("https://{domain}");
    if !segments.is_empty() {
        url.push('/');
        url.push_str(&segments.join("/"));
    }
    Some(url)
}

fn truncate_display(line: &str, max_chars: usize) -> &str {
    let mut end = line.len();
    let mut seen = 0usize;
    for (idx, _) in line.char_indices() {
        if seen == max_chars {
            end = idx;
            break;
        }
        seen += 1;
    }
    if seen < max_chars { line } else { &line[..end] }
}

pub(crate) async fn run_search(
    cli: &Cli,
    fetch_client: &Arc<noxa_fetch::FetchClient>,
    resolved: &config::ResolvedConfig,
    query: &str,
) -> Result<(), String> {
    if query.trim().is_empty() {
        return Err("Search query must not be empty or whitespace-only.".into());
    }

    let num = cli.num_results.clamp(1, 50);
    let concurrency = clamp_search_scrape_concurrency(cli.num_scrape_concurrency);

    let mut search_backend = String::from("noxa cloud");
    let results: Vec<(String, String, String)> = {
        let searxng_url = std::env::var("SEARXNG_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(base_url) = searxng_url {
            validate_operator_url(&base_url).map_err(|e| format!("SEARXNG_URL is invalid: {e}"))?;
            // Strip scheme for display (searxng.example.com vs https://searxng.example.com)
            let display = base_url
                .strip_prefix("https://")
                .or_else(|| base_url.strip_prefix("http://"))
                .unwrap_or(&base_url);
            search_backend = format!("searxng ({display})");
            noxa_fetch::searxng_search(fetch_client, &base_url, query, num)
                .await
                .map_err(|e| format!("SearXNG search failed: {e}"))?
                .into_iter()
                .map(|r| (r.title, r.url, r.content))
                .collect()
        } else {
            let api_key = cli
                .api_key
                .clone()
                .filter(|s| !s.is_empty())
                .or_else(|| std::env::var("NOXA_API_KEY").ok().filter(|s| !s.is_empty()))
                .ok_or("--search requires SEARXNG_URL or NOXA_API_KEY")?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| format!("http client error: {e}"))?;
            let resp = client
                .post("https://api.noxa.io/v1/search")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&serde_json::json!({ "query": query, "num_results": num }))
                .send()
                .await
                .map_err(|e| format!("API error: {e}"))?;
            let status = resp.status();
            // Read text first so we can include a body preview in both error paths.
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                let preview: String = body.chars().take(240).collect();
                return Err(format!("search API returned HTTP {status}: {preview}"));
            }
            let resp: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
                let preview: String = body.chars().take(240).collect();
                format!("parse error: {e} (body: {preview})")
            })?;
            resp.get("results")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|r| {
                            (
                                r.get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                r.get("url")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                r.get("snippet")
                                    .or_else(|| r.get("content"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    if results.is_empty() {
        eprintln!("{yellow}no results found for: {query}{reset}");
        return Ok(());
    }

    eprintln!(
        "\n{bold}{cyan}search{reset}  {bold}{query}{reset}  {dim}{} result(s)  via {search_backend}{reset}\n",
        results.len()
    );

    if cli.no_scrape {
        for (i, (title, url, snip)) in results.iter().enumerate() {
            println!("{dim}{i:2}.{reset} {bold}{title}{reset}");
            println!("     {blue}{url}{reset}");
            if !snip.is_empty() {
                println!("     {dim}{snip}{reset}");
            }
            println!();
        }
        return Ok(());
    }

    let store_root = content_store_root(resolved.output_dir.as_deref());

    let valid: Vec<(usize, String, String, String)> = results
        .into_iter()
        .enumerate()
        .filter_map(|(i, (title, url, snip))| match validate_url_sync(&url) {
            Ok(()) => Some((i + 1, title, url, snip)),
            Err(e) => {
                eprintln!("{dim}   skip {url}: {e}{reset}");
                None
            }
        })
        .collect();

    let url_refs: Vec<&str> = valid.iter().map(|(_, _, u, _)| u.as_str()).collect();
    let options = build_extraction_options(resolved);
    let scraped = fetch_client
        .fetch_and_extract_batch_with_options(&url_refs, concurrency, &options)
        .await;

    for ((idx, title, url, snip), scrape) in valid.iter().zip(scraped.iter()) {
        let store_path = store_root.join(url_to_store_path(url)).with_extension("md");
        println!("{dim}{idx:2}.{reset} {bold}{title}{reset}");
        println!("     {blue}{url}{reset}");
        if !snip.is_empty() {
            println!("     {dim}{snip}{reset}");
        }
        match &scrape.result {
            Ok(_) => println!("     {green}✓{reset} {pink}{}{reset}", store_path.display()),
            Err(e) => println!("     {yellow}✗ scrape failed:{reset} {dim}{e}{reset}"),
        }
        println!();
    }

    let saved = scraped.iter().filter(|s| s.result.is_ok()).count();
    eprintln!(
        "{green}{bold}✓{reset} {bold}{saved}/{} scraped{reset}  {pink}{}{reset}\n\
         {dim}  grep{reset}  {cyan}noxa --grep {green}\"TERM\"{reset}\n",
        valid.len(),
        store_root.display(),
    );

    Ok(())
}
