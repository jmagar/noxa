/// Change tracking between two extraction snapshots.
/// Pure computation -- no I/O, WASM-safe.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use similar::TextDiff;

use crate::types::{ExtractionResult, Link};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeStatus {
    Same,
    Changed,
    New,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataChange {
    pub field: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentDiff {
    pub status: ChangeStatus,
    pub text_diff: Option<String>,
    pub metadata_changes: Vec<MetadataChange>,
    pub links_added: Vec<Link>,
    pub links_removed: Vec<Link>,
    pub word_count_delta: i64,
}

/// Compare two extraction results and produce a diff.
/// `old` is the previous snapshot, `new_result` is the current extraction.
pub fn diff(old: &ExtractionResult, new_result: &ExtractionResult) -> ContentDiff {
    let text_diff = compute_text_diff(&old.content.markdown, &new_result.content.markdown);
    let metadata_changes = compute_metadata_changes(&old.metadata, &new_result.metadata);
    let (links_added, links_removed) =
        compute_link_changes(&old.content.links, &new_result.content.links);
    let word_count_delta = new_result.metadata.word_count as i64 - old.metadata.word_count as i64;

    let status = if text_diff.is_none()
        && metadata_changes.is_empty()
        && links_added.is_empty()
        && links_removed.is_empty()
        && word_count_delta == 0
    {
        ChangeStatus::Same
    } else {
        ChangeStatus::Changed
    };

    ContentDiff {
        status,
        text_diff,
        metadata_changes,
        links_added,
        links_removed,
        word_count_delta,
    }
}

fn compute_text_diff(old: &str, new: &str) -> Option<String> {
    if old == new {
        return None;
    }

    let diff = TextDiff::from_lines(old, new);
    let unified = diff
        .unified_diff()
        .context_radius(3)
        .header("old", "new")
        .to_string();

    if unified.is_empty() {
        None
    } else {
        Some(unified)
    }
}

/// Compare each metadata field, returning only those that changed.
fn compute_metadata_changes(
    old: &crate::types::Metadata,
    new: &crate::types::Metadata,
) -> Vec<MetadataChange> {
    let mut changes = Vec::new();

    push_metadata_change(&mut changes, "title", &old.title, &new.title);
    push_metadata_change(
        &mut changes,
        "description",
        &old.description,
        &new.description,
    );
    push_metadata_change(&mut changes, "author", &old.author, &new.author);
    push_metadata_change(
        &mut changes,
        "published_date",
        &old.published_date,
        &new.published_date,
    );
    push_metadata_change(&mut changes, "language", &old.language, &new.language);
    push_metadata_change(&mut changes, "url", &old.url, &new.url);
    push_metadata_change(&mut changes, "site_name", &old.site_name, &new.site_name);
    push_metadata_change(&mut changes, "image", &old.image, &new.image);
    push_metadata_change(&mut changes, "favicon", &old.favicon, &new.favicon);
    push_metadata_change(&mut changes, "word_count", &old.word_count, &new.word_count);
    push_metadata_change(
        &mut changes,
        "content_hash",
        &old.content_hash,
        &new.content_hash,
    );
    push_metadata_change(
        &mut changes,
        "source_type",
        &old.source_type,
        &new.source_type,
    );
    push_metadata_change(&mut changes, "file_path", &old.file_path, &new.file_path);
    push_metadata_change(
        &mut changes,
        "last_modified",
        &old.last_modified,
        &new.last_modified,
    );
    push_metadata_change(
        &mut changes,
        "is_truncated",
        &old.is_truncated,
        &new.is_truncated,
    );
    push_metadata_change(
        &mut changes,
        "technologies",
        &old.technologies,
        &new.technologies,
    );
    push_metadata_change(&mut changes, "seed_url", &old.seed_url, &new.seed_url);
    push_metadata_change(
        &mut changes,
        "crawl_depth",
        &old.crawl_depth,
        &new.crawl_depth,
    );
    push_metadata_change(
        &mut changes,
        "search_query",
        &old.search_query,
        &new.search_query,
    );
    push_metadata_change(&mut changes, "fetched_at", &old.fetched_at, &new.fetched_at);

    changes
}

fn push_metadata_change<T>(changes: &mut Vec<MetadataChange>, field: &str, old: &T, new: &T)
where
    T: serde::Serialize + PartialEq,
{
    if old != new {
        changes.push(MetadataChange {
            field: field.to_string(),
            old: serialize_metadata_value(old),
            new: serialize_metadata_value(new),
        });
    }
}

fn serialize_metadata_value<T: serde::Serialize>(value: &T) -> Option<String> {
    match serde_json::to_value(value).unwrap_or(serde_json::Value::Null) {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text),
        other => Some(other.to_string()),
    }
}

/// Links added/removed, compared by href (ignoring text differences).
fn compute_link_changes(old: &[Link], new: &[Link]) -> (Vec<Link>, Vec<Link>) {
    let old_hrefs: HashSet<&str> = old.iter().map(|l| l.href.as_str()).collect();
    let new_hrefs: HashSet<&str> = new.iter().map(|l| l.href.as_str()).collect();

    let added: Vec<Link> = new
        .iter()
        .filter(|l| !old_hrefs.contains(l.href.as_str()))
        .cloned()
        .collect();

    let removed: Vec<Link> = old
        .iter()
        .filter(|l| !new_hrefs.contains(l.href.as_str()))
        .cloned()
        .collect();

    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainType;
    use crate::types::{Content, DomainData, Metadata};

    /// Build a minimal ExtractionResult for test comparisons.
    fn make_result(markdown: &str, title: Option<&str>, links: Vec<Link>) -> ExtractionResult {
        let word_count = markdown.split_whitespace().count();
        ExtractionResult {
            metadata: Metadata {
                title: title.map(String::from),
                description: None,
                author: None,
                published_date: None,
                language: None,
                url: None,
                site_name: None,
                image: None,
                favicon: None,
                word_count,
                content_hash: None,
                source_type: None,
                file_path: None,
                last_modified: None,
                is_truncated: None,
                technologies: Vec::new(),
                seed_url: None,
                crawl_depth: None,
                search_query: None,
                fetched_at: None,
            },
            content: Content {
                markdown: markdown.to_string(),
                plain_text: markdown.to_string(),
                links,
                images: vec![],
                code_blocks: vec![],
                raw_html: None,
            },
            domain_data: Some(DomainData {
                domain_type: DomainType::Generic,
            }),
            structured_data: vec![],
        }
    }

    fn link(href: &str, text: &str) -> Link {
        Link {
            href: href.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn test_identical_content() {
        let a = make_result("# Hello\n\nSome content here.", Some("Hello"), vec![]);
        let b = make_result("# Hello\n\nSome content here.", Some("Hello"), vec![]);

        let result = diff(&a, &b);

        assert_eq!(result.status, ChangeStatus::Same);
        assert!(result.text_diff.is_none());
        assert!(result.metadata_changes.is_empty());
        assert!(result.links_added.is_empty());
        assert!(result.links_removed.is_empty());
        assert_eq!(result.word_count_delta, 0);
    }

    #[test]
    fn test_title_change() {
        let a = make_result("# Hello\n\nContent.", Some("Old Title"), vec![]);
        let b = make_result("# Hello\n\nContent.", Some("New Title"), vec![]);

        let result = diff(&a, &b);

        assert_eq!(result.status, ChangeStatus::Changed);
        assert!(result.text_diff.is_none(), "text is identical");
        assert_eq!(result.metadata_changes.len(), 1);
        assert_eq!(result.metadata_changes[0].field, "title");
        assert_eq!(result.metadata_changes[0].old.as_deref(), Some("Old Title"));
        assert_eq!(result.metadata_changes[0].new.as_deref(), Some("New Title"));
    }

    #[test]
    fn test_content_change() {
        let a = make_result("# Hello\n\nOld paragraph.", Some("Title"), vec![]);
        let b = make_result("# Hello\n\nNew paragraph.", Some("Title"), vec![]);

        let result = diff(&a, &b);

        assert_eq!(result.status, ChangeStatus::Changed);
        assert!(result.text_diff.is_some());
        let diff_text = result.text_diff.unwrap();
        assert!(diff_text.contains('-'), "should have removal markers");
        assert!(diff_text.contains('+'), "should have addition markers");
    }

    #[test]
    fn test_link_added() {
        let a = make_result("Content.", None, vec![]);
        let b = make_result(
            "Content.",
            None,
            vec![link("https://example.com", "Example")],
        );

        let result = diff(&a, &b);

        assert_eq!(result.links_added.len(), 1);
        assert_eq!(result.links_added[0].href, "https://example.com");
        assert!(result.links_removed.is_empty());
    }

    #[test]
    fn test_link_removed() {
        let a = make_result(
            "Content.",
            None,
            vec![link("https://example.com", "Example")],
        );
        let b = make_result("Content.", None, vec![]);

        let result = diff(&a, &b);

        assert!(result.links_added.is_empty());
        assert_eq!(result.links_removed.len(), 1);
        assert_eq!(result.links_removed[0].href, "https://example.com");
        assert_eq!(result.status, ChangeStatus::Changed);
    }

    #[test]
    fn test_links_added_and_removed() {
        let a = make_result(
            "Content.",
            None,
            vec![
                link("https://old.com", "Old"),
                link("https://stable.com", "Stable"),
            ],
        );
        let b = make_result(
            "Content.",
            None,
            vec![
                link("https://stable.com", "Stable"),
                link("https://new.com", "New"),
            ],
        );

        let result = diff(&a, &b);

        assert_eq!(result.links_added.len(), 1);
        assert_eq!(result.links_added[0].href, "https://new.com");
        assert_eq!(result.links_removed.len(), 1);
        assert_eq!(result.links_removed[0].href, "https://old.com");
        assert_eq!(result.status, ChangeStatus::Changed);
    }

    #[test]
    fn test_link_only_change_sets_status_changed() {
        let a = make_result("Content.", None, vec![]);
        let b = make_result(
            "Content.",
            None,
            vec![link("https://example.com/spec_(draft)", "Spec")],
        );

        let result = diff(&a, &b);

        assert_eq!(result.status, ChangeStatus::Changed);
        assert!(result.text_diff.is_none());
        assert!(result.metadata_changes.is_empty());
        assert_eq!(result.links_added.len(), 1);
        assert!(result.links_removed.is_empty());
    }

    #[test]
    fn test_diff_includes_rag_metadata_fields() {
        let mut a = make_result("Content.", Some("Title"), vec![]);
        a.metadata.word_count = 10;
        a.metadata.content_hash = Some("hash-a".into());
        a.metadata.source_type = Some("web".into());
        a.metadata.file_path = Some("/tmp/old.html".into());
        a.metadata.last_modified = Some("2025-01-01T00:00:00Z".into());
        a.metadata.is_truncated = Some(false);
        a.metadata.technologies = vec!["React".into()];
        a.metadata.seed_url = Some("https://example.com".into());
        a.metadata.crawl_depth = Some(1);
        a.metadata.search_query = Some("old query".into());
        a.metadata.fetched_at = Some("2025-01-01T01:00:00Z".into());

        let mut b = make_result("Content.", Some("Title"), vec![]);
        b.metadata.word_count = 25;
        b.metadata.content_hash = Some("hash-b".into());
        b.metadata.source_type = Some("file".into());
        b.metadata.file_path = Some("/tmp/new.html".into());
        b.metadata.last_modified = Some("2025-01-02T00:00:00Z".into());
        b.metadata.is_truncated = Some(true);
        b.metadata.technologies = vec!["React".into(), "TypeScript".into()];
        b.metadata.seed_url = Some("https://mirror.example.com".into());
        b.metadata.crawl_depth = Some(2);
        b.metadata.search_query = Some("new query".into());
        b.metadata.fetched_at = Some("2025-01-02T01:00:00Z".into());

        let result = diff(&a, &b);
        let changed_fields: HashSet<&str> = result
            .metadata_changes
            .iter()
            .map(|change| change.field.as_str())
            .collect();

        for field in [
            "word_count",
            "content_hash",
            "source_type",
            "file_path",
            "last_modified",
            "is_truncated",
            "technologies",
            "seed_url",
            "crawl_depth",
            "search_query",
            "fetched_at",
        ] {
            assert!(
                changed_fields.contains(field),
                "missing metadata diff for {field}: {changed_fields:?}"
            );
        }

        assert_eq!(result.status, ChangeStatus::Changed);
    }

    #[test]
    fn test_word_count_delta() {
        let a = make_result("one two three", None, vec![]);
        let b = make_result("one two three four five", None, vec![]);

        let result = diff(&a, &b);

        assert_eq!(result.word_count_delta, 2);

        // Negative delta
        let result_rev = diff(&b, &a);
        assert_eq!(result_rev.word_count_delta, -2);
    }

    #[test]
    fn test_unified_diff_format() {
        let a = make_result("line one\nline two\nline three\n", None, vec![]);
        let b = make_result("line one\nline changed\nline three\n", None, vec![]);

        let result = diff(&a, &b);

        assert!(result.text_diff.is_some());
        let diff_text = result.text_diff.unwrap();
        assert!(diff_text.contains("--- old"), "should have old header");
        assert!(diff_text.contains("+++ new"), "should have new header");
        assert!(diff_text.contains("-line two"), "should show removed line");
        assert!(
            diff_text.contains("+line changed"),
            "should show added line"
        );
    }

    #[test]
    fn test_empty_content() {
        let a = make_result("", None, vec![]);
        let b = make_result("", None, vec![]);

        let result = diff(&a, &b);

        assert_eq!(result.status, ChangeStatus::Same);
        assert!(result.text_diff.is_none());
        assert_eq!(result.word_count_delta, 0);
    }

    #[test]
    fn test_link_text_change_ignored() {
        // Same href, different text -- should not appear in added/removed
        let a = make_result(
            "Content.",
            None,
            vec![link("https://example.com", "Old Text")],
        );
        let b = make_result(
            "Content.",
            None,
            vec![link("https://example.com", "New Text")],
        );

        let result = diff(&a, &b);

        assert!(result.links_added.is_empty());
        assert!(result.links_removed.is_empty());
    }
}
