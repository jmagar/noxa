use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::{Browser, OutputFormat, PdfModeArg};

/// Wrapper that owns the `[cli]` section of noxa.toml.
/// Unknown top-level sections (e.g. `[rag]`) are silently ignored.
#[derive(Debug, Default, Deserialize)]
struct TomlRoot {
    #[serde(default)]
    cli: NoxaConfig,
}

/// Non-secret, non-URL configuration defaults loaded from noxa.toml `[cli]`.
/// All fields optional — absent means "use the hard default".
/// Unknown fields are silently ignored so config files written for a newer
/// version of noxa work on older binaries.
///
/// DELIBERATELY EXCLUDED:
/// - on_change: passes content to sh -c; must remain CLI-only to prevent
///   shell injection via config file writes.
/// - Secrets/URLs (api_key, proxy, webhook, llm_base_url): stay in .env.
///
/// BOOL FLAG LIMITATION:
/// only_main_content, metadata, verbose, use_sitemap set to true here
/// cannot be overridden to false from the CLI for a single run (no --no-flag
/// variant in clap). Use NOXA_CONFIG=/dev/null (or an empty file) to bypass.
#[derive(Debug, Default, Deserialize)]
pub struct NoxaConfig {
    // Output
    pub format: Option<OutputFormat>,
    pub metadata: Option<bool>,
    pub verbose: Option<bool>,

    // Fetch
    pub browser: Option<Browser>,
    pub timeout: Option<u64>,
    pub pdf_mode: Option<PdfModeArg>,
    pub only_main_content: Option<bool>,

    // CSS selectors
    pub include_selectors: Option<Vec<String>>,
    pub exclude_selectors: Option<Vec<String>>,

    // Crawl
    pub depth: Option<usize>,
    pub max_pages: Option<usize>,
    pub concurrency: Option<usize>,
    pub delay: Option<u64>,
    pub path_prefix: Option<String>,
    pub include_paths: Option<Vec<String>>,
    pub exclude_paths: Option<Vec<String>>,
    pub use_sitemap: Option<bool>,

    // LLM (non-secret: provider name and model only; base URL stays in .env)
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub output_dir: Option<PathBuf>,
}

impl NoxaConfig {
    /// Load config from an explicit path, NOXA_CONFIG env var, ~/.noxa/noxa.toml,
    /// or a noxa.toml sibling to the running binary.
    /// Returns an empty (all-None) config if no file is found.
    /// Prints an error and exits if the file exists but is invalid TOML.
    pub fn load(explicit_path: Option<&str>) -> Self {
        let noxa_config_env = std::env::var("NOXA_CONFIG").ok();
        let was_explicit = explicit_path.is_some() || noxa_config_env.is_some();

        let path = if let Some(p) = explicit_path.map(PathBuf::from).or_else(|| noxa_config_env.map(PathBuf::from)) {
            p
        } else {
            match find_config_file() {
                Some(p) => p,
                None => return Self::default(),
            }
        };

        if !path.exists() {
            if was_explicit {
                let display_name = display_name(&path);
                eprintln!("error: config file not found: {display_name}");
                std::process::exit(1);
            }
            return Self::default();
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read config file {}: {e}", display_name(&path));
                std::process::exit(1);
            }
        };

        if path.to_str() == Some("/dev/null") || content.trim().is_empty() {
            return Self::default();
        }

        let name = display_name(&path);

        // Detect secret-looking keys in raw TOML before parsing
        let secret_keys = ["api_key", "proxy", "webhook", "llm_base_url", "password", "token", "secret"];
        let has_secrets = secret_keys.iter().any(|k| {
            // TOML syntax: `key = ` (with optional whitespace)
            content.contains(&format!("{k} =")) || content.contains(&format!("{k}="))
        });

        use crate::theme::*;
        if has_secrets {
            eprintln!(
                "{dim}config:{reset} {cyan}{bold}{name}{reset}  \
                 {yellow}⚠  secrets detected — api_key, proxy, webhook belong in .env{reset}"
            );
        } else {
            eprintln!("{dim}config:{reset} {cyan}{bold}{name}{reset}  {green}✓{reset}");
        }
        tracing::debug!("config path: {}", path.display());

        match toml::from_str::<TomlRoot>(&content) {
            Ok(root) => root.cli,
            Err(e) => {
                eprintln!("error: invalid TOML in config file {name}: {e}");
                std::process::exit(1);
            }
        }
    }
}

/// Search for noxa.toml in the standard locations (no explicit path given).
/// 1. ~/.noxa/noxa.toml
/// 2. Directory containing the running binary
/// 3. Current working directory (dev/in-repo convenience)
fn find_config_file() -> Option<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".noxa").join("noxa.toml");
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("noxa.toml");
            if p.exists() {
                return Some(p);
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join("noxa.toml");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_else(|| path.to_str().unwrap_or("config"))
        .to_string()
}

/// Fully resolved configuration after merging CLI flags > config file > hard defaults.
/// All fields are concrete — no Option<T>. This is what the rest of main.rs reads.
pub struct ResolvedConfig {
    // Output
    pub format: OutputFormat,
    pub metadata: bool,
    pub verbose: bool,

    // Fetch
    pub browser: Browser,
    pub timeout: u64,
    pub pdf_mode: PdfModeArg,
    pub only_main_content: bool,
    /// CLI-only output flag — not configurable via noxa.toml.
    pub raw_html: bool,

    // CSS selectors
    pub include_selectors: Vec<String>,
    pub exclude_selectors: Vec<String>,

    // Crawl
    pub depth: usize,
    pub max_pages: usize,
    pub concurrency: usize,
    pub delay: u64,
    pub path_prefix: Option<String>,
    pub include_paths: Vec<String>,
    pub exclude_paths: Vec<String>,
    pub use_sitemap: bool,

    // LLM
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub output_dir: Option<PathBuf>,
}

use clap::parser::ValueSource;

fn resolve_optional_setting(
    cli_explicit: bool,
    cli_value: Option<String>,
    cfg_value: Option<String>,
    env_value: Option<String>,
) -> Option<String> {
    if cli_explicit {
        cli_value
    } else {
        cfg_value.or_else(|| env_value.filter(|s| !s.is_empty()))
    }
}

/// Merge CLI flags (detected via ValueSource), config file, and hard defaults
/// into a single ResolvedConfig. CLI explicit values always win.
pub fn resolve(cli: &crate::Cli, matches: &clap::ArgMatches, cfg: &NoxaConfig) -> ResolvedConfig {
    let explicit = |name: &str| matches.value_source(name) == Some(ValueSource::CommandLine);

    ResolvedConfig {
        format: if explicit("format") {
            cli.format.clone()
        } else {
            cfg.format.clone().unwrap_or(crate::OutputFormat::Markdown)
        },
        browser: if explicit("browser") {
            cli.browser.clone()
        } else {
            cfg.browser.clone().unwrap_or(crate::Browser::Chrome)
        },
        pdf_mode: if explicit("pdf_mode") {
            cli.pdf_mode.clone()
        } else {
            cfg.pdf_mode.clone().unwrap_or(crate::PdfModeArg::Auto)
        },
        timeout: if explicit("timeout") {
            cli.timeout
        } else {
            cfg.timeout.unwrap_or(30)
        },
        depth: if explicit("depth") {
            cli.depth
        } else {
            cfg.depth.unwrap_or(1)
        },
        max_pages: if explicit("max_pages") {
            cli.max_pages
        } else {
            cfg.max_pages.unwrap_or(20)
        },
        concurrency: if explicit("concurrency") {
            cli.concurrency
        } else {
            cfg.concurrency.unwrap_or(5)
        },
        delay: if explicit("delay") {
            cli.delay
        } else {
            cfg.delay.unwrap_or(100)
        },
        path_prefix: if explicit("path_prefix") {
            cli.path_prefix.clone()
        } else {
            cfg.path_prefix.clone()
        },
        include_paths: if explicit("include_paths") {
            cli.include_paths
                .as_deref()
                .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
                .unwrap_or_default()
        } else {
            cfg.include_paths.clone().unwrap_or_default()
        },
        exclude_paths: if explicit("exclude_paths") {
            cli.exclude_paths
                .as_deref()
                .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
                .unwrap_or_default()
        } else {
            cfg.exclude_paths.clone().unwrap_or_default()
        },
        include_selectors: if explicit("include") {
            cli.include
                .as_deref()
                .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
                .unwrap_or_default()
        } else {
            cfg.include_selectors.clone().unwrap_or_default()
        },
        exclude_selectors: if explicit("exclude") {
            cli.exclude
                .as_deref()
                .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
                .unwrap_or_default()
        } else {
            cfg.exclude_selectors.clone().unwrap_or_default()
        },
        only_main_content: cli.only_main_content || cfg.only_main_content.unwrap_or(false),
        metadata: cli.metadata || cfg.metadata.unwrap_or(false),
        verbose: cli.verbose || cfg.verbose.unwrap_or(false),
        use_sitemap: cli.sitemap || cfg.use_sitemap.unwrap_or(false),
        raw_html: cli.raw_html,
        llm_provider: resolve_optional_setting(
            explicit("llm_provider"),
            cli.llm_provider.clone(),
            cfg.llm_provider.clone(),
            std::env::var("NOXA_LLM_PROVIDER").ok(),
        ),
        llm_model: resolve_optional_setting(
            explicit("llm_model"),
            cli.llm_model.clone(),
            cfg.llm_model.clone(),
            std::env::var("NOXA_LLM_MODEL").ok(),
        ),
        output_dir: if explicit("output_dir") {
            cli.output_dir.clone()
        } else {
            cfg.output_dir.clone()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    fn from_toml(s: &str) -> NoxaConfig {
        toml::from_str::<TomlRoot>(s).unwrap().cli
    }

    #[test]
    fn test_noxa_config_deserialize_full() {
        let cfg = from_toml(r#"
            [cli]
            format = "llm"
            depth = 3
            max_pages = 100
            concurrency = 10
            delay = 200
            browser = "firefox"
            timeout = 60
            only_main_content = true
            use_sitemap = true
            path_prefix = "/docs/"
            include_paths = ["/docs/*", "/api/*"]
            exclude_paths = ["/changelog/*", "/blog/*"]
            include_selectors = ["article", ".content"]
            exclude_selectors = ["nav", "footer"]
            llm_provider = "gemini"
            llm_model = "gemini-2.5-pro"
            pdf_mode = "fast"
            metadata = true
            verbose = false
        "#);
        assert!(matches!(cfg.format, Some(crate::OutputFormat::Llm)));
        assert_eq!(cfg.depth, Some(3));
        assert_eq!(
            cfg.exclude_paths,
            Some(vec!["/changelog/*".to_string(), "/blog/*".to_string()])
        );
        assert!(matches!(cfg.pdf_mode, Some(crate::PdfModeArg::Fast)));
    }

    #[test]
    fn test_noxa_config_empty() {
        let cfg = from_toml("");
        assert!(cfg.format.is_none());
        assert!(cfg.depth.is_none());
    }

    #[test]
    fn test_noxa_config_unknown_fields_ignored() {
        let cfg = from_toml(r#"
            [cli]
            depth = 2
            future_field = true
        "#);
        assert_eq!(cfg.depth, Some(2));
    }

    #[test]
    fn test_noxa_config_output_dir_deserialize() {
        let cfg = from_toml(r#"
            [cli]
            output_dir = "out"
        "#);
        assert_eq!(cfg.output_dir, Some(PathBuf::from("out")));
    }

    #[test]
    fn test_noxa_config_rag_section_ignored() {
        // [rag] section must not cause a parse error
        let cfg = from_toml(r#"
            [cli]
            depth = 5

            [rag]
            uuid_namespace = "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
        "#);
        assert_eq!(cfg.depth, Some(5));
    }

    #[test]
    fn test_resolve_uses_config_output_dir() {
        let cli = crate::Cli::parse_from(["noxa"]);
        let matches = crate::Cli::command().get_matches_from(["noxa"]);
        let cfg = from_toml(r#"[cli]
output_dir = "out""#);
        let resolved = resolve(&cli, &matches, &cfg);
        assert_eq!(resolved.output_dir, Some(PathBuf::from("out")));
    }

    #[test]
    fn test_resolve_keeps_cli_output_dir_raw_base_path() {
        let cli = crate::Cli::parse_from(["noxa", "--output-dir", "out"]);
        let matches = crate::Cli::command().get_matches_from(["noxa", "--output-dir", "out"]);
        let cfg = NoxaConfig::default();
        let resolved = resolve(&cli, &matches, &cfg);
        assert_eq!(resolved.output_dir, Some(PathBuf::from("out")));
    }

    #[test]
    fn test_resolve_prefers_config_llm_provider_over_env_default() {
        let resolved =
            resolve_optional_setting(false, None, Some("gemini".into()), Some("ollama".into()));
        assert_eq!(resolved, Some("gemini".into()));
    }

    #[test]
    fn test_resolve_uses_env_llm_provider_when_config_missing() {
        let resolved = resolve_optional_setting(false, None, None, Some("ollama".into()));
        assert_eq!(resolved, Some("ollama".into()));
    }

    #[test]
    fn test_load_dev_null_returns_default() {
        let cfg = NoxaConfig::load(Some("/dev/null"));
        assert!(cfg.format.is_none());
        assert!(cfg.llm_provider.is_none());
    }

    #[test]
    fn test_load_whitespace_file_returns_default() {
        let mut path = std::env::temp_dir();
        let suffix = format!(
            "noxa-config-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(suffix);
        std::fs::write(&path, "   \n\t  ").unwrap();

        let cfg = NoxaConfig::load(Some(path.to_str().unwrap()));
        assert!(cfg.format.is_none());
        assert!(cfg.llm_model.is_none());

        let _ = std::fs::remove_file(&path);
    }
}
