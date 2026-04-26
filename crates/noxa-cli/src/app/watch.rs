use super::*;

pub(crate) fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (now % 86400) / 3600;
    let minutes = (now % 3600) / 60;
    let seconds = now % 60;
    // Append "UTC" so users are not misled into thinking this is local time.
    format!("{hours:02}:{minutes:02}:{seconds:02} UTC")
}

/// Shared HTTP client for webhook delivery — built once and reused across all
/// webhook calls to avoid paying TLS/connector setup on every event.
static WEBHOOK_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn webhook_client() -> reqwest::Client {
    WEBHOOK_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("webhook reqwest::Client build should not fail")
        })
        .clone()
}

/// Fire a webhook POST with a JSON payload.
/// Returns a `JoinHandle` so callers can await delivery with a bounded timeout.
/// Auto-detects Discord and Slack webhook URLs and wraps the payload accordingly.
///
/// The webhook URL is **not** logged verbatim to avoid leaking embedded tokens
/// (Discord/Slack webhook tokens appear in the URL path).
pub(crate) fn fire_webhook(url: &str, payload: &serde_json::Value) -> tokio::task::JoinHandle<()> {
    let url = url.to_string();
    let is_discord = url.contains("discord.com/api/webhooks");
    let is_slack = url.contains("hooks.slack.com");

    // Derive a safe display string: scheme + host only, no path/query/token.
    let safe_host = url::Url::parse(&url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| "[redacted]".to_string());

    let body = if is_discord {
        let event = payload
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("notification");
        let details = serde_json::to_string_pretty(payload).unwrap_or_default();
        serde_json::json!({
            "embeds": [{
                "title": format!("noxa: {event}"),
                "description": format!("```json\n{details}\n```"),
                "color": 5814783
            }]
        })
        .to_string()
    } else if is_slack {
        let event = payload
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("notification");
        let details = serde_json::to_string_pretty(payload).unwrap_or_default();
        serde_json::json!({
            "text": format!("*noxa: {event}*\n```{details}```")
        })
        .to_string()
    } else {
        serde_json::to_string(payload).unwrap_or_default()
    };

    let client = webhook_client();
    tokio::spawn(async move {
        match client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
        {
            Ok(resp) => {
                eprintln!("[webhook] POST {} -> {}", safe_host, resp.status());
            }
            Err(e) => eprintln!("[webhook] POST failed: {e}"),
        }
    })
}

pub(crate) async fn run_watch(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    urls: &[String],
) -> Result<(), String> {
    if urls.is_empty() {
        return Err("--watch requires at least one URL".into());
    }
    if cli.watch_interval == 0 {
        return Err("--watch-interval must be at least 1 second".into());
    }

    let client = Arc::new(
        FetchClient::new(build_fetch_config(cli, resolved))
            .map_err(|e| format!("client error: {e}"))?,
    );
    let options = build_extraction_options(resolved);

    // Ctrl+C handler
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        flag.store(true, Ordering::Relaxed);
    });

    // Single-URL mode: preserve original behavior exactly
    if urls.len() == 1 {
        return run_watch_single(cli, resolved, &client, &options, &urls[0], &cancelled).await;
    }

    // Multi-URL mode: batch fetch, diff each, report aggregate
    run_watch_multi(cli, resolved, &client, &options, urls, &cancelled).await
}

const WATCH_ON_CHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

fn parse_on_change_command(cmd: &str) -> Result<Vec<String>, String> {
    let argv = shlex::split(cmd)
        .ok_or_else(|| "failed to parse command: invalid shell-style quoting".to_string())?;
    if argv.is_empty() {
        return Err("failed to run command: command is empty".to_string());
    }
    Ok(argv)
}

pub(crate) async fn run_on_change_command(
    cmd: &str,
    payload: &str,
    max_runtime: std::time::Duration,
) -> Result<(), String> {
    let argv = parse_on_change_command(cmd)?;
    let mut command = tokio::process::Command::new(&argv[0]);
    command.args(&argv[1..]);
    command.stdin(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to run command: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| format!("failed to write command stdin: {e}"))?;
        // Explicitly close stdin so the child sees EOF.  The implicit drop at the
        // end of the if-let would also work, but being explicit avoids a future
        // refactor accidentally keeping the write-end open and deadlocking
        // commands that wait for EOF (e.g. `cat`, `jq`).
        drop(stdin);
    }

    match tokio::time::timeout(max_runtime, child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(format!("command exited with status {status}")),
        Ok(Err(e)) => Err(format!("failed to wait for command: {e}")),
        Err(_) => {
            let _ = child.kill().await;
            match child.wait().await {
                Ok(_) => Err(format!(
                    "command timed out after {}s",
                    max_runtime.as_secs()
                )),
                Err(e) => Err(format!("command timed out and could not be reaped: {e}")),
            }
        }
    }
}

/// Original single-URL watch loop -- backward compatible.
pub(crate) async fn run_watch_single(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    client: &Arc<FetchClient>,
    options: &ExtractionOptions,
    url: &str,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    // Watch restart continuity: try to restore the last stored snapshot as the
    // baseline instead of always doing a fresh fetch on startup.
    // Reuse the client's store instead of creating a redundant instance.
    let store = client.store();

    let (mut previous, mut is_initial_baseline) = if let Some(s) = store {
        match s.read(url).await {
            Ok(Some(stored)) => {
                eprintln!(
                    "[watch] Restored baseline from store: {url} ({} words)",
                    stored.metadata.word_count
                );
                (stored, false)
            }
            _ => {
                let fetched = client
                    .fetch_and_extract_with_options(url, options)
                    .await
                    .map_err(|e| format!("initial fetch failed: {e}"))?;
                eprintln!(
                    "[watch] Initial snapshot: {url} ({} words)",
                    fetched.metadata.word_count
                );
                (fetched, true)
            }
        }
    } else {
        let fetched = client
            .fetch_and_extract_with_options(url, options)
            .await
            .map_err(|e| format!("initial fetch failed: {e}"))?;
        eprintln!(
            "[watch] Initial snapshot: {url} ({} words)",
            fetched.metadata.word_count
        );
        (fetched, false)
    };

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(cli.watch_interval)).await;

        if cancelled.load(Ordering::Relaxed) {
            eprintln!("[watch] Stopped");
            break;
        }

        let current = match client.fetch_and_extract_with_options(url, options).await {
            Ok(result) => result,
            Err(e) => {
                eprintln!("[watch] Fetch error ({}): {e}", timestamp());
                continue;
            }
        };

        let diff = noxa_core::diff::diff(&previous, &current);

        if diff.status == ChangeStatus::Same {
            eprintln!("[watch] No changes ({})", timestamp());
            is_initial_baseline = false;
        } else {
            print_diff_output(&diff, &resolved.format);
            eprintln!("[watch] Changes detected! ({})", timestamp());

            // Append change to ops log.
            let watch_ops_log = client.ops_log().cloned();
            log_operation(
                &watch_ops_log,
                url,
                Op::Diff,
                || {
                    serde_json::json!({
                        "source": "watch",
                        "interval_secs": cli.watch_interval
                    })
                },
                || serde_json::to_value(&diff).unwrap_or(serde_json::Value::Null),
            )
            .await;

            // is_initial_baseline suppresses --on-change on the first reconciliation
            // write when there was no stored snapshot (avoids spurious triggers on startup).
            if !is_initial_baseline {
                if let Some(ref cmd) = cli.on_change {
                    let diff_json = serde_json::to_string(&diff).unwrap_or_default();
                    eprintln!("[watch] Running: {cmd}");
                    if let Err(e) =
                        run_on_change_command(cmd, &diff_json, WATCH_ON_CHANGE_TIMEOUT).await
                    {
                        eprintln!("[watch] Failed to run command: {e}");
                    }
                }

                if let Some(ref webhook_url) = cli.webhook {
                    // Fire-and-forget: watch loop continues immediately without waiting for delivery.
                    let _handle = fire_webhook(
                        webhook_url,
                        &serde_json::json!({
                            "event": "watch_change",
                            "url": url,
                            "status": format!("{:?}", diff.status),
                            "word_count_delta": diff.word_count_delta,
                            "metadata_changes": diff.metadata_changes.len(),
                            "links_added": diff.links_added.len(),
                            "links_removed": diff.links_removed.len(),
                        }),
                    );
                }
            }

            is_initial_baseline = false;
            previous = current;
        }
    }

    Ok(())
}

/// Multi-URL watch loop -- batch fetch all URLs, diff each, report aggregate.
pub(crate) async fn run_watch_multi(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    client: &Arc<FetchClient>,
    options: &ExtractionOptions,
    urls: &[String],
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    let url_refs: Vec<&str> = urls.iter().map(|u| u.as_str()).collect();

    // Initial pass: fetch all URLs in parallel
    let initial_results = client
        .fetch_and_extract_batch_with_options(&url_refs, resolved.concurrency, options)
        .await;

    let mut snapshots = std::collections::HashMap::new();
    let mut ok_count = 0usize;
    let mut err_count = 0usize;

    for r in initial_results {
        match r.result {
            Ok(extraction) => {
                snapshots.insert(r.url, extraction);
                ok_count += 1;
            }
            Err(e) => {
                eprintln!("[watch] Initial fetch error: {} -- {e}", r.url);
                err_count += 1;
            }
        }
    }

    eprintln!(
        "[watch] Watching {} URLs (interval: {}s)",
        urls.len(),
        cli.watch_interval
    );
    eprintln!("[watch] Initial snapshots: {ok_count} ok, {err_count} errors");

    let mut check_number = 0u64;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(cli.watch_interval)).await;

        if cancelled.load(Ordering::Relaxed) {
            eprintln!("[watch] Stopped");
            break;
        }

        check_number += 1;

        let current_results = client
            .fetch_and_extract_batch_with_options(&url_refs, resolved.concurrency, options)
            .await;

        let mut changed: Vec<serde_json::Value> = Vec::new();
        let mut same_count = 0usize;
        let mut fetch_errors = 0usize;

        for r in current_results {
            match r.result {
                Ok(current) => {
                    if let Some(previous) = snapshots.get(&r.url) {
                        let diff = noxa_core::diff::diff(previous, &current);
                        if diff.status == ChangeStatus::Same {
                            same_count += 1;
                        } else {
                            changed.push(serde_json::json!({
                                "url": r.url,
                                "word_count_delta": diff.word_count_delta,
                            }));
                            snapshots.insert(r.url, current);
                        }
                    } else {
                        // URL failed initially, first successful fetch -- store as baseline
                        snapshots.insert(r.url, current);
                        same_count += 1;
                    }
                }
                Err(e) => {
                    eprintln!("[watch] Fetch error: {} -- {e}", r.url);
                    fetch_errors += 1;
                }
            }
        }

        let ts = timestamp();
        let err_suffix = if fetch_errors > 0 {
            format!(", {fetch_errors} errors")
        } else {
            String::new()
        };

        if changed.is_empty() {
            eprintln!(
                "[watch] Check {check_number} ({ts}): 0 changed, {same_count} same{err_suffix}"
            );
        } else {
            eprintln!(
                "[watch] Check {check_number} ({ts}): {} changed, {same_count} same{err_suffix}",
                changed.len(),
            );
            for entry in &changed {
                let url = entry["url"].as_str().unwrap_or("?");
                let delta = entry["word_count_delta"].as_i64().unwrap_or(0);
                eprintln!("  -> {url} (word delta: {delta:+})");
            }

            // Append each changed URL to ops log.
            let multi_ops_log = client.ops_log().cloned();
            for entry in &changed {
                if let Some(url) = entry["url"].as_str() {
                    log_operation(
                        &multi_ops_log,
                        url,
                        Op::Diff,
                        || {
                            serde_json::json!({
                            "source": "watch",
                            "interval_secs": cli.watch_interval,
                                "check_number": check_number
                            })
                        },
                        || entry.clone(),
                    )
                    .await;
                }
            }

            // Fire --on-change once with all changes
            if let Some(ref cmd) = cli.on_change {
                let payload = serde_json::json!({
                    "event": "watch_changes",
                    "check_number": check_number,
                    "total_urls": urls.len(),
                    "changed": changed.len(),
                    "same": same_count,
                    "changes": changed,
                });
                let payload_json = serde_json::to_string(&payload).unwrap_or_default();
                eprintln!("[watch] Running: {cmd}");
                if let Err(e) =
                    run_on_change_command(cmd, &payload_json, WATCH_ON_CHANGE_TIMEOUT).await
                {
                    eprintln!("[watch] Failed to run command: {e}");
                }
            }

            // Fire webhook once with aggregate payload
            if let Some(ref webhook_url) = cli.webhook {
                // Fire-and-forget: watch loop continues immediately without waiting for delivery.
                let _handle = fire_webhook(
                    webhook_url,
                    &serde_json::json!({
                        "event": "watch_changes",
                        "check_number": check_number,
                        "total_urls": urls.len(),
                        "changed": changed.len(),
                        "same": same_count,
                        "changes": changed,
                    }),
                );
            }
        }
    }

    Ok(())
}
