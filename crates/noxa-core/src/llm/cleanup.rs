/// Whitespace cleanup, HTML entity decoding, invisible Unicode stripping,
/// leaked JS removal, CSS artifact filtering, and text-level noise removal.
mod alt;
mod assets;
mod basic;
mod css;
#[cfg(test)]
mod tests;
mod ui;
mod word_lists;

#[cfg(test)]
pub(crate) use alt::is_long_alt_description;
pub(crate) use alt::{strip_alt_text_noise, strip_long_alt_descriptions};
pub(crate) use assets::{is_asset_label, strip_asset_labels};
pub(crate) use basic::{
    collapse_spaced_text, collapse_whitespace, decode_html_entities, strip_emphasis,
    strip_invisible_unicode, strip_leaked_js,
};
#[cfg(test)]
pub(crate) use css::is_css_artifact_line;
pub(crate) use css::{strip_css_artifacts, strip_css_class_lines};
#[cfg(test)]
pub(crate) use ui::is_ui_control_line;
pub(crate) use ui::strip_ui_control_text;
pub(crate) use word_lists::{collapse_word_lists, dedup_adjacent_descriptions};
