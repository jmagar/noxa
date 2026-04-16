use super::*;

pub(crate) fn crawl_status_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .map(|home| crawl_status_dir_from_home(&home))
        .unwrap_or_else(|| crawl_status_dir_from_home(Path::new(".")))
}

pub(crate) fn crawl_status_dir_from_home(home: &Path) -> PathBuf {
    home.join(".noxa").join("crawls")
}

pub(crate) fn crawl_status_key(input: &str) -> String {
    let normalized = normalize_url(input);
    let host = url::Url::parse(&normalized)
        .ok()
        .and_then(|parsed| parsed.host_str().map(ToOwned::to_owned))
        .or_else(|| {
            input
                .trim()
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .split('/')
                .next()
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_string());
    let sanitized: String = host
        .strip_prefix("www.")
        .unwrap_or(&host)
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
pub(crate) fn crawl_status_path_for_home(home: &Path, input: &str) -> PathBuf {
    crawl_status_dir_from_home(home).join(format!("{}.json", crawl_status_key(input)))
}

pub(crate) fn crawl_status_path(input: &str) -> PathBuf {
    crawl_status_dir().join(format!("{}.json", crawl_status_key(input)))
}

#[cfg(test)]
pub(crate) fn crawl_log_path_for_home(home: &Path, input: &str) -> PathBuf {
    crawl_status_dir_from_home(home).join(format!("{}.log", crawl_status_key(input)))
}

pub(crate) fn crawl_log_path(input: &str) -> PathBuf {
    crawl_status_dir().join(format!("{}.log", crawl_status_key(input)))
}

const CRAWL_STATUS_VERSION: u8 = 1;

pub(crate) fn crawl_status_version() -> u8 {
    CRAWL_STATUS_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CrawlStatusPhase {
    Running,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrawlStatusState {
    Running,
    Done,
    Stale,
    NeverStarted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CrawlStatusRecord {
    #[serde(default = "crawl_status_version")]
    pub(crate) version: u8,
    pub(crate) url: String,
    #[serde(default)]
    pub(crate) pid: u32,
    pub(crate) phase: CrawlStatusPhase,
    #[serde(default)]
    pub(crate) pages_done: usize,
    #[serde(default)]
    pub(crate) pages_ok: usize,
    #[serde(default)]
    pub(crate) pages_errors: usize,
    #[serde(default)]
    pub(crate) max_pages: usize,
    #[serde(default)]
    pub(crate) last_url: Option<String>,
    #[serde(default)]
    pub(crate) elapsed_secs: f64,
    #[serde(default)]
    pub(crate) docs_dir: String,
    #[serde(default)]
    pub(crate) excluded: usize,
    #[serde(default)]
    pub(crate) total_words: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LegacyCrawlStatusRecord {
    pub(crate) url: String,
    #[serde(default)]
    pub(crate) pid: u32,
    #[serde(default)]
    pub(crate) pages_done: usize,
    #[serde(default)]
    pub(crate) pages_ok: usize,
    #[serde(default)]
    pub(crate) pages_errors: usize,
    #[serde(default)]
    pub(crate) max_pages: usize,
    #[serde(default)]
    pub(crate) last_url: Option<String>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    pub(crate) elapsed_secs: f64,
    #[serde(default)]
    pub(crate) docs_dir: String,
    #[serde(default)]
    pub(crate) excluded: usize,
    #[serde(default)]
    pub(crate) total_words: usize,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum CrawlStatusOnDisk {
    Typed(CrawlStatusRecord),
    Legacy(LegacyCrawlStatusRecord),
}

impl From<LegacyCrawlStatusRecord> for CrawlStatusRecord {
    fn from(value: LegacyCrawlStatusRecord) -> Self {
        Self {
            version: crawl_status_version(),
            url: value.url,
            pid: value.pid,
            phase: if value.done {
                CrawlStatusPhase::Done
            } else {
                CrawlStatusPhase::Running
            },
            pages_done: value.pages_done,
            pages_ok: value.pages_ok,
            pages_errors: value.pages_errors,
            max_pages: value.max_pages,
            last_url: value.last_url,
            elapsed_secs: value.elapsed_secs,
            docs_dir: value.docs_dir,
            excluded: value.excluded,
            total_words: value.total_words,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_crawl_status(
    url: &str,
    pid: u32,
    phase: CrawlStatusPhase,
    pages_done: usize,
    pages_ok: usize,
    pages_errors: usize,
    max_pages: usize,
    last_url: Option<&str>,
    elapsed_secs: f64,
    docs_dir: &str,
    excluded: usize,
    total_words: usize,
) -> CrawlStatusRecord {
    CrawlStatusRecord {
        version: crawl_status_version(),
        url: url.to_string(),
        pid,
        phase,
        pages_done,
        pages_ok,
        pages_errors,
        max_pages,
        last_url: last_url.map(ToOwned::to_owned),
        elapsed_secs: (elapsed_secs * 10.0).round() / 10.0,
        docs_dir: docs_dir.to_string(),
        excluded,
        total_words,
    }
}

pub(crate) fn crawl_status_tmp_path(path: &Path) -> PathBuf {
    path.with_extension(format!("json.{}.tmp", std::process::id()))
}

pub(crate) fn encode_crawl_status(status: &CrawlStatusRecord) -> io::Result<Vec<u8>> {
    serde_json::to_vec_pretty(status)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(crate) fn write_crawl_status_sync(path: &Path, status: &CrawlStatusRecord) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = crawl_status_tmp_path(path);
    let bytes = encode_crawl_status(status)?;
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub(crate) async fn write_crawl_status_async(
    path: PathBuf,
    status: CrawlStatusRecord,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = crawl_status_tmp_path(&path);
    let bytes = encode_crawl_status(&status)?;
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, &path).await?;
    Ok(())
}

pub(crate) fn read_crawl_status(path: &Path) -> io::Result<CrawlStatusRecord> {
    let content = std::fs::read_to_string(path)?;
    let parsed = serde_json::from_str::<CrawlStatusOnDisk>(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(match parsed {
        CrawlStatusOnDisk::Typed(status) => status,
        CrawlStatusOnDisk::Legacy(status) => status.into(),
    })
}

pub(crate) fn classify_crawl_status(
    status: Option<&CrawlStatusRecord>,
    pid_running: bool,
) -> CrawlStatusState {
    match status {
        None => CrawlStatusState::NeverStarted,
        Some(status) => match status.phase {
            CrawlStatusPhase::Done => CrawlStatusState::Done,
            CrawlStatusPhase::Running if pid_running => CrawlStatusState::Running,
            CrawlStatusPhase::Running => CrawlStatusState::Stale,
        },
    }
}

pub(crate) fn kill_zero_probe(pid: libc::pid_t) -> io::Result<()> {
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(crate) fn is_pid_running_with<F>(pid: u32, proc_root: Option<&Path>, kill_probe: F) -> bool
where
    F: Fn(libc::pid_t) -> io::Result<()>,
{
    if pid == 0 {
        return false;
    }
    if let Some(root) = proc_root {
        return root.join(pid.to_string()).exists();
    }
    match kill_probe(pid as libc::pid_t) {
        Ok(()) => true,
        Err(error) => matches!(error.raw_os_error(), Some(code) if code == libc::EPERM),
    }
}

pub(crate) fn is_pid_running(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let proc_root = Path::new("/proc");
        if proc_root.exists() {
            return is_pid_running_with(pid, Some(proc_root), kill_zero_probe);
        }
    }

    is_pid_running_with(pid, None, kill_zero_probe)
}

pub(crate) fn write_initial_crawl_status(
    path: &Path,
    url: &str,
    pid: u32,
    max_pages: usize,
    docs_dir: &str,
) -> io::Result<()> {
    let status = build_crawl_status(
        url,
        pid,
        CrawlStatusPhase::Running,
        0,
        0,
        0,
        max_pages,
        None,
        0.0,
        docs_dir,
        0,
        0,
    );
    write_crawl_status_sync(path, &status)
}
