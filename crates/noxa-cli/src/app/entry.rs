use super::*;
use crate::setup;

pub(crate) async fn run() {
    dotenvy::dotenv().ok();

    if matches!(std::env::args().nth(1).as_deref(), Some("setup")) {
        setup::run();
        return;
    }

    if matches!(std::env::args().nth(1).as_deref(), Some("mcp")) {
        init_mcp_logging();

        if let Err(e) = noxa_mcp::run().await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // Use low-level API to get both typed Cli and ArgMatches for ValueSource detection.
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    // Load config BEFORE init_logging so verbose from config takes effect.
    let cfg = config::NoxaConfig::load(cli.config.as_deref());
    let resolved = config::resolve(&cli, &matches, &cfg);

    init_logging(resolved.verbose);

    // Validate webhook URL early so any SSRF attempt is rejected before operations run.
    if let Some(ref webhook_url) = cli.webhook
        && let Err(e) = validate_url(webhook_url).await
    {
        eprintln!("error: invalid webhook URL: {e}");
        process::exit(1);
    }

    // --map: sitemap discovery mode
    if cli.map {
        if let Err(e) = run_map(&cli, &resolved).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    if let Some(ref domain) = cli.status {
        run_status(domain);
        return;
    }

    if let Some(ref domain) = cli.refresh {
        if let Err(e) = run_refresh(domain, &cli, &resolved).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    if let Some(ref query) = cli.retrieve {
        run_retrieve(query, content_store_root(resolved.output_dir.as_deref()));
        return;
    }

    // --crawl: recursive crawl mode
    if cli.crawl {
        if cli.wait {
            // Block and stream live progress
            if let Err(e) = run_crawl(&cli, &resolved).await {
                eprintln!("error: {e}");
                process::exit(1);
            }
        } else {
            // Background mode: re-exec self with --wait, detach
            spawn_crawl_background(&cli, &resolved);
        }
        return;
    }

    // --watch: poll URL(s) for changes
    if cli.watch {
        let watch_urls: Vec<String> = match collect_urls(&cli) {
            Ok(entries) => entries.into_iter().map(|(url, _)| url).collect(),
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        };
        if let Err(e) = run_watch(&cli, &resolved, &watch_urls).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // --diff-with: change tracking mode
    if let Some(ref snapshot_path) = cli.diff_with {
        if let Err(e) = run_diff(&cli, &resolved, snapshot_path).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // --brand: brand identity extraction mode
    if cli.brand {
        if let Err(e) = run_brand(&cli, &resolved).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // --research: deep research via cloud API
    if let Some(ref query) = cli.research {
        if let Err(e) = run_research(&cli, &resolved, query).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    if let Some(ref filter) = cli.list {
        run_list(filter, content_store_root(resolved.output_dir.as_deref()));
        return;
    }

    if let Some(ref pattern) = cli.grep {
        run_grep(pattern, content_store_root(resolved.output_dir.as_deref()));
        return;
    }

    if let Some(ref query) = cli.search {
        let fetch_client = Arc::new(
            noxa_fetch::FetchClient::new(build_fetch_config(&cli, &resolved)).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                process::exit(1);
            }),
        );
        if let Err(e) = run_search(&cli, &fetch_client, &resolved, query).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // Collect all URLs from args + --urls-file
    let entries = match collect_urls(&cli) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // LLM modes: --extract-json, --extract-prompt, --summarize
    // When multiple URLs are provided, run batch LLM extraction over all of them.
    if has_llm_flags(&cli) {
        if entries.len() > 1 {
            if let Err(e) = run_batch_llm(&cli, &resolved, &entries).await {
                eprintln!("error: {e}");
                process::exit(1);
            }
        } else if let Err(e) = run_llm(&cli, &resolved).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // Multi-URL batch mode
    if entries.len() > 1 {
        if let Err(e) = run_batch(&cli, &resolved, &entries).await {
            eprintln!("error: {e}");
            process::exit(1);
        }
        return;
    }

    // --raw-html: skip extraction, dump the fetched HTML
    if resolved.raw_html {
        match fetch_html(&cli, &resolved).await {
            Ok(r) => println!("{}", r.html),
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
        return;
    }

    // Single-page extraction (handles both HTML and PDF via content-type detection)
    match fetch_and_extract(&cli, &resolved).await {
        Ok(FetchOutput::Local(result)) => {
            print_output(&result, &resolved.format, resolved.metadata);
            if !cli.no_store {
                let url = cli
                    .urls
                    .first()
                    .map(|u| normalize_url(u))
                    .unwrap_or_default();
                let content = format_output(&result, &resolved.format, resolved.metadata);
                let store_root = content_store_root(resolved.output_dir.as_deref());
                let dest = store_root
                    .join(url_to_store_path(&url))
                    .with_extension("md");
                print_save_hint(&dest, &content);
            }
        }
        Ok(FetchOutput::Cloud(resp)) => {
            print_cloud_output(&resp, &resolved.format);
            if !cli.no_store {
                let url = cli
                    .urls
                    .first()
                    .map(|u| normalize_url(u))
                    .unwrap_or_default();
                let content = format_cloud_output(&resp, &resolved.format);
                let store_root = content_store_root(resolved.output_dir.as_deref());
                let dest = store_root
                    .join(url_to_store_path(&url))
                    .with_extension("md");
                print_save_hint(&dest, &content);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}
