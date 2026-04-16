use once_cell::sync::Lazy;
use scraper::Selector;

use super::*;

macro_rules! selector {
    ($s:expr) => {{
        static SEL: Lazy<Selector> = Lazy::new(|| Selector::parse($s).unwrap());
        &*SEL
    }};
}

pub(super) fn find_logo(doc: &Html, base_url: Option<&Url>) -> Option<String> {
    for el in doc.select(selector!("img")) {
        let class = el.value().attr("class").unwrap_or("");
        let id = el.value().attr("id").unwrap_or("");
        if (contains_ci(class, "logo") || contains_ci(id, "logo"))
            && let Some(src) = el.value().attr("src")
        {
            return Some(resolve_url(src, base_url));
        }
    }

    for el in doc.select(selector!("img")) {
        let alt = el.value().attr("alt").unwrap_or("");
        if contains_ci(alt, "logo")
            && let Some(src) = el.value().attr("src")
        {
            return Some(resolve_url(src, base_url));
        }
    }

    for el in doc.select(selector!("a[href='/'] img, a[href] img")) {
        if let Some(parent) = el.parent().and_then(|p| p.value().as_element()) {
            let href = parent.attr("href").unwrap_or("");
            if (href == "/" || href.ends_with(".com") || href.ends_with(".com/"))
                && let Some(src) = el.value().attr("src")
            {
                return Some(resolve_url(src, base_url));
            }
        }
    }

    None
}

pub(super) fn find_favicon(doc: &Html, base_url: Option<&Url>) -> Option<String> {
    doc.select(selector!("link[rel]"))
        .find(|el| {
            el.value()
                .attr("rel")
                .is_some_and(|r| r.to_lowercase().contains("icon"))
        })
        .and_then(|el| el.value().attr("href"))
        .map(|href| resolve_url(href, base_url))
}

pub(super) fn extract_brand_name(doc: &Html) -> Option<String> {
    for el in doc.select(selector!("meta[property='og:site_name']")) {
        if let Some(content) = el.value().attr("content") {
            let name = content.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    for el in doc.select(selector!("meta[name='application-name']")) {
        if let Some(content) = el.value().attr("content") {
            let name = content.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    for el in doc.select(selector!("title")) {
        let title: String = el.text().collect();
        let title = title.trim();
        if !title.is_empty() {
            return Some(clean_title_to_brand(title));
        }
    }

    None
}

fn clean_title_to_brand(title: &str) -> String {
    for sep in [" | ", " - ", " — ", " · ", " :: ", " // "] {
        if let Some(pos) = title.find(sep) {
            let left = title[..pos].trim();
            let right = title[pos + sep.len()..].trim();
            let page_suffixes = ["home", "homepage", "official", "welcome"];
            if page_suffixes
                .iter()
                .any(|s| left.to_lowercase().ends_with(s))
            {
                return left.to_string();
            }
            if page_suffixes
                .iter()
                .any(|s| right.to_lowercase().ends_with(s))
            {
                return left.to_string();
            }
            if right.len() < left.len() && right.len() >= 2 {
                return right.to_string();
            }
            return left.to_string();
        }
    }
    title.to_string()
}

pub(super) fn find_all_logos(doc: &Html, base_url: Option<&Url>) -> Vec<LogoVariant> {
    let mut logos = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut add = |url: String, kind: &str| {
        if !url.is_empty() && seen_urls.insert(url.clone()) {
            logos.push(LogoVariant {
                url,
                kind: kind.to_string(),
            });
        }
    };

    for el in doc.select(selector!("link[rel]")) {
        let rel = el.value().attr("rel").unwrap_or("").to_lowercase();
        if let Some(href) = el.value().attr("href")
            && rel.contains("icon")
            && !rel.contains("apple")
        {
            add(resolve_url(href, base_url), "favicon");
        }
    }

    for el in doc.select(selector!("link[rel='apple-touch-icon']")) {
        if let Some(href) = el.value().attr("href") {
            add(resolve_url(href, base_url), "apple-touch-icon");
        }
    }
    for el in doc.select(selector!("link[rel='apple-touch-icon-precomposed']")) {
        if let Some(href) = el.value().attr("href") {
            add(resolve_url(href, base_url), "apple-touch-icon");
        }
    }

    for el in doc.select(selector!("img")) {
        let class = el.value().attr("class").unwrap_or("");
        let id = el.value().attr("id").unwrap_or("");
        let alt = el.value().attr("alt").unwrap_or("");
        if (contains_ci(class, "logo") || contains_ci(id, "logo") || contains_ci(alt, "logo"))
            && let Some(src) = el.value().attr("src")
        {
            add(resolve_url(src, base_url), "logo");
        }
    }

    for el in doc.select(selector!(
        "header svg[viewBox], nav svg[viewBox], a svg[viewBox]"
    )) {
        if logos.iter().all(|l| l.kind != "svg")
            && let Some(parent) = el.parent().and_then(|p| p.value().as_element())
            && parent
                .attr("href")
                .is_some_and(|h| h == "/" || h.ends_with(".com") || h.ends_with(".com/"))
        {
            logos.push(LogoVariant {
                url: "(inline-svg)".to_string(),
                kind: "svg".to_string(),
            });
        }
    }

    logos
}

pub(super) fn find_og_image(doc: &Html, base_url: Option<&Url>) -> Option<String> {
    for el in doc.select(selector!("meta[property='og:image']")) {
        if let Some(content) = el.value().attr("content") {
            let url = content.trim();
            if !url.is_empty() {
                return Some(resolve_url(url, base_url));
            }
        }
    }
    for el in doc.select(selector!("meta[name='twitter:image']")) {
        if let Some(content) = el.value().attr("content") {
            let url = content.trim();
            if !url.is_empty() {
                return Some(resolve_url(url, base_url));
            }
        }
    }
    None
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn resolve_url(src: &str, base_url: Option<&Url>) -> String {
    match base_url {
        Some(base) => base
            .join(src)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| src.to_string()),
        None => src.to_string(),
    }
}
