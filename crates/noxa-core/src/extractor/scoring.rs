use scraper::{ElementRef, Html};
use tracing::debug;

use crate::noise;

use super::{A_SELECTOR, CANDIDATE_SELECTOR, P_SELECTOR};

const STRUCTURAL_NOISE_TAGS: &[&str] = &["nav", "aside", "footer", "header"];

pub(super) fn is_inside_structural_noise(el: ElementRef<'_>) -> bool {
    let mut node = el.parent();
    while let Some(parent) = node {
        if let Some(parent_el) = ElementRef::wrap(parent) {
            let tag = parent_el.value().name();
            if STRUCTURAL_NOISE_TAGS.contains(&tag) {
                return true;
            }
            // Also check role-based structural noise
            if let Some(role) = parent_el.value().attr("role")
                && (role == "navigation" || role == "contentinfo")
            {
                return true;
            }
        }
        node = parent.parent();
    }
    false
}

/// Score each candidate node and return the best one.
pub(super) fn find_best_node(doc: &Html) -> Option<ElementRef<'_>> {
    let mut best: Option<(ElementRef<'_>, f64)> = None;

    for candidate in doc.select(&CANDIDATE_SELECTOR) {
        if noise::is_noise(candidate) || noise::is_noise_descendant(candidate) {
            continue;
        }

        let score = score_node(candidate);

        if score > 0.0 && best.as_ref().is_none_or(|(_, s)| score > *s) {
            best = Some((candidate, score));
        }
    }

    best.map(|(el, score)| {
        debug!(score, tag = el.value().name(), "best content candidate");
        el
    })
}

fn score_node(el: ElementRef<'_>) -> f64 {
    let text = el.text().collect::<String>();
    let text_len = text.len() as f64;

    // Very short nodes aren't content
    if text_len < 50.0 {
        return 0.0;
    }

    let mut score = 0.0;

    // Base score: text length (log scale to avoid huge nodes dominating purely by size)
    score += text_len.ln();

    // Bonus for <article> or <main> — these are strong semantic signals
    let tag = el.value().name();
    match tag {
        "article" => score += 50.0,
        "main" => score += 50.0,
        _ => {}
    }

    // Bonus for role="main"
    if el.value().attr("role") == Some("main") {
        score += 50.0;
    }

    // Bonus for common content class/id patterns
    if let Some(class) = el.value().attr("class") {
        let cl = class.to_lowercase();
        if cl.contains("content")
            || cl.contains("article")
            || cl.contains("post")
            || cl.contains("entry")
        {
            score += 25.0;
        }
    }
    if let Some(id) = el.value().attr("id") {
        let id = id.to_lowercase();
        if id.contains("content")
            || id.contains("article")
            || id.contains("post")
            || id.contains("main")
        {
            score += 25.0;
        }
    }

    // Paragraph density: count <p> children — real content has paragraphs
    let p_count = el.select(&P_SELECTOR).count() as f64;
    score += p_count * 3.0;

    // Link density penalty: nodes that are mostly links (nav, footer) score low.
    // link_text_len / total_text_len — lower is better for content.
    let link_text_len: f64 = el
        .select(&A_SELECTOR)
        .map(|a| a.text().collect::<String>().len() as f64)
        .sum();

    // Semantic nodes (article, main, role=main) get milder link density penalties.
    // Documentation pages often have high link density from TOCs inside the main
    // content container — these are expected, not spam.
    let is_semantic = matches!(tag, "article" | "main") || el.value().attr("role") == Some("main");

    if text_len > 0.0 {
        let link_density = link_text_len / text_len;
        if is_semantic {
            // Semantic nodes: only penalize extreme link density
            if link_density > 0.7 {
                score *= 0.3;
            } else if link_density > 0.5 {
                score *= 0.5;
            }
        } else {
            // Generic divs: heavy penalty for link-dense content
            if link_density > 0.5 {
                score *= 0.1;
            } else if link_density > 0.3 {
                score *= 0.5;
            }
        }
    }

    score
}

/// Count words in text (for word_count metadata).
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}
