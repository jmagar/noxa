/// Brand identity extraction from HTML.
/// Pure DOM/CSS analysis: extracts colors, fonts, logo, and favicon
/// from style blocks, inline styles, and semantic HTML patterns.
/// No network calls, no LLM — WASM-safe.
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::Html;
use serde::Serialize;
use url::Url;

mod colors;
mod css;
mod fonts;
mod logos;
#[cfg(test)]
mod tests;

use colors::extract_colors;
use css::collect_css;
use fonts::extract_fonts;
#[cfg(test)]
pub(crate) use fonts::{extract_font_name_from_url, extract_google_fonts_from_url};
use logos::{extract_brand_name, find_all_logos, find_favicon, find_logo, find_og_image};

#[derive(Debug, Clone, Serialize)]
pub struct BrandColor {
    pub hex: String,
    pub usage: ColorUsage,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum ColorUsage {
    Primary,
    Secondary,
    Background,
    Text,
    Accent,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogoVariant {
    pub url: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrandIdentity {
    pub name: Option<String>,
    pub colors: Vec<BrandColor>,
    pub fonts: Vec<String>,
    pub logos: Vec<LogoVariant>,
    pub logo_url: Option<String>,
    pub favicon_url: Option<String>,
    pub og_image: Option<String>,
}

static CSS_DECL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)([\w-]+)\s*:\s*([^;}{]+)").unwrap());
static HEX_COLOR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"#([0-9a-fA-F]{3})\b|#([0-9a-fA-F]{6})\b").unwrap());
static RGB_COLOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)rgb\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*\)").unwrap()
});
static RGBA_COLOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)rgba\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*[\d.]+\s*\)").unwrap()
});
static HSL_COLOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)hsla?\(\s*(\d{1,3})\s*,\s*(\d{1,3})%\s*,\s*(\d{1,3})%\s*(?:,\s*[\d.]+\s*)?\)")
        .unwrap()
});
static FONT_FAMILY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)font-family\s*:\s*([^;}{]+)").unwrap());

pub fn extract_brand(html: &str, url: Option<&str>) -> BrandIdentity {
    let doc = Html::parse_document(html);
    let base_url = url.and_then(|u| Url::parse(u).ok());

    let css_sources = collect_css(&doc);

    BrandIdentity {
        name: extract_brand_name(&doc),
        colors: extract_colors(&css_sources),
        fonts: extract_fonts(&css_sources),
        logos: find_all_logos(&doc, base_url.as_ref()),
        logo_url: find_logo(&doc, base_url.as_ref()),
        favicon_url: find_favicon(&doc, base_url.as_ref()),
        og_image: find_og_image(&doc, base_url.as_ref()),
    }
}
