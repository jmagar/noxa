use once_cell::sync::Lazy;
use scraper::Selector;

use super::*;

macro_rules! selector {
    ($s:expr) => {{
        static SEL: Lazy<Selector> = Lazy::new(|| Selector::parse($s).unwrap());
        &*SEL
    }};
}

/// A CSS declaration with its property context and raw value.
pub(super) struct CssDecl {
    pub(super) property: String,
    pub(super) value: String,
}

/// Extract CSS custom properties that look like color values.
/// e.g., `--primary: #3b82f6;` or `--brand-bg: rgb(30, 41, 59);`
static CSS_VAR: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)--([\w-]+)\s*:\s*([^;}{]+)").unwrap());

/// Extract Tailwind arbitrary color values from class strings.
/// e.g., `bg-[#1a1a2e]`, `text-[#e94560]`, `border-[rgb(255,0,0)]`
static TW_COLOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:bg|text|border|ring|outline|shadow|accent|fill|stroke)-\[([^\]]+)\]").unwrap()
});

pub(super) fn collect_css(doc: &Html) -> Vec<CssDecl> {
    let mut decls = Vec::new();

    for el in doc.select(selector!("style")) {
        let text: String = el.text().collect();
        parse_declarations(&text, &mut decls);
        parse_css_variables(&text, &mut decls);
    }

    for el in doc.select(selector!("[style]")) {
        if let Some(style) = el.value().attr("style") {
            parse_declarations(style, &mut decls);
            parse_css_variables(style, &mut decls);
        }
    }

    for el in doc.select(selector!("[class]")) {
        if let Some(class) = el.value().attr("class") {
            parse_tailwind_colors(class, &mut decls);
        }
    }

    for el in doc.select(selector!("meta[name='theme-color']")) {
        if let Some(content) = el.value().attr("content") {
            decls.push(CssDecl {
                property: "background-color".to_string(),
                value: content.to_string(),
            });
        }
    }

    for el in doc.select(selector!("link[rel='preload'][as='font']")) {
        if let Some(href) = el.value().attr("href")
            && let Some(font_name) = fonts::extract_font_name_from_url(href)
        {
            decls.push(CssDecl {
                property: "font-family".to_string(),
                value: format!("\"{}\"", font_name),
            });
        }
    }

    for el in doc.select(selector!("link[rel='stylesheet']")) {
        if let Some(href) = el.value().attr("href")
            && (href.contains("fonts.googleapis.com") || href.contains("fonts.bunny.net"))
        {
            for font in fonts::extract_google_fonts_from_url(href) {
                decls.push(CssDecl {
                    property: "font-family".to_string(),
                    value: format!("\"{}\"", font),
                });
            }
        }
    }

    decls
}

fn parse_declarations(css_text: &str, out: &mut Vec<CssDecl>) {
    for cap in CSS_DECL.captures_iter(css_text) {
        let property = cap[1].to_ascii_lowercase();
        let value = cap[2].trim().to_string();
        out.push(CssDecl { property, value });
    }
}

fn parse_css_variables(css_text: &str, out: &mut Vec<CssDecl>) {
    for cap in CSS_VAR.captures_iter(css_text) {
        let var_name = cap[1].to_ascii_lowercase();
        let value = cap[2].trim().to_string();

        if HEX_COLOR.is_match(&value)
            || RGB_COLOR.is_match(&value)
            || RGBA_COLOR.is_match(&value)
            || HSL_COLOR.is_match(&value)
        {
            let property = if var_name.contains("background") || var_name.contains("bg") {
                "background-color"
            } else if var_name.contains("text")
                || var_name.contains("foreground")
                || var_name.contains("fg")
            {
                "color"
            } else if var_name.contains("border") || var_name.contains("accent") {
                "border-color"
            } else {
                "color"
            };
            out.push(CssDecl {
                property: property.to_string(),
                value,
            });
        }
    }
}

fn parse_tailwind_colors(class: &str, out: &mut Vec<CssDecl>) {
    for cap in TW_COLOR.captures_iter(class) {
        let value = &cap[1];
        if HEX_COLOR.is_match(value)
            || RGB_COLOR.is_match(value)
            || RGBA_COLOR.is_match(value)
            || HSL_COLOR.is_match(value)
        {
            let full = cap.get(0).unwrap().as_str();
            let property = if full.starts_with("bg-") {
                "background-color"
            } else if full.starts_with("text-") {
                "color"
            } else if full.starts_with("border-") {
                "border-color"
            } else {
                "color"
            };
            out.push(CssDecl {
                property: property.to_string(),
                value: value.to_string(),
            });
        }
    }
}
