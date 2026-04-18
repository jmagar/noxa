use super::*;

pub(crate) fn spawn_crawl_background(cli: &Cli, resolved: &config::ResolvedConfig) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: cannot find self: {e}");
            return;
        }
    };

    // Rebuild args from original argv, inject --wait
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    args.push("--wait".to_string());

    let url = cli.urls.first().map(|s| s.as_str()).unwrap_or("?");
    let status_path = crawl_status_path(&normalize_url(url));
    let log_path = crawl_log_path(&normalize_url(url));
    if let Some(p) = log_path.parent() {
        let _ = std::fs::create_dir_all(p);
    }

    // Open /dev/null for writing to use as a writable sink for stdout/stderr fallback.
    let open_devnull = || {
        std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
    };
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .or_else(|_| open_devnull())
        .expect("failed to open log file and /dev/null fallback");
    let log_clone = log_file
        .try_clone()
        .or_else(|_| open_devnull())
        .expect("failed to clone log fd and /dev/null fallback");

    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(log_clone));

    // Detach from process group so it survives terminal close (Unix only).
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    let domain_dir = crawl_domain_dir(url, resolved.output_dir.as_deref());

    match cmd.spawn() {
        Ok(child) => {
            if let Err(error) = write_initial_crawl_status(
                &status_path,
                &normalize_url(url),
                child.id(),
                resolved.max_pages,
                &domain_dir.to_string_lossy(),
            ) {
                eprintln!("warning: failed to write initial crawl status: {error}");
            }
            eprintln!(
                "\n  {green}{bold}✓ crawl started{reset}  {bold}{cyan}{url}{reset}\n\
                 \n\
                 {dim}  docs{reset}    {pink}{}{reset}\n\
                 {dim}  config{reset}  depth {}  ·  up to {} pages  ·  {} concurrent{}\n\
                 {dim}  status{reset}  noxa --status {url}\n\
                 {dim}  log{reset}     {}{reset}\n\
                 {dim}  pid{reset}     {dim}{}{reset}\n",
                domain_dir.display(),
                resolved.depth,
                resolved.max_pages,
                resolved.concurrency,
                if resolved.use_sitemap {
                    "  ·  sitemap"
                } else {
                    ""
                },
                log_path.display(),
                child.id()
            );
        }
        Err(e) => eprintln!("error spawning background crawl: {e}"),
    }
}

pub(crate) async fn run_crawl(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<(), String> {
    let url = cli
        .urls
        .first()
        .ok_or("--crawl requires a URL argument")
        .map(|u| normalize_url(u))?;
    let url = url.as_str();

    if cli.file.is_some() || cli.stdin {
        return Err("--crawl cannot be used with --file or --stdin".into());
    }

    let include_patterns = resolved.include_paths.clone();
    let exclude_patterns = resolved.exclude_paths.clone();

    // Set up streaming progress channel
    let (progress_tx, mut progress_rx) = tokio::sync::broadcast::channel::<PageResult>(100);

    // Set up cancel flag for Ctrl+C handling
    let cancel_flag = Arc::new(AtomicBool::new(false));

    // Register Ctrl+C handler when --crawl-state is set
    let state_path = cli.crawl_state.clone();
    if state_path.is_some() {
        let flag = Arc::clone(&cancel_flag);
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            flag.store(true, Ordering::Relaxed);
            eprintln!("\nCtrl+C received, saving crawl state...");
        });
    }

    let config = CrawlConfig {
        fetch: build_fetch_config(cli, resolved),
        max_depth: resolved.depth,
        max_pages: resolved.max_pages,
        concurrency: resolved.concurrency.max(1),
        delay: std::time::Duration::from_millis(resolved.delay),
        path_prefix: resolved.path_prefix.clone(),
        use_sitemap: resolved.use_sitemap,
        include_patterns,
        exclude_patterns,
        progress_tx: Some(progress_tx),
        cancel_flag: Some(Arc::clone(&cancel_flag)),
        extraction_options: build_extraction_options(resolved),
        body_retention: BodyRetention::Full,
    };

    // Load resume state if --crawl-state file exists
    let resume_state = state_path
        .as_ref()
        .and_then(|p| Crawler::load_state(p))
        .inspect(|s| {
            eprintln!(
                "Resuming crawl: {} pages already visited, {} URLs in frontier",
                s.visited.len(),
                s.frontier.len(),
            );
        });

    let max_pages = resolved.max_pages;
    let completed_offset = resume_state.as_ref().map_or(0, |s| s.completed_pages);

    // Compute docs dir once — used for status file and final summary
    let domain_dir = crawl_domain_dir(url, resolved.output_dir.as_deref());
    let docs_dir_str = domain_dir.to_string_lossy().to_string();

    // Status file: ~/.noxa/crawls/<domain>.json — updated each page
    let status_path = crawl_status_path(url);
    let start_time = std::time::Instant::now();
    write_initial_crawl_status(
        &status_path,
        url,
        std::process::id(),
        max_pages,
        &docs_dir_str,
    )
    .map_err(|e| format!("failed to write crawl status: {e}"))?;

    let status_path_bg = status_path.clone();
    let url_for_status = url.to_string();
    let docs_dir_bg = docs_dir_str.clone();
    let print_progress = cli.wait;

    // Spawn background task to print streaming progress to stderr
    let progress_handle = tokio::spawn(async move {
        let mut count = completed_offset;
        let mut ok = 0usize;
        let mut errors = 0usize;
        loop {
            let page = match progress_rx.recv().await {
                Ok(page) => page,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    eprintln!("warning: crawl progress consumer lagged; skipped {skipped} updates");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            count += 1;
            if page.error.is_some() {
                errors += 1;
            } else {
                ok += 1;
            }
            if print_progress {
                eprintln!("{}", format_progress(&page, count, max_pages));
            }
            let status = build_crawl_status(
                &url_for_status,
                std::process::id(),
                CrawlStatusPhase::Running,
                count,
                ok,
                errors,
                max_pages,
                Some(&page.url),
                start_time.elapsed().as_secs_f64(),
                &docs_dir_bg,
                0,
                0,
            );
            if let Err(error) = write_crawl_status_async(status_path_bg.clone(), status).await {
                eprintln!("warning: failed to update crawl status: {error}");
            }
        }
    });

    let crawler = Crawler::new(url, config).map_err(|e| format!("crawler error: {e}"))?;
    let result = crawler.crawl(url, resume_state).await;

    // Drop the crawler (and its progress_tx clone) so the progress task finishes
    drop(crawler);
    let _ = progress_handle.await;

    // Mark crawl done in status file
    let final_words: usize = result
        .pages
        .iter()
        .filter_map(|p| p.extraction.as_ref())
        .map(|e| e.metadata.word_count)
        .sum();
    let final_status = build_crawl_status(
        url,
        std::process::id(),
        CrawlStatusPhase::Done,
        result.total,
        result.ok,
        result.errors,
        max_pages,
        None,
        result.elapsed_secs,
        &docs_dir_str,
        result.excluded,
        final_words,
    );
    write_crawl_status_async(status_path.clone(), final_status)
        .await
        .map_err(|e| format!("failed to finalize crawl status: {e}"))?;

    // If cancelled via Ctrl+C and --crawl-state is set, save state for resume
    let was_cancelled = cancel_flag.load(Ordering::Relaxed);
    if was_cancelled {
        if let Some(ref path) = state_path {
            Crawler::save_state(
                path,
                url,
                &result.visited,
                &result.remaining_frontier,
                completed_offset + result.pages.len(),
                resolved.max_pages,
                resolved.depth,
            )?;
            eprintln!(
                "Crawl state saved to {} ({} pages completed). Resume with --crawl-state {}",
                path.display(),
                completed_offset + result.pages.len(),
                path.display(),
            );
        }
    } else if let Some(ref path) = state_path {
        // Crawl completed normally — clean up state file
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }

    // Log per-page errors and extraction warnings to stderr
    for page in &result.pages {
        if let Some(ref err) = page.error {
            eprintln!("error: {} -- {}", page.url, err);
        } else if let Some(ref extraction) = page.extraction {
            let reason = detect_empty(extraction);
            if !matches!(reason, EmptyReason::None) {
                warn_empty(&page.url, &reason);
            }
        }
    }

    // ContentStore auto-persisted every page during the crawl.
    // Show where they landed; if no output_dir is configured, just print to stdout.
    if !cli.no_store {
        let saved = result
            .pages
            .iter()
            .filter(|p| p.extraction.is_some())
            .count();
        let total_words: usize = result
            .pages
            .iter()
            .filter_map(|p| p.extraction.as_ref())
            .map(|e| e.metadata.word_count)
            .sum();
        let words_str = if total_words >= 1_000_000 {
            format!("~{:.1}M words", total_words as f64 / 1_000_000.0)
        } else if total_words >= 1_000 {
            format!("~{}k words", total_words / 1_000)
        } else {
            format!("{total_words} words")
        };
        let pages_str = if saved >= max_pages {
            format!("{bold}{saved}{reset}{dim}/{max_pages} (capped){reset}")
        } else {
            format!("{bold}{saved}{reset}{dim}/{max_pages}{reset}")
        };
        let err_part = if result.errors > 0 {
            format!("  {yellow}{} errors{reset}", result.errors)
        } else {
            String::new()
        };
        let excl_part = if result.excluded > 0 {
            format!("  {dim}{} excluded{reset}", result.excluded)
        } else {
            String::new()
        };
        eprintln!(
            "\n  {green}{bold}✓{reset} {pages_str} pages  {dim}{words_str}{reset}  {pink}{}{reset}  {dim}{:.1}s{reset}{err_part}{excl_part}\n",
            domain_dir.display(),
            result.elapsed_secs,
        );
    } else {
        print_crawl_output(&result, &resolved.format, resolved.metadata);
    }

    // Fire webhook on crawl complete and await delivery with a bounded timeout.
    if let Some(ref webhook_url) = cli.webhook {
        let urls: Vec<&str> = result.pages.iter().map(|p| p.url.as_str()).collect();
        let handle = fire_webhook(
            webhook_url,
            &serde_json::json!({
                "event": "crawl_complete",
                "total": result.total,
                "ok": result.ok,
                "errors": result.errors,
                "elapsed_secs": result.elapsed_secs,
                "urls": urls,
            }),
        );
        let _ = tokio::time::timeout(std::time::Duration::from_secs(15), handle).await;
    }

    if result.errors > 0 {
        Err(format!(
            "{} of {} pages failed",
            result.errors, result.total
        ))
    } else {
        Ok(())
    }
}

pub(crate) async fn run_map(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<(), String> {
    let url = cli
        .urls
        .first()
        .ok_or("--map requires a URL argument")
        .map(|u| normalize_url(u))?;
    let url = url.as_str();

    let client = FetchClient::new(build_fetch_config(cli, resolved))
        .map_err(|e| format!("client error: {e}"))?;

    // map_site() calls sitemap::discover() and appends an Op::Map entry to ops_log.
    let entries = client.map_site(url).await?;

    if entries.is_empty() {
        eprintln!("no sitemap URLs found for {url}");
    } else {
        eprintln!("discovered {} URLs", entries.len());
    }

    print_map_output(&entries, &resolved.format);
    Ok(())
}

fn crawl_domain_dir(url: &str, output_dir: Option<&Path>) -> PathBuf {
    let store_root = content_store_root(output_dir);
    url::Url::parse(&normalize_url(url))
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.strip_prefix("www.").unwrap_or(h).replace('.', "_"))
        })
        .map(|domain| store_root.join(domain))
        .unwrap_or(store_root)
}
