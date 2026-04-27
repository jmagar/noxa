use crate::error::RagError;

use super::{IngestionProvenance, ParsedFile, extract_ingestion_provenance, make_text_result};

pub(crate) fn parse_ipynb_file(
    bytes: &[u8],
    url: String,
    title: String,
) -> Result<ParsedFile, RagError> {
    let v: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| RagError::Parse(format!("ipynb JSON parse: {e}")))?;

    let cells = v["cells"]
        .as_array()
        .ok_or_else(|| RagError::Parse("ipynb: missing 'cells' array".to_string()))?;

    let mut parts: Vec<String> = Vec::new();
    for cell in cells {
        let cell_type = cell["cell_type"].as_str().unwrap_or("");
        if !matches!(cell_type, "markdown" | "code") {
            continue;
        }
        let source = match &cell["source"] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(lines) => lines.iter().filter_map(|l| l.as_str()).collect(),
            _ => continue,
        };
        let trimmed = source.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }

    let text = parts.join("\n\n");
    let word_count = crate::chunker::word_count(&text);
    Ok(ParsedFile {
        extraction: make_text_result(text.clone(), text, url, Some(title), "notebook", word_count),
        provenance: IngestionProvenance::default(),
    })
}

pub(crate) fn parse_json_file(
    bytes: &[u8],
    file_url: String,
    title: String,
) -> Result<ParsedFile, RagError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| RagError::Parse(format!("JSON parse failed: {e}")))?;
    let extraction = serde_json::from_slice::<noxa_core::types::ExtractionResult>(bytes)
        .map_err(|e| RagError::Parse(format!("JSON parse failed: {e}")))?;
    let mut extraction = extraction;
    if extraction.metadata.url.is_none() {
        extraction.metadata.url = Some(file_url);
    }
    if extraction.metadata.title.is_none() && !title.is_empty() {
        extraction.metadata.title = Some(title);
    }
    Ok(ParsedFile {
        extraction,
        provenance: extract_ingestion_provenance(&value),
    })
}

pub(crate) fn parse_markdown_file(bytes: Vec<u8>, file_url: String, title: String) -> ParsedFile {
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let word_count = crate::chunker::word_count(&content);
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
    let word_count = crate::chunker::word_count(&content);
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
    let word_count = crate::chunker::word_count(&stripped);
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

pub(crate) fn parse_html_file(
    bytes: Vec<u8>,
    file_url: String,
) -> Result<ParsedFile, RagError> {
    let html = String::from_utf8_lossy(&bytes).into_owned();
    let mut extraction = noxa_core::extract(&html, Some(&file_url))
        .map_err(|e| RagError::Parse(format!("HTML extract: {e}")))?;
    extraction.metadata.url = Some(file_url);
    extraction.metadata.source_type = Some("file".to_string());
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
    let word_count = crate::chunker::word_count(&text);
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

/// Scan the first 8 KiB of bytes for DOCTYPE/ENTITY declarations that could
/// trigger exponential entity expansion (the "billion laughs" attack). Returns
/// `true` if such declarations are found.
///
/// quick-xml 0.37.x does NOT apply DTD expansion limits, so this pre-scan is
/// the primary — and not merely defensive — guard against out-of-memory
/// attacks. Later versions of quick-xml may add limits, but we keep the scan
/// regardless (defense-in-depth).
pub(super) fn contains_xml_entity_expansion_risk(bytes: &[u8]) -> bool {
    let header = &bytes[..bytes.len().min(8192)];
    // Scan raw bytes so non-UTF-8 sequences cannot silently suppress the guard.
    // `from_utf8().unwrap_or("")` would return "" on any non-UTF-8 byte in the
    // window, letting a <!DOCTYPE immediately following binary noise slip through.
    header.windows(9).any(|w| w == b"<!DOCTYPE")
        || header.windows(8).any(|w| w == b"<!ENTITY")
}

pub(crate) fn parse_xml_file(
    bytes: Vec<u8>,
    file_url: String,
    title: String,
) -> Result<ParsedFile, RagError> {
    if contains_xml_entity_expansion_risk(&bytes) {
        return Err(RagError::Parse(
            "XML entity expansion risk detected: file contains DOCTYPE/ENTITY declarations"
                .to_string(),
        ));
    }
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let text = extract_xml_text(&content).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "xml text extraction failed; falling back to raw text");
        content.clone()
    });
    let word_count = crate::chunker::word_count(&text);
    Ok(ParsedFile {
        extraction: make_text_result(
            text.clone(),
            text,
            file_url,
            Some(title),
            "file",
            word_count,
        ),
        provenance: IngestionProvenance::default(),
    })
}

/// Extract plain text from XML/OPML/RSS/Atom by collecting all text and CDATA nodes.
pub(crate) fn extract_xml_text(xml: &str) -> Result<String, RagError> {
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
            Ok(Event::CData(e)) => {
                let text = String::from_utf8_lossy(e.as_ref());
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(RagError::Parse(format!("XML parse failed: {e}"))),
            _ => {}
        }
    }

    Ok(parts.join("\n"))
}
