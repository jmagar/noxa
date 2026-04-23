use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::time::sleep;

use super::rag_daemon::{
    DEFAULT_QDRANT_URL, DEFAULT_TEI_URL, is_rag_daemon_running, spawn_rag_daemon,
};

const DEFAULT_COLLECTION: &str = "noxa_rag";
const STABLE_POLLS_REQUIRED: u32 = 3;
const ALERT_COOLDOWN: Duration = Duration::from_secs(300);

async fn probe_http(client: &reqwest::Client, url: &str) -> bool {
    client
        .get(url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn get_qdrant_point_count(
    client: &reqwest::Client,
    base_url: &str,
    collection: &str,
) -> Option<u64> {
    let resp = client
        .get(format!("{base_url}/collections/{collection}"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json.pointer("/result/points_count")
        .and_then(|v| v.as_u64())
}

async fn check_failed_jobs(path: &std::path::Path, prev_size: &mut u64) -> Vec<String> {
    let Ok(meta) = tokio::fs::metadata(path).await else {
        return vec![];
    };
    let current_size = meta.len();
    if current_size <= *prev_size {
        if current_size < *prev_size {
            *prev_size = 0;
        }
        return vec![];
    }

    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return vec![];
    };
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    if file.seek(std::io::SeekFrom::Start(*prev_size)).await.is_err() {
        return vec![];
    }
    let mut buf = String::new();
    // Cap the read at 1 MiB so a runaway log file cannot balloon memory.
    if AsyncReadExt::take(&mut file, 1 << 20).read_to_string(&mut buf).await.is_err() {
        return vec![];
    }
    // Advance by bytes actually consumed (may be < current_size when truncated).
    *prev_size = (*prev_size + buf.len() as u64).min(current_size);

    buf.lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            let path = super::sanitize_display(v.get("path")?.as_str()?);
            let error = super::sanitize_display(v.get("error")?.as_str()?);
            Some(format!("RAG index failure: {path} — {error}"))
        })
        .collect()
}

/// Emit an alert for a service transitioning offline, respecting the cooldown window.
/// Recovery alerts always fire immediately. Returns the new `up` state.
fn check_service_alert(
    key: &'static str,
    was_up: bool,
    now_up: bool,
    offline_msg: &str,
    online_msg: &str,
    alerted: &mut HashMap<&'static str, Instant>,
) -> bool {
    if now_up != was_up {
        if now_up {
            alerted.remove(key);
            println!("{online_msg}");
        } else {
            let should_alert = alerted.get(key).map_or(true, |t| t.elapsed() >= ALERT_COOLDOWN);
            if should_alert {
                alerted.insert(key, Instant::now());
                println!("{offline_msg}");
            }
        }
    }
    now_up
}

pub(crate) async fn run_rag_watch() {
    let Some(_singleton) = super::watch_singleton::acquire(super::watch_singleton::RAG_WATCH)
    else {
        return;
    };

    let tei_url = std::env::var("TEI_URL").unwrap_or_else(|_| DEFAULT_TEI_URL.into());
    let qdrant_url = std::env::var("QDRANT_URL").unwrap_or_else(|_| DEFAULT_QDRANT_URL.into());
    let collection =
        std::env::var("NOXA_RAG_COLLECTION").unwrap_or_else(|_| DEFAULT_COLLECTION.into());
    let failed_log = std::env::var("NOXA_RAG_FAILED_LOG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".noxa")
                .join("noxa-rag-failed.jsonl")
        });

    let tei_health = format!("{tei_url}/health");
    let qdrant_health = format!("{qdrant_url}/healthz");

    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("rag-watch: failed to build HTTP client: {e}");
            return;
        }
    };

    let mut daemon_running = tokio::task::spawn_blocking(is_rag_daemon_running)
        .await
        .unwrap_or(false);
    if !daemon_running {
        match spawn_rag_daemon() {
            Ok(pid) => {
                println!("RAG daemon was not running — started automatically (PID {pid})");
                daemon_running = true;
            }
            Err(e) => println!("RAG daemon is not running and could not be started: {e}"),
        }
    }

    let (mut tei_up, mut qdrant_up) =
        tokio::join!(probe_http(&client, &tei_health), probe_http(&client, &qdrant_health));

    if !tei_up {
        println!("TEI embeddings server is offline ({tei_url}) — RAG indexing will stall");
    }
    if !qdrant_up {
        println!("Qdrant is offline ({qdrant_url}) — RAG indexing and search will not work");
    }

    let mut last_point_count = get_qdrant_point_count(&client, &qdrant_url, &collection)
        .await
        .unwrap_or(0);
    let mut announced_count = last_point_count;
    let mut stable_polls: u32 = 0;

    let mut failed_log_size: u64 = tokio::fs::metadata(&failed_log)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let mut offline_alerted: HashMap<&'static str, Instant> = HashMap::new();

    let tei_offline = format!("TEI embeddings server is offline ({tei_url}) — RAG indexing will stall");
    let tei_online = format!("TEI embeddings server is back online ({tei_url})");
    let qdrant_offline = format!("Qdrant is offline ({qdrant_url}) — RAG indexing and search will not work");
    let qdrant_online = format!("Qdrant is back online ({qdrant_url})");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                return;
            }
            _ = sleep(Duration::from_secs(10)) => {}
        }

        let daemon_now = tokio::task::spawn_blocking(is_rag_daemon_running)
            .await
            .unwrap_or(false);
        if !daemon_now && daemon_running {
            println!("RAG daemon stopped unexpectedly — attempting restart");
            match spawn_rag_daemon() {
                Ok(pid) => {
                    println!("RAG daemon restarted (PID {pid})");
                    daemon_running = true;
                }
                Err(e) => {
                    println!("RAG daemon restart failed: {e}");
                    daemon_running = false;
                }
            }
        } else {
            daemon_running = daemon_now;
        }

        let (tei_now, qdrant_now) =
            tokio::join!(probe_http(&client, &tei_health), probe_http(&client, &qdrant_health));

        tei_up = check_service_alert(
            "tei", tei_up, tei_now, &tei_offline, &tei_online, &mut offline_alerted,
        );
        qdrant_up = check_service_alert(
            "qdrant", qdrant_up, qdrant_now, &qdrant_offline, &qdrant_online, &mut offline_alerted,
        );

        if qdrant_up {
            if let Some(count) = get_qdrant_point_count(&client, &qdrant_url, &collection).await {
                if count > last_point_count {
                    stable_polls = 0;
                } else if count == last_point_count && count > announced_count {
                    stable_polls += 1;
                    if stable_polls >= STABLE_POLLS_REQUIRED {
                        let delta = count - announced_count;
                        println!(
                            "RAG indexing complete: {collection} — {count} points (+{delta} new)"
                        );
                        announced_count = count;
                        stable_polls = 0;
                    }
                }
                last_point_count = count;
            }
        }

        for msg in check_failed_jobs(&failed_log, &mut failed_log_size).await {
            println!("{msg}");
        }
    }
}
