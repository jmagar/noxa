use std::fs;
use std::path::PathBuf;

pub(super) fn ollama_running() -> bool {
    std::net::TcpStream::connect("127.0.0.1:11434").is_ok()
}

pub(super) fn check_ollama_model(model: &str) -> bool {
    // Hit /api/tags and compare exact model names from the response JSON.
    let Ok(mut stream) = std::net::TcpStream::connect("127.0.0.1:11434") else {
        return false;
    };
    use std::io::{Read as _, Write as _};
    let req = "GET /api/tags HTTP/1.0\r\nHost: localhost\r\n\r\n";
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = String::new();
    let _ = stream.read_to_string(&mut buf);
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
    let content = fs::read_to_string(".env").ok()?;
    content
        .lines()
        .find(|l| l.starts_with(&format!("{key}=")))
        .map(|l| l[key.len() + 1..].trim().to_string())
}

pub(super) fn claude_desktop_config_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_default()
            .join("Library/Application Support/Claude/claude_desktop_config.json")
    }
    #[cfg(not(target_os = "macos"))]
    {
        dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
            .join("claude/claude_desktop_config.json")
    }
}
