use super::*;

pub(crate) fn init_logging(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("noxa=debug")
    } else {
        EnvFilter::try_from_env("NOXA_LOG").unwrap_or_else(|_| EnvFilter::new("warn"))
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

pub(crate) fn init_mcp_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init()
        .ok();
}

/// Build an operations log from CLI flags.
///
/// Returns `None` when `--no-store` or `NOXA_NO_OPERATIONS_LOG` / `NOXA_NO_STORE` is set.
pub(crate) fn build_ops_log(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
) -> Option<Arc<FilesystemOperationsLog>> {
    if cli.no_store
        || env_flag_enabled("NOXA_NO_OPERATIONS_LOG")
        || env_flag_enabled("NOXA_NO_STORE")
    {
        return None;
    }
    let root = content_store_root(resolved.output_dir.as_deref());
    Some(Arc::new(FilesystemOperationsLog::new(root)))
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            let trimmed = value.trim();
            // Empty / whitespace-only values are treated as unset (not enabled).
            // Only explicit truthy tokens enable the flag so that patterns like
            // `export NOXA_NO_STORE=` (common "unset-like" shell idioms) do not
            // accidentally disable the store.
            matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Append an entry to the operations log if one is configured.
///
/// Centralises the repeated `if let Some(ref log) … append … warn` pattern
/// that appears in every command handler.
pub(crate) async fn log_operation(
    ops_log: &Option<Arc<FilesystemOperationsLog>>,
    url: &str,
    op: Op,
    input: impl FnOnce() -> serde_json::Value,
    output: impl FnOnce() -> serde_json::Value,
) {
    if let Some(log) = ops_log {
        let domain = domain_from_url(url);
        let op_dbg = format!("{op:?}");
        let entry = OperationEntry {
            op,
            at: chrono::Utc::now(),
            url: url.to_string(),
            input: input(),
            output: output(),
        };
        if let Err(e) = log.append(&domain, &entry).await {
            // Log domain rather than the full URL to avoid leaking sensitive query parameters
            // (API keys, auth tokens) that may appear in operator-provided URLs.
            tracing::warn!(op = %op_dbg, %domain, error = %e, "ops log append failed");
        }
    }
}
