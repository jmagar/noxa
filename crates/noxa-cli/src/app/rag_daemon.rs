use std::path::PathBuf;

const DEFAULT_CONFIG: &str = ".noxa/noxa.toml";
const DEFAULT_LOG: &str = ".noxa/rag-daemon.log";
pub(crate) const DEFAULT_TEI_URL: &str = "http://localhost:52000";
pub(crate) const DEFAULT_QDRANT_URL: &str = "http://localhost:53333";

pub(crate) fn rag_config_path() -> PathBuf {
    std::env::var("NOXA_RAG_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(DEFAULT_CONFIG))
}

fn rag_daemon_bin() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        let candidate = exe.with_file_name("noxa-rag-daemon");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    which_bin("noxa-rag-daemon")
}

fn which_bin(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).find_map(|dir| {
            let candidate = dir.join(name);
            candidate.exists().then_some(candidate)
        })
    })
}

fn find_rag_daemon_pid() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        let Ok(procs) = std::fs::read_dir("/proc") else {
            return None;
        };
        for entry in procs.flatten() {
            let path = entry.path();
            let Some(pid_str) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(pid) = pid_str.parse::<u32>() else {
                continue;
            };
            let Ok(cmdline) = std::fs::read(path.join("cmdline")) else {
                continue;
            };
            // argv[0] is the first NUL-terminated token; compare only its basename.
            let argv0 = cmdline.split(|&b: &u8| b == 0).next().unwrap_or(&[]);
            let basename = argv0.rsplit(|&b: &u8| b == b'/').next().unwrap_or(argv0);
            if basename == b"noxa-rag-daemon" {
                return Some(pid);
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        let out = std::process::Command::new("pgrep")
            .args(["-x", "noxa-rag-daemon"])
            .output()
            .ok()?;
        std::str::from_utf8(&out.stdout)
            .ok()?
            .lines()
            .next()?
            .trim()
            .parse()
            .ok()
    }
}

pub(crate) fn is_rag_daemon_running() -> bool {
    find_rag_daemon_pid().is_some()
}

fn probe_service(url: &str) -> bool {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()
        .and_then(|c| c.get(url).send().ok())
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

pub(crate) fn spawn_rag_daemon() -> Result<u32, String> {
    let bin = rag_daemon_bin().ok_or_else(|| {
        let install_hint = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .map(|bin_dir| format!("cp {}/noxa-rag-daemon ~/.cargo/bin/", bin_dir.display()))
            .unwrap_or_else(|| {
                "cargo install --git https://github.com/jmagar/noxa --bin noxa-rag-daemon"
                    .to_string()
            });
        format!(
            "noxa-rag-daemon not found in PATH\n  \
             Build all binaries: cargo build --release  (from the noxa workspace)\n  \
             Then install:       {install_hint}"
        )
    })?;

    let config = rag_config_path();
    if !config.exists() {
        return Err(format!(
            "RAG config not found at {}\n  \
             Create it with: noxa setup\n  \
             Or copy the example: cp config/config.example.toml ~/.noxa/noxa.toml",
            config.display()
        ));
    }

    let log_path = dirs::home_dir().unwrap_or_default().join(DEFAULT_LOG);
    if let Some(p) = log_path.parent() {
        let _ = std::fs::create_dir_all(p);
    }

    let open_devnull = || std::fs::OpenOptions::new().write(true).open("/dev/null");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .or_else(|_| open_devnull())
        .map_err(|e| format!("failed to open log file: {e}"))?;
    let log_clone = log_file
        .try_clone()
        .or_else(|_| open_devnull())
        .map_err(|e| format!("failed to clone log fd: {e}"))?;

    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(["--config", &config.to_string_lossy()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(log_clone));

    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn noxa-rag-daemon: {e}"))?;
    Ok(child.id())
}

pub(crate) fn run_rag_start() {
    if let Some(pid) = find_rag_daemon_pid() {
        eprintln!(
            "\n  \x1b[33m\x1b[1m⚠ rag daemon already running\x1b[0m  \x1b[2m(PID {pid})\x1b[0m\n\
             \n\
             \x1b[2m  log\x1b[0m     {}\n\
             \x1b[2m  status\x1b[0m  noxa --watch-rag\n",
            dirs::home_dir().unwrap_or_default().join(DEFAULT_LOG).display(),
        );
        return;
    }

    let tei_url = std::env::var("TEI_URL").unwrap_or_else(|_| DEFAULT_TEI_URL.to_string());
    let qdrant_url = std::env::var("QDRANT_URL").unwrap_or_else(|_| DEFAULT_QDRANT_URL.to_string());

    match spawn_rag_daemon() {
        Ok(pid) => {
            let log_path = dirs::home_dir().unwrap_or_default().join(DEFAULT_LOG);
            let config = rag_config_path();
            eprintln!(
                "\n  \x1b[32m\x1b[1m✓ rag daemon started\x1b[0m\n\
                 \n\
                 \x1b[2m  config\x1b[0m  {}\n\
                 \x1b[2m  log\x1b[0m     {}\n\
                 \x1b[2m  pid\x1b[0m     \x1b[2m{pid}\x1b[0m\n",
                config.display(),
                log_path.display(),
            );

            if !probe_service(&format!("{tei_url}/health")) {
                eprintln!(
                    "  \x1b[33m⚠ TEI not reachable at {tei_url}\x1b[0m\n\
                     \x1b[2m    The daemon will retry, but embeddings won't work until TEI is up.\x1b[0m\n\
                     \x1b[2m    Start TEI: docker start axon-tei\x1b[0m\n"
                );
            }
            if !probe_service(&format!("{qdrant_url}/healthz")) {
                eprintln!(
                    "  \x1b[33m⚠ Qdrant not reachable at {qdrant_url}\x1b[0m\n\
                     \x1b[2m    The daemon will retry, but indexing won't work until Qdrant is up.\x1b[0m\n\
                     \x1b[2m    Start Qdrant: docker start axon-qdrant\x1b[0m\n"
                );
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

pub(crate) fn run_rag_stop() {
    match find_rag_daemon_pid() {
        None => eprintln!("  rag daemon is not running"),
        Some(pid) => {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
            #[cfg(not(unix))]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .status();
            }
            eprintln!("  rag daemon stopped (PID {pid})");
        }
    }
}
