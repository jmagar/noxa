use once_cell::sync::Lazy;
use regex::Regex;

pub(crate) fn strip_alt_text_noise(input: &str) -> String {
    let mut in_code = false;
    input
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                in_code = !in_code;
            }
            if in_code {
                return true;
            }
            if trimmed.starts_with('#') || trimmed.starts_with('-') || trimmed.starts_with('>') {
                return true;
            }
            !is_alt_text_noise(trimmed)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_alt_text_noise(line: &str) -> bool {
    if line.is_empty() {
        return false;
    }
    is_descriptive_alt_prefix(line)
        || is_broken_image_fragment(line)
        || is_social_avatar_label(line)
        || is_repeated_brand_list(line)
}

fn is_descriptive_alt_prefix(line: &str) -> bool {
    let lower = line.to_lowercase();
    let prefixes = [
        "image of ",
        "photo of ",
        "animation of ",
        "interactive animation of ",
        "screenshot of ",
        "illustration of ",
        "illustration ",
        "picture of ",
        "a image of ",
        "a photo of ",
        "an image of ",
        "an illustration ",
        "an animation ",
        "a screenshot ",
        "a rendering ",
        "a graphic ",
        "a diagram ",
    ];
    prefixes.iter().any(|p| lower.starts_with(p)) && line.split_whitespace().count() >= 4
}

fn is_broken_image_fragment(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.split_whitespace().all(|token| {
        let t = token.trim();
        if t.is_empty() {
            return true;
        }
        let exts = [
            ".webp)", ".svg)", ".png)", ".jpg)", ".jpeg)", ".gif)", ".avif)",
        ];
        exts.iter()
            .any(|ext| t.ends_with(ext) && t.len() <= ext.len() + 5)
    })
}

fn is_social_avatar_label(line: &str) -> bool {
    let lower = line.to_lowercase();
    if lower.matches("twitter image").count() >= 3 {
        return true;
    }
    let handle_count = line.split_whitespace().filter(|w| w.starts_with('@')).count();
    let avatar_count = lower.matches("avatar").count();
    handle_count >= 3 && avatar_count >= 2
}

fn is_repeated_brand_list(line: &str) -> bool {
    use std::collections::HashMap;

    let items: Vec<&str> = line
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if items.len() < 4 {
        return false;
    }
    let multi_word: Vec<&str> = items
        .iter()
        .filter(|i| i.split_whitespace().count() >= 2)
        .copied()
        .collect();
    if multi_word.len() < 4 {
        return false;
    }
    let mut first_words: HashMap<&str, usize> = HashMap::new();
    for item in &multi_word {
        if let Some(fw) = item.split_whitespace().next() {
            *first_words.entry(fw).or_insert(0) += 1;
        }
    }
    first_words.values().any(|&count| count * 2 > multi_word.len())
}

pub(crate) fn strip_long_alt_descriptions(input: &str) -> String {
    static ELEMENT_DESC_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"This element contains [^.]*\.[^.]*\.(?:\s*[^.]*\.)*").unwrap());

    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        if is_long_alt_description(line) {
            continue;
        }
        let cleaned = ELEMENT_DESC_RE.replace_all(line, "");
        let cleaned = cleaned.trim_end();
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(cleaned);
    }
    out
}

pub(crate) fn is_long_alt_description(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 80 {
        return false;
    }
    if trimmed.starts_with('#') || trimmed.starts_with('-') || trimmed.starts_with('>') {
        return false;
    }

    let lower = trimmed.to_lowercase();
    const ALT_PREFIXES: &[&str] = &[
        "an illustration ",
        "an image ",
        "a screenshot ",
        "a photo ",
        "a picture ",
        "a diagram ",
        "a graphic ",
        "a rendering ",
        "an animation ",
        "an icon ",
        "this element contains ",
        "this image shows ",
        "this image depicts ",
    ];

    ALT_PREFIXES.iter().any(|p| lower.starts_with(p))
}
