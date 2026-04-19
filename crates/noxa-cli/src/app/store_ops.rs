use super::*;

const MAX_URL_DISPLAY: usize = 80;

fn truncate_url(url: &str) -> String {
    if url.len() <= MAX_URL_DISPLAY {
        return url.to_string();
    }
    let truncated: String = url.chars().take(MAX_URL_DISPLAY - 3).collect();
    format!("{truncated}...")
}

pub(crate) async fn run_list(filter: &str, store_root: std::path::PathBuf) -> Result<(), String> {
    if !store_root.exists() {
        eprintln!(
            "{dim}no local docs yet — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --search \"...\"{reset} {dim}to build your store{reset}"
        );
        return Ok(());
    }

    let store = FilesystemContentStore::new(&store_root);

    if filter.is_empty() {
        // Top-level: list all domain directories with doc counts.
        let domains = match store.list_domains().await {
            Ok(domains) => domains,
            Err(e) => return Err(format!("error listing stored docs: {e}")),
        };

        if domains.is_empty() {
            eprintln!("{dim}no docs stored yet{reset}");
            return Ok(());
        }

        let total: usize = domains.iter().map(|d| d.doc_count).sum();
        eprintln!("\n{bold}{cyan}stored docs{reset}  {dim}{total} total{reset}\n");
        for d in &domains {
            eprintln!("  {bold}{}{reset}  {dim}({}){reset}", d.name, d.doc_count);
        }
        eprintln!("\n{dim}noxa --list <domain>{reset}  {dim}to see individual docs{reset}\n");
    } else {
        // Domain view: list all docs for the given domain.
        let domain = filter.strip_prefix("www.").unwrap_or(filter);
        let docs = match store.list_docs(domain).await {
            Ok(docs) => docs,
            Err(e) => return Err(format!("error listing docs for {filter}: {e}")),
        };

        if docs.is_empty() {
            eprintln!("{dim}no docs found for{reset} {bold}{filter}{reset}");
            return Ok(());
        }

        let url_width = docs
            .iter()
            .map(|d| d.url.len().min(MAX_URL_DISPLAY))
            .max()
            .unwrap_or(0);

        eprintln!(
            "\n{bold}{cyan}{filter}{reset}  {dim}({} docs){reset}\n",
            docs.len()
        );
        for doc in &docs {
            let rel = doc
                .md_path
                .strip_prefix(&store_root)
                .unwrap_or(&doc.md_path);
            let display_url = truncate_url(&doc.url);
            eprintln!(
                "  {blue}{:<url_width$}{reset}  {dim}{}{reset}",
                display_url,
                rel.display()
            );
        }
        eprintln!();
    }
    Ok(())
}

pub(crate) async fn run_grep(
    pattern: &str,
    store_root: std::path::PathBuf,
) -> Result<(), String> {
    if !store_root.exists() {
        eprintln!(
            "{dim}no local docs yet — run{reset} {cyan}noxa <url>{reset} {dim}or{reset} {cyan}noxa --search \"...\"{reset} {dim}to build your store{reset}"
        );
        return Ok(());
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
            // rg not installed — fall back using the store enumeration API with regex
            eprintln!("{dim}rg not found, using built-in search{reset}\n");

            // Match rg smart-case: case-insensitive unless pattern has uppercase.
            let pat = if pattern.chars().any(|c| c.is_uppercase()) {
                pattern.to_string()
            } else {
                format!("(?i){pattern}")
            };
            let re = match regex::Regex::new(&pat) {
                Ok(r) => r,
                Err(e) => return Err(format!("invalid pattern: {e}")),
            };

            let store = FilesystemContentStore::new(&store_root);
            let docs = match store.list_all_docs().await {
                Ok(docs) => docs,
                Err(e) => return Err(format!("error enumerating docs: {e}")),
            };

            // Read all files concurrently then process results in path order.
            let mut set = tokio::task::JoinSet::new();
            for doc in &docs {
                let md_path = doc.md_path.clone();
                let url = doc.url.clone();
                set.spawn(async move {
                    let content = tokio::fs::read_to_string(&md_path).await;
                    (url, md_path, content)
                });
            }
            let mut read_results: Vec<(String, std::path::PathBuf, String)> = Vec::new();
            while let Some(res) = set.join_next().await {
                if let Ok((url, md_path, Ok(content))) = res {
                    read_results.push((url, md_path, content));
                }
            }
            read_results.sort_by(|a, b| a.1.cmp(&b.1));

            let mut matched_files = 0usize;
            let mut matched_lines = 0usize;
            for (_url, md_path, content) in &read_results {
                let hits: Vec<(usize, &str)> = content
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| re.is_match(line))
                    .collect();
                if !hits.is_empty() {
                    let rel = md_path.strip_prefix(&store_root).unwrap_or(md_path);
                    eprintln!("{pink}{}{reset}", rel.display());
                    for (lineno, line) in &hits {
                        let trimmed = line.trim();
                        let display = truncate_display(trimmed, 120);
                        eprintln!("  {dim}{:>4}{reset}  {bold}{display}{reset}", lineno + 1);
                        matched_lines += 1;
                    }
                    eprintln!();
                    matched_files += 1;
                }
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
    Ok(())
}

fn truncate_display(line: &str, max_chars: usize) -> String {
    let mut end = None;
    let mut seen = 0usize;
    for (idx, _) in line.char_indices() {
        if seen == max_chars {
            end = Some(idx);
            break;
        }
        seen += 1;
    }
    match end {
        Some(idx) => format!("{}...", &line[..idx]),
        None => line.to_string(),
    }
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
