use std::path::PathBuf;

const MAX_REL_PATH_LEN: usize = 240;
const URL_HASH_LEN: usize = 16;

/// Map a validated URL to a relative store path without extension.
pub fn try_url_to_store_path(url: &str) -> Result<PathBuf, crate::types::StoreError> {
    let parsed = url::Url::parse(url)
        .map_err(|e| crate::types::StoreError::InvalidUrl(format!("{url}: {e}")))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| crate::types::StoreError::InvalidUrl(format!("{url}: missing host")))?;
    let clean_host = sanitize_component(host.strip_prefix("www.").unwrap_or(host));

    let segments: Vec<String> = parsed
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        .map(sanitize_component)
        .collect();

    let path_part = if segments.is_empty() {
        "index".to_string()
    } else {
        segments.join("/")
    };

    let mut rel = format!("{clean_host}/{path_part}");
    let needs_hash_suffix = parsed.query().is_some() || rel.len() > MAX_REL_PATH_LEN;
    if needs_hash_suffix {
        let hash = format!("{:016x}", url_hash(url));
        let max_prefix_len = MAX_REL_PATH_LEN.saturating_sub(URL_HASH_LEN + 1);
        if rel.len() > max_prefix_len {
            rel.truncate(max_prefix_len);
        }
        rel.push('_');
        rel.push_str(&hash);
    }

    Ok(PathBuf::from(rel))
}

/// Lossy compatibility wrapper for already-validated URLs.
pub fn url_to_store_path(url: &str) -> PathBuf {
    try_url_to_store_path(url).unwrap_or_else(|_| PathBuf::from("unknown"))
}

fn url_hash(url: &str) -> u64 {
    url.bytes().fold(14695981039346656037_u64, |acc, b| {
        (acc ^ (b as u64)).wrapping_mul(1099511628211)
    })
}

pub(crate) fn sanitize_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "index".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Return the canonical content store root.
///
/// - If `output_dir` is given, the root is `output_dir/.noxa/content`.
/// - Otherwise, uses the user's home directory.
///
/// Returns `Err` if the home directory cannot be determined (`$HOME` unset).
/// This is a hard error — there is no fallback to `"."`.
pub fn content_store_root(
    output_dir: Option<&std::path::Path>,
) -> Result<PathBuf, crate::types::StoreError> {
    let base = match output_dir {
        Some(d) => d.to_path_buf(),
        None => dirs::home_dir().ok_or(crate::types::StoreError::HomeDirUnavailable)?,
    };
    Ok(base.join(".noxa").join("content"))
}

/// Extract the sanitized domain component from a URL (e.g. `"docs_example_com"`).
pub fn domain_from_url(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str().map(|h| {
                let clean = h.strip_prefix("www.").unwrap_or(h);
                sanitize_component(clean)
            })
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_to_store_path_root() {
        let p = try_url_to_store_path("https://example.com/").unwrap();
        assert_eq!(p, PathBuf::from("example_com/index"));
    }

    #[test]
    fn test_url_to_store_path_strips_www() {
        let p = try_url_to_store_path("https://www.rust-lang.org/learn").unwrap();
        assert_eq!(p, PathBuf::from("rust-lang_org/learn"));
    }

    #[test]
    fn test_url_to_store_path_query_discriminates() {
        let p1 = try_url_to_store_path("https://example.com/search?q=rust").unwrap();
        let p2 = try_url_to_store_path("https://example.com/search?q=go").unwrap();
        assert_ne!(p1, p2);
        let p1_str = p1.to_string_lossy();
        assert!(p1_str.starts_with("example_com/search_"));
    }

    #[test]
    fn test_url_to_store_path_long_urls_get_distinct_hash_suffixes() {
        let base = "https://example.com/";
        let repeated = "very-long-segment/".repeat(20);
        let p1 = try_url_to_store_path(&format!("{base}{repeated}alpha")).unwrap();
        let p2 = try_url_to_store_path(&format!("{base}{repeated}beta")).unwrap();

        assert_ne!(p1, p2, "long URLs must not alias after truncation");
        assert!(p1.to_string_lossy().len() <= MAX_REL_PATH_LEN);
        assert!(p2.to_string_lossy().len() <= MAX_REL_PATH_LEN);
    }

    #[test]
    fn test_try_url_to_store_path_rejects_invalid_urls() {
        let err = try_url_to_store_path("not a url").expect_err("invalid URLs must fail closed");
        assert!(matches!(err, crate::types::StoreError::InvalidUrl(_)));
    }

    #[test]
    fn test_store_path_stays_within_root() {
        let p = try_url_to_store_path("https://evil.com/../../../etc/passwd").unwrap();
        assert!(p.to_string_lossy().starts_with("evil_com/"));
    }

    #[test]
    fn test_url_to_store_path_strips_parent_components() {
        use std::path::Component;
        let p = try_url_to_store_path("https://evil.com/a/../../etc/./passwd").unwrap();
        assert!(!p.components().any(|c| matches!(c, Component::ParentDir)));
        assert!(!p.components().any(|c| matches!(c, Component::CurDir)));
    }

    #[test]
    fn test_url_to_store_path_sanitizes_ipv6() {
        let p = try_url_to_store_path("https://[fe80::1]/bad:path/segment").unwrap();
        let s = p.to_string_lossy();
        assert!(s.starts_with("fe80__1/"));
        assert!(!s.contains(':'));
        assert!(!s.contains('['));
        assert!(!s.contains(']'));
    }

    #[test]
    fn test_url_hash_fnv1a() {
        assert_eq!(url_hash("hello"), 0xa430d84680aabd0b);
    }

    #[test]
    fn test_content_store_root_no_output_dir() {
        // Only runs when $HOME is set, which is true in all normal test environments.
        let root = content_store_root(None).expect("home dir available in test env");
        assert!(root.ends_with(".noxa/content"));
    }

    #[test]
    fn test_content_store_root_with_output_dir() {
        let dir = std::path::PathBuf::from("/tmp/mybase");
        assert_eq!(
            content_store_root(Some(&dir)).unwrap(),
            dir.join(".noxa/content")
        );
    }

    #[test]
    fn test_domain_from_url() {
        assert_eq!(
            domain_from_url("https://docs.example.com/api"),
            "docs_example_com"
        );
        assert_eq!(
            domain_from_url("https://www.rust-lang.org/"),
            "rust-lang_org"
        );
    }
}
