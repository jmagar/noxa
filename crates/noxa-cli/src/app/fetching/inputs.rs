use crate::app::*;

pub(crate) fn collect_urls(cli: &Cli) -> Result<Vec<(String, Option<String>)>, String> {
    let mut entries: Vec<(String, Option<String>)> =
        cli.urls.iter().map(|u| (normalize_url(u), None)).collect();

    if let Some(ref path) = cli.urls_file {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Support an optional `url,name` suffix syntax.  Only split on the
            // last comma when the part after it does NOT look like a URL
            // fragment (i.e. doesn't contain "://" or ".").  This preserves
            // bare URLs that contain commas in their query strings
            // (e.g. ?ll=40.7,-74.0) while still allowing a custom display
            // name to be appended after the final comma.
            let custom = if let Some((url_part, name_part)) = trimmed.rsplit_once(',') {
                let name = name_part.trim();
                // If the name part looks like a URL component, don't treat it
                // as a custom name — the whole line is just a URL.
                if !name.is_empty() && !name.contains("://") && !name.contains('.') {
                    entries.push((normalize_url(url_part.trim()), Some(name.to_string())));
                    continue;
                }
                None
            } else {
                None
            };
            entries.push((normalize_url(trimmed), custom));
        }
    }

    Ok(entries)
}

/// Result that can be either a local extraction or a cloud API JSON response.
pub(crate) enum FetchOutput {
    Local(Box<ExtractionResult>),
    Cloud(serde_json::Value),
}

impl FetchOutput {
    /// Get the local ExtractionResult, or try to parse it from the cloud response.
    pub(crate) fn into_extraction(self) -> Result<ExtractionResult, String> {
        match self {
            FetchOutput::Local(r) => Ok(*r),
            FetchOutput::Cloud(resp) => {
                // Cloud response has an "extraction" field with the full ExtractionResult.
                // If the field is present but malformed, surface that specific error rather
                // than silently falling back and hiding the root cause.
                if let Some(v) = resp.get("extraction") {
                    serde_json::from_value(v.clone())
                        .map_err(|e| format!("failed to parse cloud extraction field: {e}"))
                } else {
                    serde_json::from_value(resp)
                        .map_err(|e| format!("could not parse extraction from cloud response: {e}"))
                }
            }
        }
    }
}
