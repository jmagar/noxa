use super::*;

pub(crate) fn print_output(result: &ExtractionResult, format: &OutputFormat, show_metadata: bool) {
    println!("{}", format_output(result, format, show_metadata));
}

pub(crate) fn print_extractor_catalog(format: &OutputFormat) {
    println!("{}", format_extractor_catalog(format));
}

pub(crate) fn format_extractor_catalog(format: &OutputFormat) -> String {
    let extractors = noxa_fetch::extractors::list();
    match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(&extractors).expect("serialization failed")
        }
        _ => {
            let mut out = String::new();
            for extractor in extractors {
                out.push_str(extractor.name);
                out.push_str(" - ");
                out.push_str(extractor.label);
                out.push('\n');
                out.push_str("  ");
                out.push_str(extractor.description);
                out.push('\n');
                out.push_str("  patterns: ");
                out.push_str(&extractor.url_patterns.join(", "));
                out.push_str("\n\n");
            }
            out.trim_end().to_string()
        }
    }
}

/// Print cloud API response in the requested format.
pub(crate) fn print_cloud_output(resp: &serde_json::Value, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(resp).expect("serialization failed")
            );
        }
        OutputFormat::Markdown => {
            // Cloud response has content.markdown
            if let Some(md) = resp
                .get("content")
                .and_then(|c| c.get("markdown"))
                .and_then(|m| m.as_str())
            {
                println!("{md}");
            } else if let Some(md) = resp.get("markdown").and_then(|m| m.as_str()) {
                println!("{md}");
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(resp).expect("serialization failed")
                );
            }
        }
        OutputFormat::Text => {
            if let Some(txt) = resp
                .get("content")
                .and_then(|c| c.get("plain_text"))
                .and_then(|t| t.as_str())
            {
                println!("{txt}");
            } else {
                // Fallback to markdown or raw JSON
                print_cloud_output(resp, &OutputFormat::Markdown);
            }
        }
        OutputFormat::Llm => {
            if let Some(llm) = resp
                .get("content")
                .and_then(|c| c.get("llm_text"))
                .and_then(|t| t.as_str())
            {
                println!("{llm}");
            } else {
                print_cloud_output(resp, &OutputFormat::Markdown);
            }
        }
        OutputFormat::Html => {
            if let Some(html) = resp
                .get("content")
                .and_then(|c| c.get("raw_html"))
                .and_then(|h| h.as_str())
            {
                println!("{html}");
            } else {
                print_cloud_output(resp, &OutputFormat::Markdown);
            }
        }
    }
}

pub(crate) fn print_diff_output(diff: &ContentDiff, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(diff).expect("serialization failed")
            );
        }
        // For markdown/text/llm, show a human-readable summary
        _ => {
            println!("Status: {:?}", diff.status);
            println!("Word count delta: {:+}", diff.word_count_delta);

            if !diff.metadata_changes.is_empty() {
                println!("\nMetadata changes:");
                for change in &diff.metadata_changes {
                    println!(
                        "  {}: {} -> {}",
                        change.field,
                        change.old.as_deref().unwrap_or("(none)"),
                        change.new.as_deref().unwrap_or("(none)"),
                    );
                }
            }

            if !diff.links_added.is_empty() {
                println!("\nLinks added:");
                for link in &diff.links_added {
                    println!("  + {} ({})", link.href, link.text);
                }
            }

            if !diff.links_removed.is_empty() {
                println!("\nLinks removed:");
                for link in &diff.links_removed {
                    println!("  - {} ({})", link.href, link.text);
                }
            }

            if let Some(ref text_diff) = diff.text_diff {
                println!("\n{text_diff}");
            }
        }
    }
}

pub(crate) fn print_crawl_output(result: &CrawlResult, format: &OutputFormat, show_metadata: bool) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(result).expect("serialization failed")
            );
        }
        _ => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                print_page_section(&page.url, extraction, format, show_metadata);
            }
        }
    }
}

pub(crate) fn print_batch_output(
    results: &[BatchExtractResult],
    format: &OutputFormat,
    show_metadata: bool,
) {
    match format {
        OutputFormat::Json => {
            // Build a JSON array of {url, result?, error?} objects
            let entries: Vec<serde_json::Value> = results
                .iter()
                .map(|r| match &r.result {
                    Ok(extraction) => serde_json::json!({
                        "url": r.url,
                        "result": extraction,
                    }),
                    Err(e) => serde_json::json!({
                        "url": r.url,
                        "error": e.to_string(),
                    }),
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&entries).expect("serialization failed")
            );
        }
        _ => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        print_page_section(&r.url, extraction, format, show_metadata);
                    }
                    Err(e) => {
                        eprintln!("error: {} -- {}", r.url, e);
                    }
                }
            }
        }
    }
}

pub(crate) fn print_map_output(entries: &[SitemapEntry], format: &OutputFormat) {
    println!("{}", format_map_output(entries, format));
}

fn print_page_section(
    url: &str,
    extraction: &ExtractionResult,
    format: &OutputFormat,
    show_metadata: bool,
) {
    println!("---");
    match format {
        OutputFormat::Markdown => {
            println!("# {url}\n");
            print!("{}", format_output(extraction, format, show_metadata));
        }
        OutputFormat::Text => {
            println!("# {url}\n");
            println!("{}", extraction.content.plain_text);
        }
        OutputFormat::Llm => {
            println!("{}", to_llm_text(extraction, Some(url)));
        }
        OutputFormat::Html => {
            println!("<!-- {url} -->\n");
            println!("{}", raw_html_or_markdown(extraction));
        }
        OutputFormat::Json => {
            println!("{}", format_output(extraction, format, show_metadata));
        }
    }
    println!();
}
