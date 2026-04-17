use std::collections::HashMap;

use super::*;

/// Generic font families that aren't brand-specific.
const GENERIC_FONTS: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "emoji",
    "math",
    "fangsong",
    "inherit",
    "initial",
    "unset",
    "revert",
];

pub(super) fn extract_fonts(decls: &[css::CssDecl]) -> Vec<String> {
    let mut freq: HashMap<String, usize> = HashMap::new();

    for decl in decls {
        if decl.property != "font-family" && decl.property != "font" {
            continue;
        }

        let family_str = if decl.property == "font" {
            FONT_FAMILY
                .captures(&format!("font-family: {}", &decl.value))
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| decl.value.clone())
        } else {
            decl.value.clone()
        };

        for font in split_font_families(&family_str) {
            let lower = font.to_lowercase();
            if !GENERIC_FONTS.contains(&lower.as_str()) && !is_junk_font_name(&lower) {
                *freq.entry(font).or_insert(0) += 1;
            }
        }
    }

    let mut fonts: Vec<(String, usize)> = freq.into_iter().collect();
    fonts.sort_by(|a, b| b.1.cmp(&a.1));
    fonts.into_iter().map(|(name, _)| name).collect()
}

pub(crate) fn extract_font_name_from_url(url: &str) -> Option<String> {
    let filename = url.rsplit('/').next()?;
    // Use rsplit_once to strip only the last extension, preserving dots in stem
    let stem = filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(filename);
    let clean = stem
        .split('-')
        .take_while(|part| {
            let lower = part.to_lowercase();
            // Skip version tokens like v12, v3, etc.
            if lower.starts_with('v') && lower[1..].chars().all(|c| c.is_ascii_digit()) && lower.len() > 1 {
                return false;
            }
            !matches!(
                lower.as_str(),
                "regular"
                    | "bold"
                    | "italic"
                    | "light"
                    | "medium"
                    | "semibold"
                    | "variable"
                    | "subset"
                    | "latin"
                    | "cyrillic"
                    | "woff"
                    | "woff2"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");

    if clean.is_empty() || clean.len() < 2 {
        None
    } else {
        Some(clean)
    }
}

pub(crate) fn extract_google_fonts_from_url(url: &str) -> Vec<String> {
    let mut fonts = Vec::new();
    for part in url.split('&') {
        let family = if let Some(rest) = part.strip_prefix("family=") {
            rest
        } else if let Some(rest) = part.split("family=").nth(1) {
            rest
        } else {
            continue;
        };
        let name = family.split(':').next().unwrap_or(family);
        // Replace + with space first, then percent-decode %HH sequences
        let with_spaces = name.replace('+', " ");
        let clean = percent_decode(&with_spaces);
        if !clean.is_empty() {
            fonts.push(clean);
        }
    }
    fonts
}

/// Decode percent-encoded sequences (%HH) in a string.
/// Used for Google Fonts URLs that may contain %20 instead of + for spaces.
fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push((hi * 16 + lo) as u8 as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn is_junk_font_name(name: &str) -> bool {
    if name.starts_with("var(") {
        return true;
    }
    if name.len() >= 8 && name.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if name.len() < 3 {
        return true;
    }
    if name.starts_with('_') || name.starts_with("--") {
        return true;
    }
    false
}

fn split_font_families(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| {
            s.trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
