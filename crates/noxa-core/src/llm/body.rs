/// Body processing pipeline for LLM output.
///
/// Orchestrates the multi-step pipeline that transforms raw markdown into
/// token-efficient LLM text. Each step is implemented in sibling modules.
mod blocks;
mod headings;
mod repeated;
#[cfg(test)]
mod tests;

use super::{cleanup, images, links};
use blocks::{
    dedup_comma_lists, dedup_content_blocks, dedup_lines, merge_stat_lines, strip_empty_code_blocks,
};
use headings::{
    dedup_duplicate_headings, dedup_heading_paragraph, dedup_text_against_headings,
    strip_empty_headings, strip_trailing_empty_headings,
};
#[cfg(test)]
pub(crate) use repeated::collapse_repeated_in_line;
use repeated::dedup_repeated_phrases;

pub(crate) struct ProcessedBody {
    pub text: String,
    pub links: Vec<(String, String)>,
}

pub(crate) fn process_body(markdown: &str) -> ProcessedBody {
    let text = cleanup::decode_html_entities(markdown);
    let text = cleanup::strip_invisible_unicode(&text);
    let text = cleanup::strip_leaked_js(&text);
    let text = cleanup::collapse_spaced_text(&text);
    let text = images::convert_linked_images(&text);
    let text = images::collapse_logo_images(&text);
    let text = images::strip_remaining_images(&text);
    let text = images::strip_bare_image_refs(&text);
    let text = cleanup::strip_emphasis(&text);
    let text = cleanup::strip_alt_text_noise(&text);
    let text = cleanup::strip_ui_control_text(&text);
    let text = cleanup::strip_long_alt_descriptions(&text);
    let text = cleanup::strip_css_artifacts(&text);
    let text = cleanup::collapse_word_lists(&text);
    let text = cleanup::dedup_adjacent_descriptions(&text);
    let (text, extracted_links) = links::extract_and_strip_links(&text);
    let text = dedup_repeated_phrases(&text);
    let text = dedup_heading_paragraph(&text);
    let text = dedup_text_against_headings(&text);
    let text = dedup_duplicate_headings(&text);
    let text = strip_empty_headings(&text);
    let text = cleanup::strip_asset_labels(&text);
    let text = cleanup::strip_css_class_lines(&text);
    let text = cleanup::collapse_whitespace(&text);
    let text = dedup_content_blocks(&text);
    let text = dedup_lines(&text);
    let text = dedup_comma_lists(&text);
    let text = strip_trailing_empty_headings(&text);
    let text = strip_empty_code_blocks(&text);
    let text = cleanup::collapse_whitespace(&text);
    let text = merge_stat_lines(&text);

    ProcessedBody {
        text,
        links: extracted_links,
    }
}
