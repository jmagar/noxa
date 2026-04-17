use crate::app::*;

pub(crate) fn build_fetch_config(cli: &Cli, resolved: &config::ResolvedConfig) -> FetchConfig {
    let (proxy, proxy_pool) = if cli.proxy.is_some() {
        (cli.proxy.clone(), Vec::new())
    } else if let Some(ref path) = cli.proxy_file {
        match noxa_fetch::parse_proxy_file(path) {
            Ok(pool) => (None, pool),
            Err(e) => {
                eprintln!("warning: {e}");
                (None, Vec::new())
            }
        }
    } else if std::path::Path::new("proxies.txt").exists() {
        // Auto-load proxies.txt from working directory if present
        match noxa_fetch::parse_proxy_file("proxies.txt") {
            Ok(pool) if !pool.is_empty() => {
                eprintln!("loaded {} proxies from proxies.txt", pool.len());
                (None, pool)
            }
            _ => (None, Vec::new()),
        }
    } else {
        (None, Vec::new())
    };

    // Use lowercase keys so user -H flags (any casing) override defaults without duplication.
    // HTTP header names are case-insensitive; we normalise to ASCII-lowercase throughout.
    let mut headers = std::collections::HashMap::from([(
        "accept-language".to_string(),
        "en-US,en;q=0.9".to_string(),
    )]);

    // Parse -H "Key: Value" flags; normalise key to lowercase for dedup.
    for h in &cli.headers {
        if let Some((key, val)) = h.split_once(':') {
            headers.insert(
                key.trim().to_ascii_lowercase(),
                val.trim().to_string(),
            );
        }
    }

    // --cookie shorthand (lowercase key for consistency)
    if let Some(ref cookie) = cli.cookie {
        headers.insert("cookie".to_string(), cookie.clone());
    }

    // --cookie-file: parse JSON array of {name, value, domain, ...}
    if let Some(ref path) = cli.cookie_file {
        match parse_cookie_file(path) {
            Ok(cookie_str) => {
                // Merge with existing cookies if --cookie was also provided
                if let Some(existing) = headers.get("cookie") {
                    headers.insert("cookie".to_string(), format!("{existing}; {cookie_str}"));
                } else {
                    headers.insert("cookie".to_string(), cookie_str);
                }
            }
            Err(e) => {
                eprintln!("error: failed to parse cookie file: {e}");
                process::exit(1);
            }
        }
    }

    let store = if cli.no_store {
        None
    } else {
        let root = content_store_root(resolved.output_dir.as_deref());
        // Ensure the root directory exists so that ContentStore::resolve_path can
        // canonicalize it.  A fresh output_dir would otherwise cause all writes to
        // fail silently because std::fs::canonicalize returns an error for a
        // non-existent path.
        std::fs::create_dir_all(&root).unwrap_or_else(|e| {
            eprintln!(
                "error: failed to initialize content store at {}: {e}",
                root.display()
            );
            process::exit(1);
        });
        Some(FilesystemContentStore::new(root))
    };

    let ops_log = build_ops_log(cli, resolved);

    FetchConfig {
        browser: resolved.browser.clone().into(),
        proxy,
        proxy_pool,
        timeout: std::time::Duration::from_secs(resolved.timeout),
        pdf_mode: resolved.pdf_mode.clone().into(),
        headers,
        store,
        ops_log,
        ..Default::default()
    }
}

/// Parse a JSON cookie file (Chrome extension format) into a Cookie header string.
/// Supports: [{name, value, domain, path, secure, httpOnly, expirationDate, ...}]
pub(crate) fn parse_cookie_file(path: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let cookies: Vec<serde_json::Value> =
        serde_json::from_str(&content).map_err(|e| format!("invalid JSON: {e}"))?;

    let pairs: Vec<String> = cookies
        .iter()
        .filter_map(|c| {
            // Accept all cookies with a name and value; domain/secure/path are metadata
            // that don't affect whether the cookie is usable for HTTP requests.
            let name = c.get("name")?.as_str()?;
            let value = c.get("value")?.as_str()?;
            if name.is_empty() {
                return None;
            }
            Some(format!("{name}={value}"))
        })
        .collect();

    if pairs.is_empty() {
        return Err("no cookies found in file".to_string());
    }

    Ok(pairs.join("; "))
}

pub(crate) fn build_extraction_options(resolved: &config::ResolvedConfig) -> ExtractionOptions {
    ExtractionOptions {
        include_selectors: resolved.include_selectors.clone(),
        exclude_selectors: resolved.exclude_selectors.clone(),
        only_main_content: resolved.only_main_content,
        include_raw_html: resolved.raw_html || matches!(resolved.format, OutputFormat::Html),
    }
}

/// Normalize a URL: prepend `https://` if no scheme is present.
pub(crate) fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// Derive a filename from a URL for `--output-dir`.
///
/// Strips the scheme/host, maps the path to a filesystem path, and appends
/// an extension matching the output format.
#[cfg(test)]
pub(crate) fn url_to_filename(raw_url: &str, format: &OutputFormat) -> String {
    let ext = match format {
        OutputFormat::Markdown | OutputFormat::Llm => "md",
        OutputFormat::Json => "json",
        OutputFormat::Text => "txt",
        OutputFormat::Html => "html",
    };

    let parsed = url::Url::parse(raw_url);
    let (host, path, query) = match &parsed {
        Ok(u) => (
            u.host_str().unwrap_or("unknown").to_string(),
            u.path().to_string(),
            u.query().map(String::from),
        ),
        Err(_) => (String::new(), String::new(), None),
    };

    let clean_host = host.strip_prefix("www.").unwrap_or(&host);
    let host_dir = clean_host.replace('.', "_");

    let mut stem = path.trim_matches('/').to_string();
    if stem.is_empty() {
        stem = format!("{host_dir}/index");
    } else {
        stem = format!("{host_dir}/{stem}");
    }

    // Append query params so /p?id=123 doesn't collide with /p?id=456
    if let Some(q) = query {
        stem = format!("{stem}_{q}");
    }

    // Sanitize: keep alphanumeric, dash, underscore, dot, slash
    let sanitized: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/') {
                c
            } else {
                '_'
            }
        })
        .collect();

    format!("{sanitized}.{ext}")
}

pub(crate) async fn validate_url(url: &str) -> Result<(), String> {
    validate_public_http_url(url).await
}

/// Canonical content store root: `output_dir/.noxa/content` when configured,
/// otherwise `~/.noxa/content`. Every fetch path writes here so the same URL
/// always maps to the same file regardless of how it was fetched.
///
/// Exits the process with an error message if the home directory cannot be
/// determined. Using `"."` as a fallback scatters data unpredictably; hard
/// error is intentional.
pub(crate) fn content_store_root(output_dir: Option<&Path>) -> PathBuf {
    noxa_store::content_store_root(output_dir).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    })
}
