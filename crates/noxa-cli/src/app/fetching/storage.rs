use crate::app::*;

pub(crate) fn write_to_file(dir: &Path, filename: &str, content: &str) -> Result<(), String> {
    // Reject path traversal and absolute paths before joining.
    if filename.split(['/', '\\']).any(|p| p == ".." || p == ".")
        || filename.starts_with('/')
        || filename.starts_with('\\')
        || filename.contains('\0')
    {
        return Err(format!("unsafe filename rejected: {filename}"));
    }
    let dest = dir.join(filename);
    // Lexical containment check (fast pre-filter).
    if !dest.starts_with(dir) {
        return Err(format!("filename escapes output directory: {filename}"));
    }

    // Ensure the output directory exists, then canonicalize it before any other I/O.
    std::fs::create_dir_all(dir).map_err(|e| format!("failed to create output directory: {e}"))?;
    let canonical_dir = std::fs::canonicalize(dir)
        .map_err(|e| format!("failed to resolve output directory: {e}"))?;

    // If `dest` already exists, check for symlinks before any further side-effects.
    // Use symlink_metadata (not exists/metadata) so dangling symlinks are also detected.
    if let Ok(meta) = dest.symlink_metadata() {
        if meta.file_type().is_symlink() {
            // Any symlink at the destination — dangling or not — is rejected.
            return Err(format!(
                "filename escapes output directory via symlink: {filename}"
            ));
        }
        // Regular file or directory: verify it's inside the canonical output dir.
        let canonical_dest = std::fs::canonicalize(&dest)
            .map_err(|e| format!("failed to resolve destination path: {e}"))?;
        if !canonical_dest.starts_with(&canonical_dir) {
            return Err(format!("filename escapes output directory: {filename}"));
        }
    }

    // Create parent directories only after symlink checks pass.
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create directory {}: {e}", parent.display()))?;
        // Re-verify after dir creation: intermediate symlinks in the path may resolve
        // to a location outside `dir`.
        let canonical_parent = std::fs::canonicalize(parent)
            .map_err(|e| format!("failed to resolve destination parent: {e}"))?;
        if !canonical_parent.starts_with(&canonical_dir) {
            return Err(format!(
                "filename escapes output directory via symlink: {filename}"
            ));
        }

        let final_name = dest
            .file_name()
            .ok_or_else(|| format!("invalid filename: {filename}"))?;
        let canonical_dest = canonical_parent.join(final_name);
        if let Ok(meta) = canonical_dest.symlink_metadata()
            && meta.file_type().is_symlink()
        {
            return Err(format!(
                "filename escapes output directory via symlink: {filename}"
            ));
        }

        std::fs::write(&canonical_dest, content)
            .map_err(|e| format!("failed to write {}: {e}", canonical_dest.display()))?;
        return Ok(());
    }

    std::fs::write(&dest, content)
        .map_err(|e| format!("failed to write {}: {e}", dest.display()))?;
    Ok(())
}

/// Print the save hint block — only called for single-URL saves, not batch/crawl.
pub(crate) fn print_save_hint(dest: &std::path::Path, content: &str) {
    let word_count = content.split_whitespace().count();
    let word_display = if word_count >= 1000 {
        format!("{:.1}k", word_count as f64 / 1000.0)
    } else {
        word_count.to_string()
    };
    let dir_part = dest
        .parent()
        .map(|p| format!("{}/", p.display()))
        .unwrap_or_default();
    let file_part = dest.file_name().and_then(|n| n.to_str()).unwrap_or("");
    eprintln!(
        "\n  {green}{bold}✓ saved{reset}  {dim}{dir_part}{reset}{bold}{cyan}{file_part}{reset}  {yellow}{bold}{word_display} words{reset}\n\
         \n\
         {dim}  grep{reset}     {cyan}noxa --grep {green}\"TERM\"{reset}\n\
         {dim}  context{reset}  {pink}noxa <url>{reset} {dim}(omit --output-dir to pipe straight to LLM){reset}\n",
    );
}

/// Validate a URL provided by the operator (e.g. SEARXNG_URL). Only checks scheme and
/// host presence; does NOT reject private/loopback addresses (operator-trusted config).
pub(crate) fn validate_operator_url(url: &str) -> Result<(), String> {
    parse_http_url(url).map(|_| ())
}

/// Synchronous URL safety check (no DNS resolution) used for filtering search results.
/// Rejects private/loopback IP addresses but does not resolve hostnames.
/// NOTE: For stronger SSRF protection with DNS resolution, use the async `validate_url`.
pub(crate) fn validate_url_sync(url: &str) -> Result<(), String> {
    let parsed = parse_http_url(url)?;
    let host = parsed.host_str().ok_or("Invalid URL: no host")?;
    if matches!(host, "localhost" | "ip6-localhost" | "ip6-loopback")
        || host.ends_with(".localhost")
    {
        return Err(format!(
            "Invalid URL: host '{host}' is not a routable public address"
        ));
    }

    let private = match parsed.host() {
        Some(url::Host::Ipv4(addr)) => {
            let ip = std::net::IpAddr::V4(addr);
            ip.is_loopback() || ip.is_unspecified() || is_private_or_reserved_ip(ip)
        }
        Some(url::Host::Ipv6(addr)) => {
            let ip = std::net::IpAddr::V6(addr);
            ip.is_loopback() || ip.is_unspecified() || is_private_or_reserved_ip(ip)
        }
        Some(url::Host::Domain(_)) => false, // DNS not checked in sync path; use async validate_url for full protection
        None => true,
    };
    if private {
        return Err(format!(
            "Invalid URL: host '{host}' is not a routable public address"
        ));
    }

    Ok(())
}
