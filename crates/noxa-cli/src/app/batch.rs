use super::*;

pub(crate) async fn run_batch(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    entries: &[(String, Option<String>)],
) -> Result<(), String> {
    let client = Arc::new(
        FetchClient::new(build_fetch_config(cli, resolved))
            .map_err(|e| format!("client error: {e}"))?,
    );

    let urls: Vec<&str> = entries.iter().map(|(u, _)| u.as_str()).collect();
    let options = build_extraction_options(resolved);
    let results = client
        .fetch_and_extract_batch_with_options(&urls, resolved.concurrency, &options)
        .await;

    let ok = results.iter().filter(|r| r.result.is_ok()).count();
    let errors = results.len() - ok;

    // Log errors and extraction warnings to stderr
    for r in &results {
        if let Err(ref e) = r.result {
            eprintln!("error: {} -- {}", r.url, e);
        } else if let Ok(ref extraction) = r.result {
            let reason = detect_empty(extraction);
            if !matches!(reason, EmptyReason::None) {
                warn_empty(&r.url, &reason);
            }
        }
    }

    print_batch_output(&results, &resolved.format, resolved.metadata);
    if !cli.no_store {
        let store_root = content_store_root(resolved.output_dir.as_deref());
        let saved = results.iter().filter(|r| r.result.is_ok()).count();
        eprintln!("Saved {saved} files to {}", store_root.display());
    }

    eprintln!(
        "Fetched {} URLs ({} ok, {} errors)",
        results.len(),
        ok,
        errors
    );

    // Fire webhook on batch complete
    if let Some(ref webhook_url) = cli.webhook {
        let urls: Vec<&str> = results.iter().map(|r| r.url.as_str()).collect();
        fire_webhook(
            webhook_url,
            &serde_json::json!({
                "event": "batch_complete",
                "total": results.len(),
                "ok": ok,
                "errors": errors,
                "urls": urls,
            }),
        );
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    if errors > 0 {
        Err(format!("{errors} of {} URLs failed", results.len()))
    } else {
        Ok(())
    }
}
