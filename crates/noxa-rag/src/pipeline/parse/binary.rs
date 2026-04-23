use std::io::Read;

use crate::error::RagError;

use super::{FormatProvenance, IngestionProvenance, ParsedFile, make_text_result};

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
    let word_count = text.split_whitespace().count();
    Ok(ParsedFile {
        extraction: make_text_result(text.clone(), text, url, Some(title), "notebook", word_count),
        provenance: IngestionProvenance::default(),
    })
}

pub(crate) fn parse_pdf_file(
    bytes: &[u8],
    url: String,
    title: String,
) -> Result<ParsedFile, RagError> {
    let result = noxa_pdf::extract_pdf(bytes, noxa_pdf::PdfMode::Auto)
        .map_err(|e| RagError::Parse(format!("PDF extract: {e}")))?;
    let text = noxa_pdf::to_markdown(&result);
    let word_count = text.split_whitespace().count();
    Ok(ParsedFile {
        extraction: make_text_result(text.clone(), text, url, Some(title), "file", word_count),
        provenance: IngestionProvenance::default(),
    })
}

pub(crate) fn parse_office_zip_file(
    bytes: &[u8],
    url: String,
    title: String,
    ext: &str,
) -> Result<ParsedFile, RagError> {
    const MAX_ENTRY_SIZE: u64 = 100 * 1024 * 1024;
    const MAX_ENTRIES: usize = 1_000;
    const MAX_TOTAL_UNCOMPRESSED_SIZE: u64 = 250 * 1024 * 1024;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| RagError::Parse(format!("{ext} ZIP open: {e}")))?;

    if archive.len() > MAX_ENTRIES {
        return Err(RagError::Parse(format!(
            "{ext}: archive has {} entries (max {MAX_ENTRIES}) — possible zip bomb",
            archive.len()
        )));
    }

    let mut total_uncompressed_size = 0u64;

    if ext == "docx" {
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                total_uncompressed_size = total_uncompressed_size.saturating_add(entry.size());
                if total_uncompressed_size > MAX_TOTAL_UNCOMPRESSED_SIZE {
                    return Err(RagError::Parse(
                        "docx: archive expands to more than 250 MiB — possible zip bomb"
                            .to_string(),
                    ));
                }
                if entry.size() > MAX_ENTRY_SIZE {
                    return Err(RagError::Parse(format!(
                        "docx: entry '{}' decompresses to {} bytes (max 100 MiB) — possible zip bomb",
                        entry.name(),
                        entry.size()
                    )));
                }
            }
        }
        let result =
            noxa_fetch::document::extract_document(bytes, noxa_fetch::document::DocType::Docx)
                .map_err(|e| RagError::Parse(format!("DOCX extract: {e}")))?;
        let mut r = result;
        r.metadata.url = Some(url);
        r.metadata.source_type = Some("file".to_string());
        if r.metadata.title.is_none() {
            r.metadata.title = Some(title);
        }
        return Ok(ParsedFile {
            extraction: r,
            provenance: IngestionProvenance::default(),
        });
    }

    let target_prefix = match ext {
        "odt" => "content",
        "pptx" => "ppt/slides/slide",
        _ => "",
    };

    let mut text_parts: Vec<String> = Vec::new();
    let mut slide_count = 0u32;
    let mut has_notes = false;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| RagError::Parse(format!("{ext} entry {i}: {e}")))?;
        total_uncompressed_size = total_uncompressed_size.saturating_add(entry.size());
        if total_uncompressed_size > MAX_TOTAL_UNCOMPRESSED_SIZE {
            return Err(RagError::Parse(format!(
                "{ext}: archive expands to more than 250 MiB — possible zip bomb"
            )));
        }

        if entry.size() > MAX_ENTRY_SIZE {
            return Err(RagError::Parse(format!(
                "{ext}: entry '{}' decompresses to {} bytes (max 100 MiB) — possible zip bomb",
                entry.name(),
                entry.size()
            )));
        }

        let name = entry.name().to_string();
        if !name.ends_with(".xml") {
            continue;
        }
        if ext == "pptx" && name.contains("ppt/slides/slide") {
            slide_count += 1;
        }
        if ext == "pptx" && name.contains("ppt/notesSlides/notesSlide") {
            has_notes = true;
        }
        if !target_prefix.is_empty() && !name.contains(target_prefix) {
            continue;
        }

        let mut xml_buf = String::new();
        entry
            .read_to_string(&mut xml_buf)
            .map_err(|e| RagError::Parse(format!("{ext} read '{name}': {e}")))?;

        let fragment = super::extract_xml_text(&xml_buf).unwrap_or_else(|_| xml_buf.clone());
        if !fragment.trim().is_empty() {
            text_parts.push(fragment);
        }
    }

    let text = text_parts.join("\n\n");
    let word_count = text.split_whitespace().count();
    let extraction = make_text_result(text.clone(), text, url, Some(title), "file", word_count);
    let provenance = if ext == "pptx" {
        IngestionProvenance {
            external_id: None,
            platform_url: None,
            format: FormatProvenance::Presentation {
                slide_count: Some(slide_count),
                has_notes: Some(has_notes),
            },
        }
    } else {
        IngestionProvenance::default()
    };
    Ok(ParsedFile {
        extraction,
        provenance,
    })
}
