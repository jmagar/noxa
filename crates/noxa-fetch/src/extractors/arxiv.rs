use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};

use super::{ExtractorInfo, host_matches, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "arxiv",
    label: "arXiv Paper",
    description: "Extract paper metadata from arXiv pages.",
    url_patterns: &["https://arxiv.org/abs/*", "https://arxiv.org/pdf/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "arxiv.org") && (url.contains("/abs/") || url.contains("/pdf/"))
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let id = parse_id(url)
        .ok_or_else(|| FetchError::Build(format!("arxiv: cannot parse id from '{url}'")))?;
    let api_url = format!("https://export.arxiv.org/api/query?id_list={id}");
    let xml = client.get_text(&api_url).await?;
    let entry = parse_atom_entry(&xml)
        .ok_or_else(|| FetchError::BodyDecode("arxiv: no <entry> in response".into()))?;

    Ok(json!({
        "url": url,
        "id": id,
        "arxiv_id": entry.id,
        "title": entry.title.map(|title| collapse_whitespace(&title)),
        "authors": entry.authors,
        "abstract": entry.summary.map(|summary| collapse_whitespace(&summary)),
        "published": entry.published,
        "updated": entry.updated,
        "primary_category": entry.primary_category,
        "categories": entry.categories,
        "doi": entry.doi,
        "comment": entry.comment,
        "pdf_url": entry.pdf_url,
        "abs_url": entry.abs_url,
    }))
}

fn parse_id(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 || (segs[0] != "abs" && segs[0] != "pdf") {
        return None;
    }
    let stripped = segs[1].trim_end_matches(".pdf");
    let no_version = match stripped.rfind('v') {
        Some(index) if stripped[index + 1..].chars().all(|c| c.is_ascii_digit()) => {
            &stripped[..index]
        }
        _ => stripped,
    };
    Some(no_version.to_string()).filter(|value| !value.is_empty())
}

#[derive(Default)]
struct AtomEntry {
    id: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    published: Option<String>,
    updated: Option<String>,
    primary_category: Option<String>,
    categories: Vec<String>,
    authors: Vec<String>,
    doi: Option<String>,
    comment: Option<String>,
    pdf_url: Option<String>,
    abs_url: Option<String>,
}

fn parse_atom_entry(xml: &str) -> Option<AtomEntry> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut in_entry = false;
    let mut in_author = false;
    let mut current: Option<&'static str> = None;
    let mut entry = AtomEntry::default();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => match element.local_name().as_ref() {
                b"entry" => in_entry = true,
                b"id" if in_entry && !in_author => current = Some("id"),
                b"title" if in_entry => current = Some("title"),
                b"summary" if in_entry => current = Some("summary"),
                b"published" if in_entry => current = Some("published"),
                b"updated" if in_entry => current = Some("updated"),
                b"author" if in_entry => in_author = true,
                b"name" if in_author => current = Some("author"),
                b"doi" if in_entry => current = Some("doi"),
                b"comment" if in_entry => current = Some("comment"),
                _ => {}
            },
            Ok(Event::Empty(element)) if in_entry => {
                let mut term = None;
                let mut href = None;
                let mut rel = None;
                let mut content_type = None;
                for attr in element.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"term" => term = attr.unescape_value().ok().map(|v| v.to_string()),
                        b"href" => href = attr.unescape_value().ok().map(|v| v.to_string()),
                        b"rel" => rel = attr.unescape_value().ok().map(|v| v.to_string()),
                        b"type" => content_type = attr.unescape_value().ok().map(|v| v.to_string()),
                        _ => {}
                    }
                }
                if let Some(term) = term {
                    if entry.primary_category.is_none() {
                        entry.primary_category = Some(term.clone());
                    }
                    entry.categories.push(term);
                }
                if let Some(href) = href {
                    if content_type.as_deref() == Some("application/pdf") {
                        entry.pdf_url = Some(href.clone());
                    }
                    if rel.as_deref() == Some("alternate") {
                        entry.abs_url = Some(href);
                    }
                }
            }
            Ok(Event::Text(text)) => {
                let text = text.unescape().ok()?.to_string();
                match current {
                    Some("id") => entry.id = Some(text.trim().to_string()),
                    Some("title") => entry.title = append_text(entry.title.take(), &text),
                    Some("summary") => entry.summary = append_text(entry.summary.take(), &text),
                    Some("published") => entry.published = Some(text.trim().to_string()),
                    Some("updated") => entry.updated = Some(text.trim().to_string()),
                    Some("author") => entry.authors.push(text.trim().to_string()),
                    Some("doi") => entry.doi = Some(text.trim().to_string()),
                    Some("comment") => entry.comment = Some(text.trim().to_string()),
                    _ => {}
                }
            }
            Ok(Event::End(element)) => match element.local_name().as_ref() {
                b"entry" => break,
                b"author" => {
                    in_author = false;
                    current = None;
                }
                _ => current = None,
            },
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }

    in_entry.then_some(entry)
}

fn append_text(prev: Option<String>, next: &str) -> Option<String> {
    match prev {
        Some(mut value) => {
            value.push_str(next);
            Some(value)
        }
        None => Some(next.to_string()),
    }
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
