use std::collections::HashSet;

use ego_tree::NodeId;
use scraper::ElementRef;
use scraper::node::Node;
use url::Url;

use crate::noise;
use crate::types::{CodeBlock, Image, Link};

use super::assets::{collect_assets_from_noise, extract_language_from_class, pick_best_srcset};
use super::blocks::{cell_has_block_content, list_items, table_to_md};
use super::{CODE_SELECTOR, ConvertedAssets, MAX_DOM_DEPTH, resolve_url};

pub(super) fn node_to_md(
    element: ElementRef<'_>,
    base_url: Option<&Url>,
    assets: &mut ConvertedAssets,
    list_depth: usize,
    exclude: &HashSet<NodeId>,
    depth: usize,
) -> String {
    if exclude.contains(&element.id()) {
        return String::new();
    }

    // Guard against deeply nested DOM trees (e.g., Express.co.uk live blogs).
    if depth > MAX_DOM_DEPTH {
        return collect_text(element);
    }

    if noise::is_noise(element) || noise::is_noise_descendant(element) {
        // Still collect images and links from noise elements — they're useful
        // metadata even though we don't include the noise text in markdown.
        // We strip noise text but preserve link/image references as metadata.
        collect_assets_from_noise(element, base_url, assets);
        return String::new();
    }

    let tag = element.value().name();
    match tag {
        // Headings
        "h1" => format!(
            "\n\n# {}\n\n",
            inline_text(element, base_url, assets, exclude, depth)
        ),
        "h2" => format!(
            "\n\n## {}\n\n",
            inline_text(element, base_url, assets, exclude, depth)
        ),
        "h3" => format!(
            "\n\n### {}\n\n",
            inline_text(element, base_url, assets, exclude, depth)
        ),
        "h4" => format!(
            "\n\n#### {}\n\n",
            inline_text(element, base_url, assets, exclude, depth)
        ),
        "h5" => format!(
            "\n\n##### {}\n\n",
            inline_text(element, base_url, assets, exclude, depth)
        ),
        "h6" => format!(
            "\n\n###### {}\n\n",
            inline_text(element, base_url, assets, exclude, depth)
        ),

        // Paragraph
        "p" => format!(
            "\n\n{}\n\n",
            inline_text(element, base_url, assets, exclude, depth)
        ),

        // Links
        "a" => {
            let text = inline_text(element, base_url, assets, exclude, depth);
            let href = element
                .value()
                .attr("href")
                .map(|h| resolve_url(h, base_url))
                .unwrap_or_default();

            if !text.is_empty() && !href.is_empty() {
                assets.links.push(Link {
                    text: text.clone(),
                    href: href.clone(),
                });
                format!("[{text}]({href})")
            } else if !text.is_empty() {
                text
            } else {
                String::new()
            }
        }

        // Images — handle lazy loading (data-src), srcset, and skip base64/blob
        "img" => {
            let alt = element.value().attr("alt").unwrap_or("").to_string();

            // Resolve src: prefer src, fall back to data-src (lazy loading),
            // then data-lazy-src, data-original (common lazy load patterns)
            let raw_src = element
                .value()
                .attr("src")
                .or_else(|| element.value().attr("data-src"))
                .or_else(|| element.value().attr("data-lazy-src"))
                .or_else(|| element.value().attr("data-original"))
                .unwrap_or("");

            // Skip base64 data URIs and blob URLs (they bloat markdown).
            // Use case-insensitive checks per RFC 3986 (schemes are case-insensitive).
            let src = if raw_src.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:"))
                || raw_src.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("blob:"))
            {
                String::new()
            } else {
                resolve_url(raw_src, base_url)
            };

            // Try srcset for better resolution image
            let src = if src.is_empty() {
                // No src found, try srcset
                element
                    .value()
                    .attr("srcset")
                    .and_then(pick_best_srcset)
                    .map(|s| resolve_url(&s, base_url))
                    .unwrap_or_default()
            } else {
                src
            };

            if !src.is_empty() {
                assets.images.push(Image {
                    alt: alt.clone(),
                    src: src.clone(),
                });
                format!("![{alt}]({src})")
            } else {
                String::new()
            }
        }

        // Bold — if it contains block elements (e.g., Drudge wraps entire columns
        // in <b>), treat as a container instead of inline bold.
        "strong" | "b" => {
            if cell_has_block_content(element) {
                children_to_md(element, base_url, assets, list_depth, exclude, depth)
            } else {
                format!(
                    "**{}**",
                    inline_text(element, base_url, assets, exclude, depth)
                )
            }
        }

        // Italic — same block-content check as bold.
        "em" | "i" => {
            if cell_has_block_content(element) {
                children_to_md(element, base_url, assets, list_depth, exclude, depth)
            } else {
                format!(
                    "*{}*",
                    inline_text(element, base_url, assets, exclude, depth)
                )
            }
        }

        // Inline code
        "code" => {
            // If parent is <pre>, this is handled by the "pre" arm
            if is_inside_pre(element) {
                // Just return raw text — the pre handler wraps it
                collect_text(element)
            } else {
                let text = collect_text(element);
                if text.is_empty() {
                    String::new()
                } else {
                    format!("`{text}`")
                }
            }
        }

        // Fenced code blocks
        "pre" => {
            let code_el = element.select(&CODE_SELECTOR).next();
            let (code, lang) = if let Some(code_el) = code_el {
                // Try <code> class first, then fall back to <pre> class
                let lang = code_el
                    .value()
                    .attr("class")
                    .and_then(extract_language_from_class)
                    .or_else(|| {
                        element
                            .value()
                            .attr("class")
                            .and_then(extract_language_from_class)
                    });
                (collect_preformatted_text(code_el, depth), lang)
            } else {
                let lang = element
                    .value()
                    .attr("class")
                    .and_then(extract_language_from_class);
                (collect_preformatted_text(element, depth), lang)
            };

            let code = code.trim_matches('\n').to_string();
            assets.code_blocks.push(CodeBlock {
                language: lang.clone(),
                code: code.clone(),
            });

            let fence_lang = lang.as_deref().unwrap_or("");
            // If the code body contains backtick runs, use a longer fence to avoid
            // premature termination of the fenced block.
            let max_backticks = code
                .chars()
                .fold((0usize, 0usize), |(max, run), c| {
                    if c == '`' {
                        (max.max(run + 1), run + 1)
                    } else {
                        (max, 0)
                    }
                })
                .0;
            let fence = "`".repeat(max_backticks.max(2) + 1);
            format!("\n\n{fence}{fence_lang}\n{code}\n{fence}\n\n")
        }

        // Blockquote
        "blockquote" => {
            let inner = children_to_md(element, base_url, assets, list_depth, exclude, depth);
            let quoted = inner
                .trim()
                .lines()
                .map(|line| format!("> {line}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!("\n\n{quoted}\n\n")
        }

        // Unordered list
        "ul" => {
            let items = list_items(element, base_url, assets, list_depth, false, exclude, depth);
            format!("\n\n{items}\n\n")
        }

        // Ordered list
        "ol" => {
            let items = list_items(element, base_url, assets, list_depth, true, exclude, depth);
            format!("\n\n{items}\n\n")
        }

        // List item — handled by ul/ol parent, but if encountered standalone:
        "li" => {
            let text = inline_text(element, base_url, assets, exclude, depth);
            format!("- {text}\n")
        }

        // Horizontal rule
        "hr" => "\n\n---\n\n".to_string(),

        // Line break
        "br" => "\n".to_string(),

        // Table
        "table" => format!(
            "\n\n{}\n\n",
            table_to_md(element, base_url, assets, exclude, depth)
        ),

        // Divs and other containers — just recurse
        _ => children_to_md(element, base_url, assets, list_depth, exclude, depth),
    }
}

/// Collect markdown from all children of an element.
pub(super) fn children_to_md(
    element: ElementRef<'_>,
    base_url: Option<&Url>,
    assets: &mut ConvertedAssets,
    list_depth: usize,
    exclude: &HashSet<NodeId>,
    depth: usize,
) -> String {
    let mut out = String::new();
    for child in element.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    let chunk =
                        node_to_md(child_el, base_url, assets, list_depth, exclude, depth + 1);
                    if !chunk.is_empty() && !out.is_empty() && needs_separator(&out, &chunk) {
                        out.push(' ');
                    }
                    out.push_str(&chunk);
                }
            }
            Node::Text(text) => {
                out.push_str(text);
            }
            _ => {}
        }
    }
    out
}

/// Collect inline text — walks children, converting inline elements to markdown.
/// This is for contexts where we want inline content (headings, paragraphs, links).
pub(super) fn inline_text(
    element: ElementRef<'_>,
    base_url: Option<&Url>,
    assets: &mut ConvertedAssets,
    exclude: &HashSet<NodeId>,
    depth: usize,
) -> String {
    let mut out = String::new();
    for child in element.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    let chunk = node_to_md(child_el, base_url, assets, 0, exclude, depth + 1);
                    if !chunk.is_empty() && !out.is_empty() && needs_separator(&out, &chunk) {
                        out.push(' ');
                    }
                    out.push_str(&chunk);
                }
            }
            Node::Text(text) => {
                out.push_str(text);
            }
            _ => {}
        }
    }
    // Collapse internal whitespace for inline content
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Check whether a space is needed between two adjacent chunks of output.
/// Returns true when the left side doesn't end with whitespace and the right
/// side doesn't start with whitespace — i.e., two words would be mashed together.
fn needs_separator(left: &str, right: &str) -> bool {
    let l = left.as_bytes().last().copied().unwrap_or(b' ');
    let r = right.as_bytes().first().copied().unwrap_or(b' ');
    !l.is_ascii_whitespace() && !r.is_ascii_whitespace()
}

/// Collect raw text content (no markdown formatting).
fn collect_text(element: ElementRef<'_>) -> String {
    element.text().collect::<String>()
}

/// Collect text from a preformatted element, preserving all whitespace.
/// Every text node is pushed verbatim -- no trimming, no collapsing.
/// Handles `<br>` as newlines and inserts newlines between block-level children
/// (e.g., `<div>` lines produced by some syntax highlighters).
fn collect_preformatted_text(element: ElementRef<'_>, depth: usize) -> String {
    if depth > MAX_DOM_DEPTH {
        return element.text().collect::<String>();
    }
    let mut out = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(text) => out.push_str(text),
            Node::Element(el) => {
                let tag = el.name.local.as_ref();
                if tag == "br" {
                    out.push('\n');
                } else if let Some(child_el) = ElementRef::wrap(child) {
                    if tag == "div" || tag == "p" {
                        if !out.is_empty() && !out.ends_with('\n') {
                            out.push('\n');
                        }
                        out.push_str(&collect_preformatted_text(child_el, depth + 1));
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    } else {
                        out.push_str(&collect_preformatted_text(child_el, depth + 1));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn is_inside_pre(element: ElementRef<'_>) -> bool {
    let mut node = element.parent();
    while let Some(parent) = node {
        if let Some(parent_el) = ElementRef::wrap(parent)
            && parent_el.value().name() == "pre"
        {
            return true;
        }
        node = parent.parent();
    }
    false
}
