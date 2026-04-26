use std::io::Write as _;
use std::path::PathBuf;

pub(crate) const CRAWL_WATCH: &str = "crawl-watch";
pub(crate) const RAG_WATCH: &str = "rag-watch";
pub(crate) const STORE_WATCH: &str = "store-watch";

fn pid_path(name: &str) -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".noxa")
        .join(format!("{name}.pid"))
}

/// RAII guard for a singleton lock file. Removes the pid file on drop so
/// process crashes (panics, early returns that drop the guard) do not leave
/// stale locks behind.
#[must_use = "SingletonGuard must be held for the lifetime of the singleton — dropping it releases the lock"]
pub(crate) struct SingletonGuard {
    path: PathBuf,
}

impl Drop for SingletonGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Attempts to acquire the singleton lock for `name`.
///
/// Returns `Some(SingletonGuard)` if this process won the lock — the caller
/// must hold the guard for the duration of the critical section. The pid file
/// is removed when the guard is dropped.
///
/// Returns `None` if another live instance already holds the lock — the
/// caller should exit.
pub(crate) fn acquire(name: &str) -> Option<SingletonGuard> {
    let path = pid_path(name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let pid_str = std::process::id().to_string();

    // Attempt atomic exclusive create (O_EXCL) — no TOCTOU between check and
    // open.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut f) => {
            let _ = f.write_all(pid_str.as_bytes());
            return Some(SingletonGuard { path });
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return None,
    }

    // File already exists — check whether the owner is still alive.
    if let Ok(contents) = std::fs::read_to_string(&path)
        && let Ok(pid) = contents.trim().parse::<u32>()
        && super::is_pid_running(pid)
    {
        return None;
    }

    // Stale PID — overwrite and take ownership. The guard will clean up on
    // drop.
    if std::fs::write(&path, pid_str).is_err() {
        return None;
    }
    Some(SingletonGuard { path })
}
