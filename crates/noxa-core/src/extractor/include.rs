use std::collections::HashSet;

use ego_tree::NodeId;
use scraper::Html;
use url::Url;

use crate::markdown;
use crate::types::{Content, ExtractionOptions};

use super::selectors::parse_selectors;

pub(super) fn extract_with_include(
    doc: &Html,
    base_url: Option<&Url>,
    include_selectors: &[String],
    exclude: &HashSet<NodeId>,
    options: &ExtractionOptions,
) -> Content {
    let selectors = parse_selectors(include_selectors);

    let mut all_md = String::new();
    let mut all_plain = String::new();
    let mut all_links = Vec::new();
    let mut all_images = Vec::new();
    let mut all_code_blocks = Vec::new();
    let mut all_raw_html = if options.include_raw_html {
        Some(String::new())
    } else {
        None
    };

    let mut seen: HashSet<NodeId> = HashSet::new();
    for selector in &selectors {
        for el in doc.select(selector) {
            if exclude.contains(&el.id()) {
                continue;
            }
            // Skip if this exact node was already emitted
            if !seen.insert(el.id()) {
                continue;
            }
            // Skip if an ancestor was already emitted (avoids duplicate nested content)
            if el.ancestors().any(|a| seen.contains(&a.id())) {
                continue;
            }

            let (md, plain, assets) = markdown::convert(el, base_url, exclude);

            if !md.is_empty() {
                if !all_md.is_empty() {
                    all_md.push_str("\n\n");
                }
                all_md.push_str(&md);
            }
            if !plain.is_empty() {
                if !all_plain.is_empty() {
                    all_plain.push('\n');
                }
                all_plain.push_str(&plain);
            }

            all_links.extend(assets.links);
            all_images.extend(assets.images);
            all_code_blocks.extend(assets.code_blocks);

            if let Some(ref mut raw) = all_raw_html {
                raw.push_str(&el.html());
            }
        }
    }

    Content {
        markdown: all_md,
        plain_text: all_plain,
        links: all_links,
        images: all_images,
        code_blocks: all_code_blocks,
        raw_html: all_raw_html,
    }
}
