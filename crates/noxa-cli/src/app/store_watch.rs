use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use tokio::time::sleep;

const THRESHOLDS_GB: &[u64] = &[1, 5, 10];
const GROWTH_SPIKE_MB: u64 = 500;
const MB: u64 = 1 << 20;
const GB: u64 = 1 << 30;

fn dir_size_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                dir_size_bytes(&p)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

pub(crate) async fn run_store_watch() {
    let Some(_singleton) = super::watch_singleton::acquire(super::watch_singleton::STORE_WATCH)
    else {
        return;
    };

    let store_root = dirs::home_dir()
        .unwrap_or_default()
        .join(".noxa")
        .join("content");

    let mut prev_bytes = dir_size_bytes(&store_root);
    let mut announced_thresholds: HashSet<u64> = THRESHOLDS_GB
        .iter()
        .copied()
        .filter(|&gb| prev_bytes >= gb * GB)
        .collect();

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                return;
            }
            _ = sleep(Duration::from_secs(60)) => {}
        }

        let root = store_root.clone();
        let current_bytes = tokio::task::spawn_blocking(move || dir_size_bytes(&root))
            .await
            .unwrap_or(0);

        let delta = current_bytes.saturating_sub(prev_bytes);
        if delta >= GROWTH_SPIKE_MB * MB {
            println!(
                "Content store grew by {:.0}MB in the last 60s (total: {:.1}GB at {})",
                delta as f64 / MB as f64,
                current_bytes as f64 / GB as f64,
                store_root.display()
            );
        }

        for &gb in THRESHOLDS_GB {
            if current_bytes >= gb * GB && announced_thresholds.insert(gb) {
                println!(
                    "Content store has reached {}GB ({:.1}GB used at {})",
                    gb,
                    current_bytes as f64 / GB as f64,
                    store_root.display()
                );
            }
        }

        prev_bytes = current_bytes;
    }
}
