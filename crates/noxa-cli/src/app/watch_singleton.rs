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

/// Returns true if this process should run (i.e. it won the singleton lock).
/// Returns false if another instance is already running — caller should exit.
pub(crate) fn acquire(name: &str) -> bool {
    let path = pid_path(name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let pid_str = std::process::id().to_string();

    // Attempt atomic exclusive create (O_EXCL).
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut f) => {
            let _ = f.write_all(pid_str.as_bytes());
            return true;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return false,
    }

    // File already exists — check whether the owner is still alive.
    if let Ok(contents) = std::fs::read_to_string(&path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            if super::is_pid_running(pid) {
                return false;
            }
        }
    }

    // Stale PID — overwrite and take ownership.
    let _ = std::fs::write(&path, pid_str);
    true
}

pub(crate) fn release(name: &str) {
    let _ = std::fs::remove_file(pid_path(name));
}
