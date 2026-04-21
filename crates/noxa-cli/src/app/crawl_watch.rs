use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use tokio::time::sleep;

use super::*;

const ERROR_RATE_THRESHOLD: f64 = 0.20;
const ALERT_COOLDOWN: Duration = Duration::from_secs(300);

pub(crate) async fn run_crawl_watch() {
    if !super::watch_singleton::acquire(super::watch_singleton::CRAWL_WATCH) {
        return;
    }

    let crawls_dir = crawl_status_dir();

    let mut seen: HashMap<String, CrawlStatusPhase> = HashMap::new();
    let mut stale_announced: HashSet<String> = HashSet::new();
    let mut prev_error_pct: HashMap<String, u32> = HashMap::new();
    let mut error_last_alerted: HashMap<String, Instant> = HashMap::new();
    let mut finished: HashSet<String> = HashSet::new();

    if let Ok(entries) = std::fs::read_dir(&crawls_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(record) = read_crawl_status(&path) {
                let key = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                seen.insert(key.clone(), record.phase);
                if record.phase == CrawlStatusPhase::Done {
                    finished.insert(key.clone());
                }
                if record.phase == CrawlStatusPhase::Running && !is_pid_running(record.pid) {
                    stale_announced.insert(key);
                }
            }
        }
    }

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                super::watch_singleton::release(super::watch_singleton::CRAWL_WATCH);
                return;
            }
            _ = sleep(Duration::from_secs(5)) => {}
        }

        let dir_entries = match std::fs::read_dir(&crawls_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut keys_on_disk: HashSet<String> = HashSet::new();

        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let record = match read_crawl_status(&path) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let key = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            keys_on_disk.insert(key.clone());

            if finished.contains(&key) {
                continue;
            }
            let prev = seen.get(&key).copied();

            match (prev, record.phase) {
                (Some(CrawlStatusPhase::Running) | None, CrawlStatusPhase::Done) => {
                    let url = sanitize_display(&record.url);
                    let docs_dir = sanitize_display(&record.docs_dir);
                    let mut msg = format!(
                        "Crawl complete: {} — {} pages, {} words ({:.1}s)",
                        url, record.pages_ok, record.total_words, record.elapsed_secs
                    );
                    if record.pages_errors > 0 {
                        msg.push_str(&format!(", {} errors", record.pages_errors));
                    }
                    msg.push_str(&format!(". Docs saved to: {docs_dir}"));
                    println!("{msg}");
                    finished.insert(key.clone());
                }

                (_, CrawlStatusPhase::Running) => {
                    let url = sanitize_display(&record.url);
                    if !is_pid_running(record.pid) && stale_announced.insert(key.clone()) {
                        println!(
                            "Crawl stalled: {} (PID {} is gone). Re-run with: noxa --crawl {}",
                            url, record.pid, url
                        );
                    }

                    if record.pages_done > 5 {
                        let pct = record.pages_errors as f64 / record.pages_done as f64;
                        let pct_rounded = (pct * 100.0) as u32;
                        let prev_pct = prev_error_pct.get(&key).copied().unwrap_or(0);
                        let cooldown_ok = error_last_alerted
                            .get(&key)
                            .map_or(true, |t| t.elapsed() >= ALERT_COOLDOWN);
                        if pct >= ERROR_RATE_THRESHOLD && pct_rounded > prev_pct && cooldown_ok {
                            println!(
                                "Crawl warning: {} — {}% error rate ({}/{} pages failed)",
                                url, pct_rounded, record.pages_errors, record.pages_done
                            );
                            prev_error_pct.insert(key.clone(), pct_rounded);
                            error_last_alerted.insert(key.clone(), Instant::now());
                        }
                    }
                }

                _ => {}
            }

            seen.insert(key, record.phase);
        }

        // Prune map entries for crawl files that no longer exist on disk.
        seen.retain(|k, _| keys_on_disk.contains(k));
        prev_error_pct.retain(|k, _| keys_on_disk.contains(k));
        stale_announced.retain(|k| keys_on_disk.contains(k));
        error_last_alerted.retain(|k, _| keys_on_disk.contains(k));
        finished.retain(|k| keys_on_disk.contains(k));
    }
}
