/// HTML-to-markdown converter.
/// Walks the DOM tree and emits clean markdown, resolving relative URLs
/// against the provided base URL when available.
use once_cell::sync::Lazy;
use std::collections::HashSet;

use ego_tree::NodeId;
use scraper::{ElementRef, Selector};
use url::Url;

use crate::types::{CodeBlock, Image, Link};

mod assets;
mod blocks;
mod render;
#[cfg(test)]
mod tests;

pub struct ConvertedAssets {
    pub links: Vec<Link>,
    pub images: Vec<Image>,
    pub code_blocks: Vec<CodeBlock>,
}

const MAX_DOM_DEPTH: usize = 200;
static CODE_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("code").unwrap());

pub fn convert(
    element: ElementRef<'_>,
    base_url: Option<&Url>,
    exclude: &HashSet<NodeId>,
) -> (String, String, ConvertedAssets) {
    let mut assets = ConvertedAssets {
        links: Vec::new(),
        images: Vec::new(),
        code_blocks: Vec::new(),
    };

    let md = render::node_to_md(element, base_url, &mut assets, 0, exclude, 0);
    let plain = strip_markdown(&md);
    let md = collapse_whitespace(&md);
    let plain = collapse_whitespace(&plain);

    (md, plain, assets)
}

pub fn resolve_url(href: &str, base_url: Option<&Url>) -> String {
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("//") {
        return href.to_string();
    }

    if let Some(base) = base_url
        && let Ok(resolved) = base.join(href)
    {
        return resolved.to_string();
    }

    href.to_string()
}

fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut consecutive_newlines = 0;
    let mut in_code_fence = false;

    for line in s.lines() {
        if line.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            consecutive_newlines = 0;
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(line.trim_end());
            result.push('\n');
            continue;
        }

        if in_code_fence {
            result.push_str(line.trim_end());
            result.push('\n');
            continue;
        }

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                result.push('\n');
            }
        } else {
            consecutive_newlines = 0;
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result.trim().to_string()
}

fn strip_markdown(md: &str) -> String {
    let table_syntax_removed = md
        .replace("```", "")
        .replace(['#', '*', '`', '[', ']', '<', '>'], "")
        .replace("](", " ")
        .replace(['|'], " ");

    let mut result = String::with_capacity(table_syntax_removed.len());
    let mut in_code_fence = false;
    for line in table_syntax_removed.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            result.push_str(line);
            result.push('\n');
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    result.trim().to_string()
}
