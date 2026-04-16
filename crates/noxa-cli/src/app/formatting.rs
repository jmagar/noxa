use super::*;

pub(crate) fn clamp_search_scrape_concurrency(concurrency: usize) -> usize {
    concurrency.clamp(1, 20)
}

/// Get raw HTML from an extraction result, falling back to markdown if unavailable.
pub(crate) fn raw_html_or_markdown(result: &ExtractionResult) -> &str {
    result
        .content
        .raw_html
        .as_deref()
        .unwrap_or(&result.content.markdown)
}

/// Format an `ExtractionResult` into a string for the given output format.
pub(crate) fn format_output(
    result: &ExtractionResult,
    format: &OutputFormat,
    show_metadata: bool,
) -> String {
    match format {
        OutputFormat::Markdown => {
            let mut out = String::new();
            if show_metadata {
                out.push_str(&format_frontmatter(&result.metadata));
            }
            out.push_str(&result.content.markdown);
            if !result.structured_data.is_empty() {
                out.push_str("\n\n## Structured Data\n\n```json\n");
                out.push_str(
                    &serde_json::to_string_pretty(&result.structured_data).unwrap_or_default(),
                );
                out.push_str("\n```");
            }
            out
        }
        OutputFormat::Json => serde_json::to_string_pretty(result).expect("serialization failed"),
        OutputFormat::Text => result.content.plain_text.clone(),
        OutputFormat::Llm => to_llm_text(result, result.metadata.url.as_deref()),
        OutputFormat::Html => raw_html_or_markdown(result).to_string(),
    }
}

#[cfg(test)]
pub(crate) fn default_search_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".noxa")
        .join("search")
}

pub(crate) fn format_cloud_output(resp: &serde_json::Value, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(resp).expect("serialization failed"),
        OutputFormat::Markdown => resp
            .get("content")
            .and_then(|c| c.get("markdown"))
            .and_then(|m| m.as_str())
            .or_else(|| resp.get("markdown").and_then(|m| m.as_str()))
            .map(str::to_string)
            .unwrap_or_else(|| serde_json::to_string_pretty(resp).expect("serialization failed")),
        OutputFormat::Text => resp
            .get("content")
            .and_then(|c| c.get("plain_text"))
            .and_then(|t| t.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format_cloud_output(resp, &OutputFormat::Markdown)),
        OutputFormat::Llm => resp
            .get("content")
            .and_then(|c| c.get("llm_text"))
            .and_then(|t| t.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format_cloud_output(resp, &OutputFormat::Markdown)),
        OutputFormat::Html => resp
            .get("content")
            .and_then(|c| c.get("raw_html"))
            .and_then(|h| h.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format_cloud_output(resp, &OutputFormat::Markdown)),
    }
}

pub(crate) fn format_map_output(entries: &[SitemapEntry], format: &OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(entries).expect("serialization failed"),
        _ => {
            let mut out = String::new();
            for entry in entries {
                out.push_str(&entry.url);
                out.push('\n');
            }
            out
        }
    }
}

pub(crate) fn format_frontmatter(meta: &Metadata) -> String {
    let mut lines = vec!["---".to_string()];

    if let Some(title) = &meta.title {
        lines.push(format!("title: \"{title}\""));
    }
    if let Some(author) = &meta.author {
        lines.push(format!("author: \"{author}\""));
    }
    if let Some(date) = &meta.published_date {
        lines.push(format!("date: \"{date}\""));
    }
    if let Some(url) = &meta.url {
        lines.push(format!("source: \"{url}\""));
    }
    if meta.word_count > 0 {
        lines.push(format!("word_count: {}", meta.word_count));
    }

    lines.push("---".to_string());
    lines.push(String::new()); // blank line after frontmatter
    lines.join("\n")
}

pub(crate) fn format_progress(page: &PageResult, index: usize, max_pages: usize) -> String {
    let status = if page.error.is_some() { "ERR" } else { "OK " };
    let timing = format!("{}ms", page.elapsed.as_millis());
    let detail = if let Some(ref extraction) = page.extraction {
        format!(", {} words", extraction.metadata.word_count)
    } else if let Some(ref err) = page.error {
        format!(" ({err})")
    } else {
        String::new()
    };
    format!(
        "[{index}/{max_pages}] {status} {} ({timing}{detail})",
        page.url
    )
}
