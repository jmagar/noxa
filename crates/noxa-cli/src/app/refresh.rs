use super::*;

pub(crate) fn sidecar_url_from_value(value: &serde_json::Value) -> Option<String> {
    if let Some(url) = value["url"].as_str().filter(|s| !s.is_empty()) {
        return Some(url.to_string());
    }
    if let Some(url) = value["current"]["metadata"]["url"].as_str() {
        return Some(url.to_string());
    }
    value["metadata"]["url"].as_str().map(|url| url.to_string())
}

pub(crate) fn refresh_domain_dir(store_root: &Path, domain: &str) -> Result<PathBuf, String> {
    let normalized = normalize_url(domain.trim());
    let component = url_to_store_path(&normalized)
        .components()
        .next()
        .map(|part| part.as_os_str().to_owned())
        .ok_or_else(|| format!("invalid refresh domain: {domain}"))?;
    Ok(store_root.join(component))
}

pub(crate) async fn read_refresh_sidecar_url(json_path: &Path) -> Result<Option<String>, String> {
    let contents = tokio::fs::read_to_string(json_path)
        .await
        .map_err(|e| format!("failed to read {}: {e}", json_path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("failed to parse {}: {e}", json_path.display()))?;
    Ok(sidecar_url_from_value(&value))
}

pub(crate) async fn collect_refresh_urls(
    store_root: &Path,
    domain: &str,
) -> Result<Vec<String>, String> {
    if !tokio::fs::try_exists(store_root)
        .await
        .map_err(|e| format!("failed to inspect {}: {e}", store_root.display()))?
    {
        return Ok(Vec::new());
    }

    let domain_dir = refresh_domain_dir(store_root, domain)?;
    if !tokio::fs::try_exists(&domain_dir)
        .await
        .map_err(|e| format!("failed to inspect {}: {e}", domain_dir.display()))?
    {
        return Ok(Vec::new());
    }

    let canonical_root = tokio::fs::canonicalize(store_root)
        .await
        .map_err(|e| format!("failed to canonicalize {}: {e}", store_root.display()))?;
    let mut stack = vec![domain_dir];
    let mut urls = Vec::new();

    while let Some(dir) = stack.pop() {
        let canonical_dir = tokio::fs::canonicalize(&dir)
            .await
            .map_err(|e| format!("failed to canonicalize {}: {e}", dir.display()))?;
        if !canonical_dir.starts_with(&canonical_root) {
            return Err(format!(
                "refresh traversal escaped content-store root: {}",
                dir.display()
            ));
        }

        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .map_err(|e| format!("failed to read {}: {e}", dir.display()))?;
        let mut paths = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("failed to read {}: {e}", dir.display()))?
        {
            paths.push(entry.path());
        }
        paths.sort();

        for path in paths {
            let metadata = tokio::fs::symlink_metadata(&path)
                .await
                .map_err(|e| format!("failed to inspect {}: {e}", path.display()))?;
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            match read_refresh_sidecar_url(&path).await {
                Ok(Some(url)) => urls.push(url),
                Ok(None) => {}
                Err(error) => eprintln!("{yellow}warning:{reset} {error}"),
            }
        }
    }

    urls.sort();
    urls.dedup();
    Ok(urls)
}

pub(crate) async fn run_refresh(
    domain: &str,
    cli: &Cli,
    resolved: &config::ResolvedConfig,
) -> Result<(), String> {
    if cli.no_store {
        return Err("--refresh requires the content store; rerun without --no-store".into());
    }

    let domain = domain.trim();
    if domain.is_empty() {
        return Err("--refresh requires a domain".into());
    }

    let store_root = content_store_root(resolved.output_dir.as_deref());
    let urls = collect_refresh_urls(&store_root, domain).await?;
    if urls.is_empty() {
        eprintln!("{dim}no cached docs found for{reset} {bold}{domain}{reset}");
        eprintln!("{dim}run:{reset} {cyan}noxa --list {domain}{reset}");
        return Ok(());
    }

    let mut fetch_config = build_fetch_config(cli, resolved);
    fetch_config.store = None;
    let client = FetchClient::new(fetch_config).map_err(|e| format!("client error: {e}"))?;
    let store = FilesystemContentStore::new(&store_root);
    let options = build_extraction_options(resolved);

    let mut unchanged = 0usize;
    let mut changed = 0usize;
    let mut failed = 0usize;

    eprintln!(
        "\n{bold}{cyan}refresh{reset}  {bold}{domain}{reset}  {dim}({} docs){reset}\n",
        urls.len()
    );

    for url in &urls {
        if let Err(error) = validate_url(url).await {
            failed += 1;
            eprintln!("  {yellow}skip{reset}  {url}  {dim}{error}{reset}");
            continue;
        }

        let extraction = match client.fetch_and_extract_with_options(url, &options).await {
            Ok(extraction) => extraction,
            Err(error) => {
                failed += 1;
                eprintln!("  {yellow}error{reset}  {url}  {dim}{error}{reset}");
                continue;
            }
        };

        let result = store
            .write(url, &extraction)
            .await
            .map_err(|e| format!("failed to store {url}: {e}"))?;

        if result.changed {
            changed += 1;
            let direction = if result.word_count_delta > 0 {
                format!("+{}", result.word_count_delta)
            } else {
                result.word_count_delta.to_string()
            };
            eprintln!("  {green}updated{reset}  {url}  {dim}{direction} words{reset}");
        } else {
            unchanged += 1;
            eprintln!("  {dim}unchanged{reset}  {url}");
        }
    }

    eprintln!(
        "\n  {green}{bold}✓{reset} {bold}{changed}{reset} updated  {dim}{unchanged} unchanged{reset}  {}",
        if failed > 0 {
            format!("{yellow}{failed} failed{reset}")
        } else {
            format!("{dim}0 failed{reset}")
        }
    );

    if failed > 0 {
        Err(format!("{failed} refreshes failed"))
    } else {
        Ok(())
    }
}

pub(crate) fn run_status(domain: &str) {
    let status_path = crawl_status_path(domain);
    let status = match read_crawl_status(&status_path) {
        Ok(status) => Some(status),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            eprintln!("error reading status: {error}");
            return;
        }
    };
    let pid_running = status
        .as_ref()
        .is_some_and(|status| is_pid_running(status.pid));
    let state = classify_crawl_status(status.as_ref(), pid_running);

    if matches!(state, CrawlStatusState::NeverStarted) {
        eprintln!(
            "\n  {dim}crawl{reset}  {bold}{cyan}{domain}{reset}  {dim}never started{reset}\n"
        );
        eprintln!("  {dim}run:{reset}  {cyan}noxa --crawl {domain}{reset}\n");
        return;
    }

    let status = status.expect("status should exist for non-never-started state");
    let url = status.url.as_str();
    let pages_done = status.pages_done;
    let pages_ok = status.pages_ok;
    let pages_errors = status.pages_errors;
    let max_pages = status.max_pages;
    let last_url = status.last_url.as_deref().unwrap_or("");
    let elapsed = status.elapsed_secs;
    let pid = status.pid;
    let docs_dir = status.docs_dir.as_str();
    let excluded = status.excluded;
    let total_words = status.total_words;

    let state_label = match state {
        CrawlStatusState::Done => format!("{green}{bold}done{reset}"),
        CrawlStatusState::Running => format!("{yellow}{bold}running{reset}"),
        CrawlStatusState::Stale => format!("{yellow}{bold}stale{reset}"),
        CrawlStatusState::NeverStarted => unreachable!(),
    };

    let done = matches!(state, CrawlStatusState::Done);
    let pages_display = if done || max_pages == 0 {
        format!("{bold}{pages_done}{reset}")
    } else {
        format!("{bold}{pages_done}{reset}{dim}/{max_pages}{reset}")
    };

    eprintln!("\n  {dim}crawl{reset}  {bold}{cyan}{url}{reset}  {state_label}\n");
    if !docs_dir.is_empty() {
        eprintln!("  {dim}docs{reset}     {pink}{docs_dir}{reset}");
    }
    let words_suffix = if done && total_words > 0 {
        if total_words >= 1_000_000 {
            format!(
                "  {dim}~{:.1}M words{reset}",
                total_words as f64 / 1_000_000.0
            )
        } else if total_words >= 1_000 {
            format!("  {dim}~{}k words{reset}", total_words / 1_000)
        } else {
            format!("  {dim}{total_words} words{reset}")
        }
    } else {
        String::new()
    };
    let excl_suffix = if done && excluded > 0 {
        format!("  {dim}{excluded} excluded{reset}")
    } else {
        String::new()
    };
    eprintln!(
        "  {dim}pages{reset}    {pages_display}  {green}{pages_ok} ok{reset}  {}{words_suffix}{excl_suffix}",
        if pages_errors > 0 {
            format!("{yellow}{pages_errors} errors{reset}")
        } else {
            format!("{dim}0 errors{reset}")
        }
    );
    eprintln!("  {dim}elapsed{reset}  {bold}{elapsed:.1}s{reset}");
    if !last_url.is_empty() && !done {
        eprintln!("  {dim}last{reset}     {pink}{last_url}{reset}");
    }
    if matches!(state, CrawlStatusState::Running) {
        eprintln!("  {dim}pid{reset}      {dim}{pid}{reset}");
        eprintln!("\n  {dim}noxa --crawl {url} --wait{reset}  {dim}to stream live progress{reset}");
    }
    if matches!(state, CrawlStatusState::Stale) && pid > 0 {
        eprintln!("  {dim}pid{reset}      {dim}{pid}{reset}");
    }
    if done {
        // Strip scheme for cleaner display in hints
        let bare = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);
        let bare = bare.strip_prefix("www.").unwrap_or(bare);
        let bare = bare.trim_end_matches('/');
        eprintln!("\n  {dim}noxa --list {bare}{reset}          {dim}browse cached pages{reset}");
        eprintln!("  {dim}noxa --retrieve <url>{reset}      {dim}read a specific page{reset}");
        eprintln!(
            "  {dim}noxa --grep \"<term>\" {reset}       {dim}search across all pages{reset}"
        );
    }
    eprintln!();
}
