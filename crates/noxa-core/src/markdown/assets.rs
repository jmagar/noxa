use scraper::{ElementRef, Selector};
use url::Url;

use crate::types::{Image, Link};

use super::{ConvertedAssets, resolve_url};

const KNOWN_LANGS: &[&str] = &[
    "javascript",
    "typescript",
    "python",
    "rust",
    "go",
    "java",
    "c",
    "cpp",
    "csharp",
    "ruby",
    "php",
    "swift",
    "kotlin",
    "scala",
    "shell",
    "bash",
    "zsh",
    "fish",
    "sql",
    "html",
    "css",
    "scss",
    "sass",
    "less",
    "json",
    "yaml",
    "yml",
    "toml",
    "xml",
    "markdown",
    "md",
    "jsx",
    "tsx",
    "vue",
    "svelte",
    "graphql",
    "protobuf",
    "dockerfile",
    "makefile",
    "lua",
    "perl",
    "r",
    "matlab",
    "haskell",
    "elixir",
    "erlang",
    "clojure",
    "dart",
    "zig",
    "nim",
    "wasm",
    "diff",
    "text",
    "plaintext",
    "console",
];

pub(super) fn extract_language_from_class(class: &str) -> Option<String> {
    for cls in class.split_whitespace() {
        // Standard prefixes: language-js, lang-python, highlight-rust
        for prefix in &["language-", "lang-", "highlight-"] {
            if let Some(lang) = cls.strip_prefix(prefix)
                && !lang.is_empty()
                && lang.len() < 20
            {
                return Some(normalize_lang(lang));
            }
        }
        // Sandpack prefix (sp-javascript, sp-python) — validate against known langs
        if let Some(lang) = cls.strip_prefix("sp-") {
            let lower = lang.to_lowercase();
            if KNOWN_LANGS.contains(&lower.as_str()) {
                return Some(normalize_lang(&lower));
            }
        }
        // Bare language name as class: class="javascript" or class="python"
        let lower = cls.to_lowercase();
        if KNOWN_LANGS.contains(&lower.as_str()) {
            return Some(normalize_lang(&lower));
        }
    }
    None
}

/// Normalize language identifiers to common short forms.
fn normalize_lang(lang: &str) -> String {
    match lang.to_lowercase().as_str() {
        "javascript" | "js" => "js".to_string(),
        "typescript" | "ts" => "ts".to_string(),
        "python" | "py" => "python".to_string(),
        "csharp" | "cs" | "c#" => "csharp".to_string(),
        "cpp" | "c++" => "cpp".to_string(),
        "shell" | "bash" | "zsh" | "sh" => "bash".to_string(),
        "yaml" | "yml" => "yaml".to_string(),
        "markdown" | "md" => "markdown".to_string(),
        "plaintext" | "text" => "text".to_string(),
        other => other.to_string(),
    }
}

/// Pick the best (largest) image from an HTML srcset attribute.
/// srcset format: "url1 300w, url2 600w, url3 1200w" or "url1 1x, url2 2x"
pub(super) fn pick_best_srcset(srcset: &str) -> Option<String> {
    let mut best_url = None;
    let mut best_size: u32 = 0;

    for entry in srcset.split(',') {
        let parts: Vec<&str> = entry.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let url = parts[0];
        // Skip data URIs
        if url.starts_with("data:") || url.starts_with("blob:") {
            continue;
        }
        let size = if parts.len() > 1 {
            let descriptor = parts[1];
            // Parse "300w" or "2x"
            descriptor
                .trim_end_matches(|c: char| !c.is_ascii_digit())
                .parse::<u32>()
                .unwrap_or(1)
        } else {
            1
        };
        if size > best_size {
            best_size = size;
            best_url = Some(url.to_string());
        }
    }

    best_url
}

/// Collect images and links from a noise element without adding text to markdown.
/// This preserves valuable metadata (links, images) from nav/header/footer
/// that would otherwise be completely lost.
pub(super) fn collect_assets_from_noise(
    element: ElementRef<'_>,
    base_url: Option<&Url>,
    assets: &mut ConvertedAssets,
) {
    // Collect images with alt text
    for img in element.select(&Selector::parse("img[alt]").unwrap()) {
        let alt = img.value().attr("alt").unwrap_or("").to_string();
        let src = img
            .value()
            .attr("src")
            .map(|s| resolve_url(s, base_url))
            .unwrap_or_default();
        if !src.is_empty() && !alt.is_empty() {
            assets.images.push(Image { alt, src });
        }
    }

    // Collect links
    for link in element.select(&Selector::parse("a[href]").unwrap()) {
        let href = link
            .value()
            .attr("href")
            .map(|h| resolve_url(h, base_url))
            .unwrap_or_default();
        let text: String = link.text().collect::<String>().trim().to_string();
        if !href.is_empty() && !text.is_empty() && href.starts_with("http") {
            assets.links.push(Link { text, href });
        }
    }
}
