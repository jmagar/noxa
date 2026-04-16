use std::collections::HashSet;

use ego_tree::NodeId;
use scraper::ElementRef;
use url::Url;

use super::render::{children_to_md, inline_text, node_to_md};
use super::ConvertedAssets;

pub(super) fn list_items(
    list_el: ElementRef<'_>,
    base_url: Option<&Url>,
    assets: &mut ConvertedAssets,
    depth: usize,
    ordered: bool,
    exclude: &HashSet<NodeId>,
    dom_depth: usize,
) -> String {
    let indent = "  ".repeat(depth);
    let mut out = String::new();
    let mut index = 1;

    for child in list_el.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            if exclude.contains(&child_el.id()) {
                continue;
            }
            let tag = child_el.value().name();
            if tag == "li" {
                let bullet = if ordered {
                    let b = format!("{index}.");
                    index += 1;
                    b
                } else {
                    "-".to_string()
                };

                // Separate nested lists from inline content
                let mut inline_parts = String::new();
                let mut nested_lists = String::new();

                for li_child in child_el.children() {
                    if let Some(li_child_el) = ElementRef::wrap(li_child) {
                        if exclude.contains(&li_child_el.id()) {
                            continue;
                        }
                        let child_tag = li_child_el.value().name();
                        if child_tag == "ul" || child_tag == "ol" {
                            nested_lists.push_str(&list_items(
                                li_child_el,
                                base_url,
                                assets,
                                depth + 1,
                                child_tag == "ol",
                                exclude,
                                dom_depth + 1,
                            ));
                        } else {
                            inline_parts.push_str(&node_to_md(
                                li_child_el,
                                base_url,
                                assets,
                                depth,
                                exclude,
                                dom_depth + 1,
                            ));
                        }
                    } else if let Some(text) = li_child.value().as_text() {
                        inline_parts.push_str(text);
                    }
                }

                let text = inline_parts
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!("{indent}{bullet} {text}\n"));

                if !nested_lists.is_empty() {
                    out.push_str(&nested_lists);
                }
            }
        }
    }
    out.trim_end_matches('\n').to_string()
}

/// Check whether a table cell contains block-level elements, indicating a layout
/// table rather than a data table.
pub(super) fn cell_has_block_content(cell: ElementRef<'_>) -> bool {
    const BLOCK_TAGS: &[&str] = &[
        "p",
        "div",
        "ul",
        "ol",
        "blockquote",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "pre",
        "table",
        "section",
        "article",
        "header",
        "footer",
        "nav",
        "aside",
    ];
    for desc in cell.descendants() {
        if let Some(el) = ElementRef::wrap(desc)
            && BLOCK_TAGS.contains(&el.value().name())
        {
            return true;
        }
    }
    false
}

pub(super) fn table_to_md(
    table_el: ElementRef<'_>,
    base_url: Option<&Url>,
    assets: &mut ConvertedAssets,
    exclude: &HashSet<NodeId>,
    depth: usize,
) -> String {
    // Collect all <td>/<th> cells grouped by row, and detect layout tables
    let mut raw_rows: Vec<Vec<ElementRef<'_>>> = Vec::new();
    let mut has_header = false;
    let mut is_layout = false;

    for child in table_el.descendants() {
        if let Some(el) = ElementRef::wrap(child) {
            if exclude.contains(&el.id()) {
                continue;
            }
            if el.value().name() == "tr" {
                let cells: Vec<ElementRef<'_>> = el
                    .children()
                    .filter_map(ElementRef::wrap)
                    .filter(|c| {
                        !exclude.contains(&c.id())
                            && (c.value().name() == "th" || c.value().name() == "td")
                    })
                    .inspect(|&c| {
                        if c.value().name() == "th" {
                            has_header = true;
                        }
                        if !is_layout && cell_has_block_content(c) {
                            is_layout = true;
                        }
                    })
                    .collect();

                if !cells.is_empty() {
                    raw_rows.push(cells);
                }
            }
        }
    }

    if raw_rows.is_empty() {
        return String::new();
    }

    // Layout table: render each cell as a standalone block section
    if is_layout {
        let mut out = String::new();
        for row in &raw_rows {
            for cell in row {
                let content = children_to_md(*cell, base_url, assets, 0, exclude, depth);
                let content = content.trim();
                if !content.is_empty() {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(content);
                }
            }
        }
        return out;
    }

    // Data table: render as markdown table
    let mut rows: Vec<Vec<String>> = raw_rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|c| inline_text(*c, base_url, assets, exclude, depth))
                .collect()
        })
        .collect();

    // Find max column count
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }

    // Normalize row lengths
    for row in &mut rows {
        while row.len() < cols {
            row.push(String::new());
        }
    }

    let mut out = String::new();

    // Header row
    let header = &rows[0];
    out.push_str("| ");
    out.push_str(&header.join(" | "));
    out.push_str(" |\n");

    // Separator
    out.push_str("| ");
    out.push_str(&(0..cols).map(|_| "---").collect::<Vec<_>>().join(" | "));
    out.push_str(" |\n");

    // Data rows (skip first if it was a header)
    let start = if has_header { 1 } else { 0 };
    for row in &rows[start..] {
        out.push_str("| ");
        out.push_str(&row.join(" | "));
        out.push_str(" |\n");
    }

    out.trim_end().to_string()
}
