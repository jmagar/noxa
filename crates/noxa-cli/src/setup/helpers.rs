use std::fs;
use std::path::PathBuf;

pub(super) fn ollama_running() -> bool {
    std::net::TcpStream::connect("127.0.0.1:11434").is_ok()
}

pub(super) fn check_ollama_model(model: &str) -> bool {
    // Hit /api/tags and compare exact model names from the response JSON.
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:11434".parse().unwrap(),
        std::time::Duration::from_secs(5),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(5)));
    use std::io::{Read as _, Write as _};
    let req = "GET /api/tags HTTP/1.0\r\nHost: localhost\r\n\r\n";
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let buf = String::from_utf8_lossy(&raw);
    let body = buf.split("\r\n\r\n").nth(1).unwrap_or("");
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    value["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["name"].as_str())
        .any(|name| name == model)
}

pub(super) fn generate_hex_key(bytes: usize) -> String {
    (0..bytes)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect()
}

pub(super) fn read_env_var(key: &str) -> Option<String> {
    // Try process env first (already loaded by dotenvy), then parse .env directly.
    if let Ok(val) = std::env::var(key) {
        return Some(val);
    }

    // Locate .env: prefer the directory next to the executable so the lookup
    // succeeds regardless of the process CWD.
    let env_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(".env")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from(".env"));

    let content = fs::read_to_string(env_path).ok()?;
    content.lines().find_map(|raw_line| {
        // Trim leading/trailing whitespace and strip optional `export ` prefix.
        let line = raw_line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        // Split on the first `=` only.
        let (k, v) = line.split_once('=')?;
        if k.trim() != key {
            return None;
        }
        // Strip matching surrounding quotes from the value.
        let v = v.trim();
        let v = if (v.starts_with('"') && v.ends_with('"'))
            || (v.starts_with('\'') && v.ends_with('\''))
        {
            &v[1..v.len() - 1]
        } else {
            v
        };
        Some(v.to_string())
    })
}

pub(super) fn claude_desktop_config_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_default()
            .join("Library/Application Support/Claude/claude_desktop_config.json")
    }
    #[cfg(target_os = "windows")]
    {
        // Windows: %APPDATA%\Claude\claude_desktop_config.json
        dirs::config_dir()
            .unwrap_or_default()
            .join("Claude/claude_desktop_config.json")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Linux / other Unix: ~/.config/Claude/claude_desktop_config.json (XDG)
        dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
            .join("Claude/claude_desktop_config.json")
    }
}
