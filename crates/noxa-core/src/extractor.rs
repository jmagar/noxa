/// Readability-style content extraction.
/// Strips noise (nav, ads, sidebars), scores remaining nodes by text density
/// and structural signals, then converts the best candidate to markdown.
use once_cell::sync::Lazy;
use scraper::{Html, Selector};
use tracing::debug;
use url::Url;

use crate::markdown;
use crate::types::{Content, ExtractionOptions};

#[cfg(test)]
mod form_integration_tests;
mod include;
mod recovery;
mod scoring;
mod selectors;
#[cfg(test)]
mod tests;

use include::extract_with_include;
use recovery::{
    recover_announcements, recover_footer_cta, recover_footer_sitemap, recover_hero_paragraph,
    recover_section_headings,
};
use scoring::find_best_node;
pub use scoring::word_count;
use selectors::build_exclude_set;

static CANDIDATE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("article, main, [role='main'], div, section, td").unwrap());
static BODY_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("body").unwrap());
static H1_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("h1").unwrap());
static H2_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("h2").unwrap());
static P_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("p").unwrap());
static A_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("a").unwrap());
static ANNOUNCEMENT_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("[role='region'][aria-label]").unwrap());
static FOOTER_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("footer").unwrap());
static FOOTER_HEADING_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("h2, h3, h4, h5, h6").unwrap());
static MAIN_CONTENT_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("article, main, [role='main']").unwrap());

const MAX_SELECTORS: usize = 100;

pub fn extract_content(doc: &Html, base_url: Option<&Url>, options: &ExtractionOptions) -> Content {
    let exclude = build_exclude_set(doc, &options.exclude_selectors);

    if !options.include_selectors.is_empty() {
        return extract_with_include(doc, base_url, &options.include_selectors, &exclude, options);
    }

    if options.only_main_content {
        if let Some(main_el) = doc.select(&MAIN_CONTENT_SELECTOR).next() {
            debug!(
                tag = main_el.value().name(),
                "only_main_content: selected element"
            );
            let (markdown, plain_text, assets) = markdown::convert(main_el, base_url, &exclude);
            let raw_html = if options.include_raw_html {
                Some(main_el.html())
            } else {
                None
            };
            return Content {
                markdown,
                plain_text,
                links: assets.links,
                images: assets.images,
                code_blocks: assets.code_blocks,
                raw_html,
            };
        }
        debug!("only_main_content: no article/main found, falling back to scoring");
    }

    let best = find_best_node(doc);

    let (content_element, mut markdown, plain_text, mut assets) = if let Some(node) = best {
        debug!(tag = node.value().name(), "selected content node");
        let (md, pt, a) = markdown::convert(node, base_url, &exclude);
        (Some(node), md, pt, a)
    } else if let Some(body) = doc.select(&BODY_SELECTOR).next() {
        debug!("no strong candidate, falling back to body");
        let (md, pt, a) = markdown::convert(body, base_url, &exclude);
        (Some(body), md, pt, a)
    } else {
        debug!("no body found, falling back to root element");
        let root = doc.root_element();
        let (md, pt, a) = markdown::convert(root, base_url, &exclude);
        (Some(root), md, pt, a)
    };

    if let Some(h1) = doc.select(&H1_SELECTOR).next() {
        let h1_text = h1
            .text()
            .collect::<String>()
            .trim()
            .trim_end_matches(|c: char| !c.is_alphanumeric())
            .trim()
            .to_string();
        if !h1_text.is_empty() && !markdown.contains(&h1_text) {
            markdown = format!("# {h1_text}\n\n{markdown}");
            recover_hero_paragraph(h1, &mut markdown);
        }
    }

    recover_announcements(doc, base_url, &mut markdown, &mut assets.links);
    recover_section_headings(doc, &mut markdown);
    recover_footer_cta(doc, base_url, &mut markdown, &mut assets.links);
    recover_footer_sitemap(doc, base_url, &mut markdown, &mut assets.links);

    let raw_html = if options.include_raw_html {
        content_element.map(|el| el.html())
    } else {
        None
    };

    Content {
        markdown,
        plain_text,
        links: assets.links,
        images: assets.images,
        code_blocks: assets.code_blocks,
        raw_html,
    }
}
