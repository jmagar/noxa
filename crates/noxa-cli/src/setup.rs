/// `noxa setup` — interactive first-run configuration.
///
/// Ported from setup.sh so the same logic is available everywhere the binary
/// is (including after a `cargo install` one-liner with no repo clone).
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dialoguer::{Confirm, Input, Password, theme::ColorfulTheme};

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() {
    println!();
    println!("\x1b[1;32m  noxa — Setup\x1b[0m");
    println!("\x1b[2m  Web extraction toolkit for AI agents\x1b[0m");
    println!();

    let script_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));

    let theme = ColorfulTheme::default();

    check_prerequisites();
    configure_env(&theme, &script_dir);
    setup_ollama(&theme);
    setup_mcp(&theme, &script_dir);
    print_summary(&script_dir);
}

// ---------------------------------------------------------------------------
// Step 1: Prerequisites
// ---------------------------------------------------------------------------

fn check_prerequisites() {
    println!("\x1b[1;32m  Step 1: Prerequisites\x1b[0m");
    println!();

    let mut all_good = true;

    // Rust
    match Command::new("rustc").arg("--version").output() {
        Ok(out) => {
            let ver = String::from_utf8_lossy(&out.stdout);
            let ver = ver.trim();
            println!("\x1b[32m[+]\x1b[0m {ver}");

            // Check >= 1.85 (edition 2024)
            let too_old = ver
                .split_whitespace()
                .nth(1)
                .and_then(|v| {
                    let mut parts = v.split('.');
                    let major: u32 = parts.next()?.parse().ok()?;
                    let minor: u32 = parts.next()?.parse().ok()?;
                    Some(major < 1 || (major == 1 && minor < 85))
                })
                .unwrap_or(false);

            if too_old {
                println!("\x1b[33m[!]\x1b[0m Rust 1.85+ required (edition 2024). Run: rustup update");
                all_good = false;
            }
        }
        Err(_) => {
            println!("\x1b[31m[x]\x1b[0m Rust not found. Install: https://rustup.rs");
            all_good = false;
        }
    }

    // Cargo
    match Command::new("cargo").arg("--version").output() {
        Ok(out) => {
            let ver = String::from_utf8_lossy(&out.stdout);
            println!("\x1b[32m[+]\x1b[0m {}", ver.trim());
        }
        Err(_) => {
            println!("\x1b[31m[x]\x1b[0m Cargo not found (should come with Rust)");
            all_good = false;
        }
    }

    // Ollama (optional)
    if Command::new("ollama").arg("--version").output().is_ok() {
        println!("\x1b[32m[+]\x1b[0m Ollama installed");
        if ollama_running() {
            println!("\x1b[32m[+]\x1b[0m Ollama is running");
        } else {
            println!("\x1b[33m[!]\x1b[0m Ollama installed but not running (start with: ollama serve)");
        }
    } else {
        println!("\x1b[33m[!]\x1b[0m Ollama not found (optional — needed for local LLM features)");
    }

    // Git
    match Command::new("git").arg("--version").output() {
        Ok(out) => {
            let ver = String::from_utf8_lossy(&out.stdout);
            println!("\x1b[32m[+]\x1b[0m {}", ver.trim());
        }
        Err(_) => {
            println!("\x1b[31m[x]\x1b[0m Git not found");
            all_good = false;
        }
    }

    println!();
    if all_good {
        println!("\x1b[32m[+]\x1b[0m All prerequisites met.");
    } else {
        println!("\x1b[31m[x]\x1b[0m Some prerequisites are missing. Fix them before continuing.");
        std::process::exit(1);
    }
    println!();
}

// ---------------------------------------------------------------------------
// Step 2: Configure .env
// ---------------------------------------------------------------------------

fn configure_env(theme: &ColorfulTheme, dir: &Path) {
    println!("\x1b[1;32m  Step 2: Configuration\x1b[0m");
    println!();

    let env_path = dir.join(".env");

    if env_path.exists() {
        println!("\x1b[33m[!]\x1b[0m .env already exists.");
        let overwrite = Confirm::with_theme(theme)
            .with_prompt("Overwrite?")
            .default(false)
            .interact()
            .unwrap_or(false);

        if !overwrite {
            println!("\x1b[34m[*]\x1b[0m Keeping existing .env");
            println!();
            return;
        }
    }

    println!("\x1b[34m[*]\x1b[0m LLM configuration");

    let ollama_model: String = Input::with_theme(theme)
        .with_prompt("Ollama model (local)")
        .default("qwen3:8b".into())
        .interact_text()
        .unwrap_or_else(|_| "qwen3:8b".into());

    let openai_key: String = Password::with_theme(theme)
        .with_prompt("OpenAI API key (optional, press enter to skip)")
        .allow_empty_password(true)
        .interact()
        .unwrap_or_default();

    let anthropic_key: String = Password::with_theme(theme)
        .with_prompt("Anthropic API key (optional, press enter to skip)")
        .allow_empty_password(true)
        .interact()
        .unwrap_or_default();

    println!();
    println!("\x1b[34m[*]\x1b[0m Proxy configuration");
    let proxies_path = dir.join("proxies.txt");
    if proxies_path.exists() {
        let count = fs::read_to_string(&proxies_path)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .count();
        println!("\x1b[32m[+]\x1b[0m proxies.txt found with {count} proxies (auto-loaded)");
    } else {
        println!("\x1b[34m[*]\x1b[0m To use proxies, create proxies.txt (format: host:port:user:pass)");
    }

    println!();
    println!("\x1b[34m[*]\x1b[0m Server configuration");

    let server_port: String = Input::with_theme(theme)
        .with_prompt("REST API port")
        .default("3000".into())
        .interact_text()
        .unwrap_or_else(|_| "3000".into());

    let auth_key: String = Input::with_theme(theme)
        .with_prompt("API auth key (press enter to auto-generate)")
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    let auth_key = if auth_key.is_empty() {
        let key = generate_hex_key(16);
        println!("\x1b[34m[*]\x1b[0m Generated auth key: {key}");
        key
    } else {
        auth_key
    };

    // Write .env
    let mut content = format!(
        "# noxa configuration — generated by noxa setup\n\
         \n\
         # --- LLM Providers ---\n\
         OLLAMA_HOST=http://localhost:11434\n\
         OLLAMA_MODEL={ollama_model}\n"
    );

    if !openai_key.is_empty() {
        content.push_str(&format!("OPENAI_API_KEY={openai_key}\n"));
    }
    if !anthropic_key.is_empty() {
        content.push_str(&format!("ANTHROPIC_API_KEY={anthropic_key}\n"));
    }

    content.push_str("\n# --- Proxy ---\n");
    content.push_str("# NOXA_PROXY_FILE=/path/to/proxies.txt\n");

    content.push_str(&format!(
        "\n# --- Server ---\n\
         NOXA_PORT={server_port}\n\
         NOXA_HOST=0.0.0.0\n\
         NOXA_AUTH_KEY={auth_key}\n\
         \n\
         # --- Logging ---\n\
         NOXA_LOG=info\n"
    ));

    if let Err(e) = fs::write(&env_path, &content) {
        println!("\x1b[31m[x]\x1b[0m Failed to write .env: {e}");
        std::process::exit(1);
    }

    println!();
    println!("\x1b[32m[+]\x1b[0m .env created.");
    println!();
}

// ---------------------------------------------------------------------------
// Step 3: Ollama
// ---------------------------------------------------------------------------

fn setup_ollama(theme: &ColorfulTheme) {
    println!("\x1b[1;32m  Step 3: Ollama (Local LLM)\x1b[0m");
    println!();

    if Command::new("ollama").arg("--version").output().is_err() {
        println!("\x1b[34m[*]\x1b[0m Ollama not installed (optional — needed for local LLM features).");

        let install = Confirm::with_theme(theme)
            .with_prompt("Install Ollama?")
            .default(true)
            .interact()
            .unwrap_or(false);

        if !install {
            println!("\x1b[34m[*]\x1b[0m Skipping Ollama. Install later: https://ollama.ai");
            println!();
            return;
        }

        #[cfg(target_os = "macos")]
        {
            println!("\x1b[34m[*]\x1b[0m Install Ollama manually: https://ollama.ai/download");
            println!();
            return;
        }

        #[cfg(not(target_os = "macos"))]
        {
            println!("\x1b[34m[*]\x1b[0m Installing Ollama...");
            let status = Command::new("sh")
                .args(["-c", "curl -fsSL https://ollama.ai/install.sh | sh"])
                .status();
            match status {
                Ok(s) if s.success() => println!("\x1b[32m[+]\x1b[0m Ollama installed."),
                _ => {
                    println!("\x1b[31m[x]\x1b[0m Ollama install failed. Try manually: https://ollama.ai");
                    println!();
                    return;
                }
            }
        }
    }

    if !ollama_running() {
        println!("\x1b[33m[!]\x1b[0m Ollama is not running.");
        let start = Confirm::with_theme(theme)
            .with_prompt("Start Ollama now?")
            .default(true)
            .interact()
            .unwrap_or(false);

        if start {
            let _ = Command::new("ollama")
                .arg("serve")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();

            std::thread::sleep(std::time::Duration::from_secs(2));

            if ollama_running() {
                println!("\x1b[32m[+]\x1b[0m Ollama is running.");
            } else {
                println!("\x1b[33m[!]\x1b[0m Ollama didn't start. Start it manually and re-run setup.");
                println!();
                return;
            }
        } else {
            println!();
            return;
        }
    }

    // Pull model
    let model = read_env_var("OLLAMA_MODEL").unwrap_or_else(|| "qwen3:8b".into());
    let has_model = check_ollama_model(&model);

    if has_model {
        println!("\x1b[32m[+]\x1b[0m Model {model} already available.");
    } else {
        println!("\x1b[34m[*]\x1b[0m Model {model} not found locally.");
        let pull = Confirm::with_theme(theme)
            .with_prompt(format!("Pull {model} now? (downloads ~5 GB)"))
            .default(true)
            .interact()
            .unwrap_or(false);

        if pull {
            let status = Command::new("ollama").args(["pull", &model]).status();
            match status {
                Ok(s) if s.success() => println!("\x1b[32m[+]\x1b[0m Model {model} ready."),
                _ => println!("\x1b[33m[!]\x1b[0m Pull failed. Run: ollama pull {model}"),
            }
        }
    }

    println!();
}

// ---------------------------------------------------------------------------
// Step 4: MCP server → Claude Desktop
// ---------------------------------------------------------------------------

fn setup_mcp(theme: &ColorfulTheme, dir: &Path) {
    println!("\x1b[1;32m  Step 4: MCP Server (Claude Desktop integration)\x1b[0m");
    println!();

    let mcp_binary = dir.join("noxa-mcp");
    let mcp_binary = if mcp_binary.exists() {
        mcp_binary
    } else {
        // Installed via cargo install: binary is next to noxa
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("noxa"));
        exe.parent()
            .map(|p| p.join("noxa-mcp"))
            .unwrap_or_else(|| PathBuf::from("noxa-mcp"))
    };

    if !mcp_binary.exists() {
        println!("\x1b[33m[!]\x1b[0m noxa-mcp binary not found. Build or install it first.");
        println!();
        return;
    }

    println!("\x1b[34m[*]\x1b[0m Tools available via MCP: scrape, crawl, map, batch, extract, summarize, diff, brand");
    println!();

    let configure = Confirm::with_theme(theme)
        .with_prompt("Configure MCP server for Claude Desktop?")
        .default(true)
        .interact()
        .unwrap_or(false);

    if !configure {
        println!(
            "\x1b[34m[*]\x1b[0m Skipping MCP setup. To configure later, add to your Claude Desktop config:"
        );
        println!(
            r#"    {{"mcpServers": {{"noxa": {{"command": "{}"}}}}}}"#,
            mcp_binary.display()
        );
        println!();
        return;
    }

    let config_path = claude_desktop_config_path();

    // Ensure parent dir exists
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Read existing config or start with empty object
    let existing = fs::read_to_string(&config_path).unwrap_or_else(|_| "{}".into());
    let mut config: serde_json::Value =
        serde_json::from_str(&existing).unwrap_or(serde_json::json!({}));

    // Check if already configured
    let already = config
        .get("mcpServers")
        .and_then(|s| s.get("noxa"))
        .is_some();

    if already {
        println!("\x1b[33m[!]\x1b[0m noxa MCP server already configured in Claude Desktop.");
        let update = Confirm::with_theme(theme)
            .with_prompt("Update the path?")
            .default(true)
            .interact()
            .unwrap_or(false);

        if !update {
            println!();
            return;
        }
    }

    // Merge
    config
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .unwrap()
        .insert(
            "noxa".into(),
            serde_json::json!({"command": mcp_binary.to_string_lossy()}),
        );

    match serde_json::to_string_pretty(&config) {
        Ok(json) => match fs::write(&config_path, json) {
            Ok(_) => {
                println!("\x1b[32m[+]\x1b[0m MCP server configured in Claude Desktop.");
                println!("\x1b[34m[*]\x1b[0m Restart Claude Desktop to activate.");
            }
            Err(e) => println!("\x1b[31m[x]\x1b[0m Failed to write config: {e}"),
        },
        Err(e) => println!("\x1b[31m[x]\x1b[0m JSON serialisation error: {e}"),
    }

    println!();
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

fn print_summary(dir: &Path) {
    let noxa = dir.join("noxa");
    let server = dir.join("noxa-server");

    // Read NOXA_PORT from .env if present
    let port = read_env_var("NOXA_PORT").unwrap_or_else(|| "3000".into());

    println!("\x1b[1;32m  Setup Complete\x1b[0m");
    println!();
    println!("  \x1b[1mCLI:\x1b[0m");
    println!("    {} https://example.com --format llm", noxa.display());
    println!();
    println!("  \x1b[1mREST API:\x1b[0m");
    println!("    {}", server.display());
    println!("    curl http://localhost:{port}/health");
    println!();
    println!("  \x1b[1mMCP Server:\x1b[0m");
    println!("    Configured in Claude Desktop (restart to activate)");
    println!();
    println!("  \x1b[1mConfig:\x1b[0m  {}/.env", dir.display());
    println!();
    println!("\x1b[2m  Tip: Add to PATH for convenience:\x1b[0m");
    println!("    export PATH=\"{}:$PATH\"", dir.display());
    println!();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ollama_running() -> bool {
    std::net::TcpStream::connect("127.0.0.1:11434").is_ok()
}

fn check_ollama_model(model: &str) -> bool {
    // Hit /api/tags and look for the model name
    let Ok(mut stream) = std::net::TcpStream::connect("127.0.0.1:11434") else {
        return false;
    };
    use std::io::{Write as _, Read as _};
    let req = "GET /api/tags HTTP/1.0\r\nHost: localhost\r\n\r\n";
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = String::new();
    let _ = stream.read_to_string(&mut buf);
    buf.contains(model)
}

fn generate_hex_key(bytes: usize) -> String {
    (0..bytes)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect()
}

fn read_env_var(key: &str) -> Option<String> {
    // Try process env first (already loaded by dotenvy), then parse .env directly
    if let Ok(val) = std::env::var(key) {
        return Some(val);
    }
    let content = fs::read_to_string(".env").ok()?;
    content
        .lines()
        .find(|l| l.starts_with(&format!("{key}=")))
        .map(|l| l[key.len() + 1..].trim().to_string())
}

fn claude_desktop_config_path() -> PathBuf {
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
