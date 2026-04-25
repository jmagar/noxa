use scraper::{ElementRef, Html};
use tracing::debug;
use url::Url;

use crate::markdown;
use crate::types::Link;

use super::scoring::is_inside_structural_noise;
use super::{
    A_SELECTOR, ANNOUNCEMENT_SELECTOR, FOOTER_HEADING_SELECTOR, FOOTER_SELECTOR, H2_SELECTOR,
};

pub(super) fn recover_announcements(
    doc: &Html,
    base_url: Option<&Url>,
    markdown: &mut String,
    links: &mut Vec<Link>,
) {
    for el in doc.select(&ANNOUNCEMENT_SELECTOR) {
        let label = el.value().attr("aria-label").unwrap_or("");
        if !label.to_lowercase().contains("announcement") {
            continue;
        }

        let text = el.text().collect::<String>();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() || markdown.contains(&text) {
            continue;
        }

        // Build markdown for the announcement, including any links
        let mut announcement = format!("> **{text}**");
        for a in el.select(&A_SELECTOR) {
            let link_text = a.text().collect::<String>().trim().to_string();
            let href = a
                .value()
                .attr("href")
                .map(|h| markdown::resolve_url(h, base_url))
                .unwrap_or_default();
            if !link_text.is_empty() && !href.is_empty() {
                links.push(Link {
                    text: link_text,
                    href,
                });
            }
        }
        announcement.push_str("\n\n");

        debug!("recovered announcement banner");
        *markdown = format!("{announcement}{markdown}");
    }
}

/// Recover the hero paragraph (mission/tagline) that's near the H1 but inside
/// a noise-stripped container like `<header>`. Walk siblings/cousins of the H1
/// to find a substantial `<p>` that isn't in the markdown.
pub(super) fn recover_hero_paragraph(h1: ElementRef<'_>, markdown: &mut String) {
    // Walk up to find a container that holds both H1 and sibling content
    let mut node = h1.parent();
    for _ in 0..4 {
        let Some(parent) = node else { break };
        let Some(parent_el) = ElementRef::wrap(parent) else {
            node = parent.parent();
            continue;
        };

        // Search <p> descendants of this container, limited to close proximity
        // (direct children and grandchildren only) to avoid pulling in unrelated paragraphs
        for descendant in parent_el.descendants().take(50) {
            let Some(el) = ElementRef::wrap(descendant) else {
                continue;
            };
            if el.value().name() != "p" {
                continue;
            }
            let text = el
                .text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            // Only recover substantial paragraphs (taglines, mission statements)
            if text.len() < 40 || text.len() > 300 {
                continue;
            }
            if markdown.contains(&text) {
                continue;
            }
            // Insert right after the H1 heading line
            debug!(text = text.as_str(), "recovered hero paragraph");
            let insert = format!("\n{text}\n");
            if let Some(pos) = markdown.find('\n') {
                markdown.insert_str(pos + 1, &insert);
            } else {
                markdown.push_str(&insert);
            }
            return;
        }
        node = parent.parent();
    }
}

/// Recover <h2> headings that were stripped because their wrapper div had a
/// noise class like "header". If adjacent content from the same parent section
/// IS in the markdown, the heading should be there too.
pub(super) fn recover_section_headings(doc: &Html, markdown: &mut String) {
    for h2 in doc.select(&H2_SELECTOR) {
        let h2_text = h2.text().collect::<String>().trim().to_string();
        if h2_text.is_empty() || find_content_position(markdown, &h2_text).is_some() {
            continue;
        }

        // Don't recover headings inside structural noise tags (nav, aside, footer,
        // header). These are genuine noise — not false-positive class matches like
        // <div class="section-header"> inside a content section.
        if is_inside_structural_noise(h2) {
            continue;
        }

        // Walk up to the nearest section/div parent, then check if any sibling
        // content from that parent made it into the markdown.
        let anchor = find_sibling_anchor_text(h2, markdown);
        if let Some(anchor) = anchor {
            debug!(
                heading = h2_text.as_str(),
                "recovered stripped section heading"
            );
            // Insert the heading before the anchor's content block.
            // Walk backwards past short orphan lines (stat numbers etc.)
            // that likely belong to the same section.
            if let Some(pos) = find_content_position(markdown, &anchor) {
                let line_start = markdown[..pos].rfind('\n').map_or(0, |p| p + 1);
                let insert_pos = walk_back_past_orphans(markdown, line_start);
                let heading_md = format!("## {h2_text}\n\n");
                markdown.insert_str(insert_pos, &heading_md);
            }
        }
    }

    // Also recover <p> "eyebrow" text (short taglines above section headings).
    // These are typically inside the same noise-stripped wrapper as the <h2>.
    // Eyebrows are short (e.g., "/the web access layer for agents") — skip full paragraphs.
    for h2 in doc.select(&H2_SELECTOR) {
        let h2_text = h2.text().collect::<String>().trim().to_string();
        if h2_text.is_empty() || find_content_position(markdown, &h2_text).is_none() {
            continue;
        }

        // Look for a preceding <p> sibling inside the same parent
        if let Some(parent) = h2.parent().and_then(ElementRef::wrap) {
            for child in parent.children() {
                if let Some(child_el) = ElementRef::wrap(child) {
                    // Stop when we reach the h2 itself
                    if child_el == h2 {
                        break;
                    }
                    if child_el.value().name() == "p" {
                        let p_text = child_el.text().collect::<String>().trim().to_string();
                        // Only short text qualifies as an eyebrow — full paragraphs
                        // are regular content, not taglines.
                        if p_text.is_empty() || p_text.len() > 80 {
                            continue;
                        }
                        // Skip decorative route-style labels (e.g., "/proof is in
                        // the numbers", "/press room") — common design pattern, not content.
                        if p_text.starts_with('/') {
                            continue;
                        }
                        // Check against a stripped version of the markdown to handle
                        // formatting like **bold** that breaks plain-text matching.
                        let plain_md = strip_md_formatting(markdown);
                        if plain_md.contains(&p_text) {
                            continue;
                        }
                        {
                            // Insert the eyebrow text at the start of the heading's line
                            if let Some(pos) = find_content_position(markdown, &h2_text) {
                                let line_start = markdown[..pos].rfind('\n').map_or(0, |p| p + 1);
                                let eyebrow_md = format!("*{p_text}*\n\n");
                                markdown.insert_str(line_start, &eyebrow_md);
                                debug!(eyebrow = p_text.as_str(), "recovered eyebrow text");
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Find text from a sibling element (in the same section) that IS in the markdown.
/// This confirms the heading belongs to content we already captured.
fn find_sibling_anchor_text(heading: ElementRef<'_>, markdown: &str) -> Option<String> {
    let heading_text = heading.text().collect::<String>();

    // Walk up to find the containing section or significant parent
    let mut node = heading.parent();
    while let Some(parent) = node {
        if let Some(parent_el) = ElementRef::wrap(parent) {
            let tag = parent_el.value().name();
            if tag == "section" || tag == "article" || tag == "main" || tag == "body" {
                // Search descendant <p> and <h3> elements for text in the markdown.
                // Using specific elements avoids the multiline blob issue from
                // concatenating all text nodes of a large container.
                for descendant in parent_el.descendants() {
                    if let Some(el) = ElementRef::wrap(descendant) {
                        let dtag = el.value().name();
                        if dtag != "p" && dtag != "h3" && dtag != "h4" {
                            continue;
                        }
                        // Normalize whitespace to match how the markdown converter collapses it
                        let el_text: String = el
                            .text()
                            .collect::<String>()
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        // Skip if this text is part of the heading itself
                        if el_text.is_empty() || heading_text.contains(&el_text) {
                            continue;
                        }
                        if el_text.len() > 15 && find_content_position(markdown, &el_text).is_some()
                        {
                            return Some(el_text);
                        }
                    }
                }
                break;
            }
        }
        node = parent.parent();
    }
    None
}

/// Recover CTA (call-to-action) links and headings from footer sections.
/// Many sites have a "hero" CTA block in the footer with documentation links
/// or signup prompts. These are valuable content, not navigational noise.
pub(super) fn recover_footer_cta(
    doc: &Html,
    base_url: Option<&Url>,
    markdown: &mut String,
    links: &mut Vec<Link>,
) {
    for footer in doc.select(&FOOTER_SELECTOR) {
        // Look for h2 headings in the footer (CTA headings like "Power your AI...")
        for h2 in footer.select(&H2_SELECTOR) {
            let h2_text = h2.text().collect::<String>().trim().to_string();
            if h2_text.is_empty() || markdown.contains(&h2_text) {
                continue;
            }
            // Skip meta headings (screen-reader-only "Footer", "Navigation")
            let h2_lower = h2_text.to_lowercase();
            if h2_lower == "footer" || h2_lower == "navigation" || h2_lower == "site map" {
                continue;
            }
            // Skip screen-reader-only headings (sr-only, visually-hidden)
            if let Some(class) = h2.value().attr("class") {
                let cl = class.to_lowercase();
                if cl.contains("sr-only")
                    || cl.contains("visually-hidden")
                    || cl.contains("screen-reader")
                {
                    continue;
                }
            }

            debug!(heading = h2_text.as_str(), "recovered footer CTA heading");
            // Normalize leading newlines to avoid buildup (e.g. after recover_footer_sitemap)
            let trimmed_tail = markdown.trim_end_matches('\n');
            markdown.truncate(trimmed_tail.len());
            markdown.push_str(&format!("\n\n## {h2_text}\n\n"));
        }

        // Recover links that point to documentation or app URLs
        for a in footer.select(&A_SELECTOR) {
            let href = match a.value().attr("href") {
                Some(h) => markdown::resolve_url(h, base_url),
                None => continue,
            };
            let text = a.text().collect::<String>().trim().to_string();
            if text.is_empty() || href.is_empty() {
                continue;
            }

            // Only recover links to docs/app/API — not generic footer nav
            let href_lower = href.to_lowercase();
            let is_valuable_cta = href_lower.contains("docs.")
                || href_lower.contains("/docs")
                || href_lower.contains("app.")
                || href_lower.contains("/app")
                || href_lower.contains("api.");

            if is_valuable_cta && !markdown.contains(&text) {
                debug!(
                    text = text.as_str(),
                    href = href.as_str(),
                    "recovered footer CTA link"
                );
                markdown.push_str(&format!("[{text}]({href})\n\n"));
                links.push(Link {
                    text: text.clone(),
                    href: href.clone(),
                });
            }
        }
    }
}

/// Recover structured site navigation from footer when it has organized
/// link categories (Products, Solutions, Resources, etc.). This captures
/// the site's offering structure — useful for LLM queries like "what does
/// this company offer?" Only fires when the footer has 3+ categories.
pub(super) fn recover_footer_sitemap(
    doc: &Html,
    base_url: Option<&Url>,
    markdown: &mut String,
    links: &mut Vec<Link>,
) {
    for footer in doc.select(&FOOTER_SELECTOR) {
        let mut categories: Vec<(String, Vec<(String, String)>)> = Vec::new();

        for heading in footer.select(&FOOTER_HEADING_SELECTOR) {
            let heading_text = heading.text().collect::<String>().trim().to_string();
            if heading_text.is_empty() || heading_text.len() > 50 {
                continue;
            }
            // Skip meta headings like "Footer" and headings already in the markdown
            if heading_text.eq_ignore_ascii_case("footer") || markdown.contains(&heading_text) {
                continue;
            }

            // Find links in the nearest container that holds both heading + link list.
            // Try parent first, then grandparent (handles wrapper divs).
            let cat_links = collect_sibling_links(heading, base_url);
            // 2–20 links: too few = not a real category, too many = aggregate container
            if cat_links.len() >= 2 && cat_links.len() <= 20 {
                categories.push((heading_text, cat_links));
            }
        }

        if categories.len() < 3 {
            continue;
        }

        // Build compact sitemap — category name + comma-separated link text
        let mut sitemap = String::from("\n\n---\n\n");
        for (heading, cat_links) in &categories {
            let names: Vec<&str> = cat_links.iter().map(|(t, _)| t.as_str()).collect();
            sitemap.push_str(&format!("**{heading}**: {}\n", names.join(", ")));

            for (text, href) in cat_links {
                links.push(Link {
                    text: text.clone(),
                    href: href.clone(),
                });
            }
        }

        debug!(categories = categories.len(), "recovered footer sitemap");
        markdown.push_str(&sitemap);
    }
}

/// Collect links from the same container as a heading element.
/// Limits to links that are siblings of the heading (direct children of the parent,
/// or inside sibling list elements), avoiding mixing in links from adjacent categories.
fn collect_sibling_links(heading: ElementRef<'_>, base_url: Option<&Url>) -> Vec<(String, String)> {
    let mut node = heading.parent();
    // Try up to 2 levels (parent, grandparent) to find a link container
    for _ in 0..2 {
        let Some(parent) = node else { break };
        let Some(parent_el) = ElementRef::wrap(parent) else {
            node = parent.parent();
            continue;
        };
        // Collect only links that are direct children of parent, or inside
        // direct-child list/div elements (i.e. first-level descendants only).
        // This prevents mixing links from adjacent categories in the same footer row.
        let mut a_elements: Vec<_> = Vec::new();
        for sibling in parent_el.children().filter_map(ElementRef::wrap) {
            if sibling == heading {
                continue;
            }
            let tag = sibling.value().name();
            if tag == "a" {
                a_elements.push(sibling);
            } else if matches!(tag, "ul" | "ol" | "div" | "nav" | "p") {
                // Collect links from first-level containers only
                for child in sibling.children().filter_map(ElementRef::wrap) {
                    let ctag = child.value().name();
                    if ctag == "a" {
                        a_elements.push(child);
                    } else if ctag == "li" {
                        for li_child in child.children().filter_map(ElementRef::wrap) {
                            if li_child.value().name() == "a" {
                                a_elements.push(li_child);
                            }
                        }
                    }
                }
            }
        }
        if a_elements.len() >= 2 {
            return a_elements
                .into_iter()
                .filter_map(|a| {
                    let text = a.text().collect::<String>().trim().to_string();
                    let href = a
                        .value()
                        .attr("href")
                        .map(|h| markdown::resolve_url(h, base_url));
                    match (text.is_empty(), href) {
                        (false, Some(h))
                            if !h.is_empty()
                                && text.len() > 1
                                && text.len() < 60
                                && !matches!(
                                    text.to_lowercase().as_str(),
                                    "here" | "link" | "click" | "more"
                                ) =>
                        {
                            Some((text, h))
                        }
                        _ => None,
                    }
                })
                .collect();
        }
        node = parent.parent();
    }
    Vec::new()
}

/// Walk backwards from `pos` in markdown, skipping blank lines and short
/// orphan lines (<=25 chars, likely stat numbers or labels) that belong to
/// the same section. Stops at headings, long content lines, or start of string.
fn walk_back_past_orphans(markdown: &str, mut pos: usize) -> usize {
    loop {
        if pos == 0 {
            break;
        }
        // Find the previous line
        let prev_end = pos.saturating_sub(1); // skip the \n
        let prev_start = markdown[..prev_end].rfind('\n').map_or(0, |p| p + 1);
        let prev_line = markdown[prev_start..prev_end].trim();

        if prev_line.is_empty() {
            pos = prev_start;
            continue;
        }
        if prev_line.starts_with('#') || prev_line.starts_with('>') || prev_line.len() > 25 {
            break;
        }
        // Short non-structural line — likely a stat number, include it
        pos = prev_start;
    }
    pos
}

/// Quick strip of markdown bold/italic markers for plain-text comparison.
fn strip_md_formatting(md: &str) -> String {
    md.replace("**", "").replace('*', "")
}

/// Find `needle` in `markdown` only at a position that isn't inside image/link
/// alt text (`![...](...)`). Returns the byte offset or None.
fn find_content_position(markdown: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }

    let mut search_from = 0;
    while let Some(pos) = markdown[search_from..].find(needle) {
        let abs_pos = search_from + pos;
        if !is_inside_image_syntax(markdown, abs_pos) {
            return Some(abs_pos);
        }
        search_from = abs_pos + needle.len();
    }
    None
}

/// Check if a position in markdown falls inside `![...](...)` image syntax.
fn is_inside_image_syntax(markdown: &str, pos: usize) -> bool {
    let before = &markdown[..pos];
    // Find the last `![` before pos
    if let Some(open) = before.rfind("![") {
        let between = &markdown[open + 2..pos];
        // If there's no `](` between the `![` and pos, and there is a `](`
        // somewhere after pos, then pos is inside the alt-text of an image.
        if !between.contains("](") && markdown[pos..].contains("](") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_content_position_skips_rejected_multibyte_image_alt_match() {
        let markdown = "![тест](https://example.com/image.png)\n\nтест";

        let pos = find_content_position(markdown, "тест").expect("visible text should be found");

        assert_eq!(pos, markdown.rfind("тест").unwrap());
    }

    #[test]
    fn find_content_position_handles_repeated_rejected_non_ascii_matches() {
        let markdown = concat!(
            "![заголовок](https://example.com/one.png)\n",
            "![заголовок](https://example.com/two.png)\n\n",
            "заголовок"
        );

        let pos =
            find_content_position(markdown, "заголовок").expect("visible text should be found");

        assert_eq!(pos, markdown.rfind("заголовок").unwrap());
    }
}
