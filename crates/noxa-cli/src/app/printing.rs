use super::*;

pub(crate) fn print_output(result: &ExtractionResult, format: &OutputFormat, show_metadata: bool) {
    match format {
        OutputFormat::Markdown => {
            if show_metadata {
                print!("{}", format_frontmatter(&result.metadata));
            }
            println!("{}", result.content.markdown);
            if !result.structured_data.is_empty() {
                println!(
                    "\n## Structured Data\n\n```json\n{}\n```",
                    serde_json::to_string_pretty(&result.structured_data).unwrap_or_default()
                );
            }
        }
        OutputFormat::Json => {
            // serde_json::to_string_pretty won't fail on our types
            println!(
                "{}",
                serde_json::to_string_pretty(result).expect("serialization failed")
            );
        }
        OutputFormat::Text => {
            println!("{}", result.content.plain_text);
        }
        OutputFormat::Llm => {
            println!("{}", to_llm_text(result, result.metadata.url.as_deref()));
        }
        OutputFormat::Html => {
            println!("{}", raw_html_or_markdown(result));
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
        OutputFormat::Markdown => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                println!("---");
                println!("# Page: {}\n", page.url);
                if show_metadata {
                    print!("{}", format_frontmatter(&extraction.metadata));
                }
                println!("{}", extraction.content.markdown);
                println!();
            }
        }
        OutputFormat::Text => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                println!("---");
                println!("# Page: {}\n", page.url);
                println!("{}", extraction.content.plain_text);
                println!();
            }
        }
        OutputFormat::Llm => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                println!("---");
                println!("{}", to_llm_text(extraction, Some(page.url.as_str())));
                println!();
            }
        }
        OutputFormat::Html => {
            for page in &result.pages {
                let Some(ref extraction) = page.extraction else {
                    continue;
                };
                println!("---");
                println!("<!-- Page: {} -->\n", page.url);
                println!("{}", raw_html_or_markdown(extraction));
                println!();
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
        OutputFormat::Markdown => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        println!("---");
                        println!("# {}\n", r.url);
                        if show_metadata {
                            print!("{}", format_frontmatter(&extraction.metadata));
                        }
                        println!("{}", extraction.content.markdown);
                        println!();
                    }
                    Err(e) => {
                        eprintln!("error: {} -- {}", r.url, e);
                    }
                }
            }
        }
        OutputFormat::Text => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        println!("---");
                        println!("# {}\n", r.url);
                        println!("{}", extraction.content.plain_text);
                        println!();
                    }
                    Err(e) => {
                        eprintln!("error: {} -- {}", r.url, e);
                    }
                }
            }
        }
        OutputFormat::Llm => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        println!("---");
                        println!("{}", to_llm_text(extraction, Some(r.url.as_str())));
                        println!();
                    }
                    Err(e) => {
                        eprintln!("error: {} -- {}", r.url, e);
                    }
                }
            }
        }
        OutputFormat::Html => {
            for r in results {
                match &r.result {
                    Ok(extraction) => {
                        println!("---");
                        println!("<!-- {} -->\n", r.url);
                        println!("{}", raw_html_or_markdown(extraction));
                        println!();
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
