use std::collections::HashSet;

use ego_tree::NodeId;
use scraper::{ElementRef, Html, Selector};
use tracing::warn;

use super::MAX_SELECTORS;

pub(super) fn build_exclude_set(doc: &Html, selectors: &[String]) -> HashSet<NodeId> {
    if selectors.len() > MAX_SELECTORS {
        warn!(
            "too many CSS selectors ({}, max {}), truncating",
            selectors.len(),
            MAX_SELECTORS
        );
    }

    let mut exclude = HashSet::new();
    for selector_str in selectors.iter().take(MAX_SELECTORS) {
        let Ok(selector) = Selector::parse(selector_str) else {
            warn!(
                selector = selector_str.as_str(),
                "invalid CSS selector, skipping"
            );
            continue;
        };
        for el in doc.select(&selector) {
            // Add the element itself and all descendants
            exclude.insert(el.id());
            for descendant in el.descendants() {
                if let Some(child_el) = ElementRef::wrap(descendant) {
                    exclude.insert(child_el.id());
                }
            }
        }
    }
    exclude
}

/// Parse CSS selector strings into Selectors, skipping invalid ones.
pub(super) fn parse_selectors(strings: &[String]) -> Vec<Selector> {
    strings
        .iter()
        .filter_map(|s| {
            Selector::parse(s)
                .map_err(|_| warn!(selector = s.as_str(), "invalid CSS selector, skipping"))
                .ok()
        })
        .collect()
}
