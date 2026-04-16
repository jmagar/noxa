use crate::error::RagError;

use super::{IngestionProvenance, ParsedFile, extract_ingestion_provenance, make_text_result};

pub(crate) fn parse_json_file(
    bytes: &[u8],
    _file_url: String,
    _title: String,
) -> Result<ParsedFile, RagError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| RagError::Parse(format!("JSON parse failed: {e}")))?;
    let extraction = serde_json::from_value::<noxa_core::types::ExtractionResult>(value.clone())
        .map_err(|e| RagError::Parse(format!("JSON parse failed: {e}")))?;
    Ok(ParsedFile {
        extraction,
        provenance: extract_ingestion_provenance(&value),
    })
}

pub(crate) fn parse_markdown_file(bytes: Vec<u8>, file_url: String, title: String) -> ParsedFile {
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let word_count = content.split_whitespace().count();
    ParsedFile {
        extraction: make_text_result(
            content,
            String::new(),
            file_url,
            Some(title),
            "file",
            word_count,
        ),
        provenance: IngestionProvenance::default(),
    }
}

pub(crate) fn parse_plain_text_file(bytes: Vec<u8>, file_url: String, title: String) -> ParsedFile {
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let word_count = content.split_whitespace().count();
    ParsedFile {
        extraction: make_text_result(
            content.clone(),
            content,
            file_url,
            Some(title),
            "file",
            word_count,
        ),
        provenance: IngestionProvenance::default(),
    }
}

pub(crate) fn parse_log_file(bytes: Vec<u8>, file_url: String, title: String) -> ParsedFile {
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    let stripped = strip_ansi_escapes::strip_str(&raw);
    let word_count = stripped.split_whitespace().count();
    ParsedFile {
        extraction: make_text_result(
            stripped.clone(),
            stripped,
            file_url,
            Some(title),
            "file",
            word_count,
        ),
        provenance: IngestionProvenance::default(),
    }
}

pub(crate) async fn parse_html_file(
    bytes: Vec<u8>,
    file_url: String,
) -> Result<ParsedFile, RagError> {
    let html = String::from_utf8_lossy(&bytes).into_owned();
    let url_for_extract = file_url.clone();
    let extraction = tokio::task::spawn_blocking(
        move || -> Result<noxa_core::types::ExtractionResult, RagError> {
            let mut r = noxa_core::extract(&html, Some(&url_for_extract))
                .map_err(|e| RagError::Parse(format!("HTML extract: {e}")))?;
            r.metadata.url = Some(url_for_extract);
            r.metadata.source_type = Some("file".to_string());
            Ok(r)
        },
    )
    .await
    .map_err(|e| RagError::Parse(format!("HTML spawn_blocking: {e}")))??;

    Ok(ParsedFile {
        extraction,
        provenance: IngestionProvenance::default(),
    })
}

pub(crate) fn parse_jsonl_file(bytes: Vec<u8>, file_url: String, title: String) -> ParsedFile {
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let text = content
        .lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            ["text", "content", "body", "message", "value"]
                .iter()
                .find_map(|k| v[k].as_str().map(str::to_string))
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let word_count = text.split_whitespace().count();
    ParsedFile {
        extraction: make_text_result(
            text.clone(),
            text,
            file_url,
            Some(title),
            "file",
            word_count,
        ),
        provenance: IngestionProvenance::default(),
    }
}

pub(crate) fn parse_xml_file(bytes: Vec<u8>, file_url: String, title: String) -> ParsedFile {
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let text = extract_xml_text(&content);
    let word_count = text.split_whitespace().count();
    ParsedFile {
        extraction: make_text_result(
            text.clone(),
            text,
            file_url,
            Some(title),
            "file",
            word_count,
        ),
        provenance: IngestionProvenance::default(),
    }
}

/// Extract plain text from XML/OPML/RSS/Atom by collecting all text nodes.
pub(crate) fn extract_xml_text(xml: &str) -> String {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    let mut parts: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => {
                if let Ok(text) = e.unescape() {
                    let t = text.trim().to_string();
                    if !t.is_empty() {
                        parts.push(t);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    parts.join("\n")
}
