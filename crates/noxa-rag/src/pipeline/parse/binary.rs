use std::io::Read;

use crate::error::RagError;

use super::{FormatProvenance, IngestionProvenance, ParsedFile, make_text_result};

pub(crate) fn parse_pdf_file(
    bytes: &[u8],
    url: String,
    title: String,
) -> Result<ParsedFile, RagError> {
    let result = noxa_pdf::extract_pdf(bytes, noxa_pdf::PdfMode::Auto)
        .map_err(|e| RagError::Parse(format!("PDF extract: {e}")))?;
    let text = noxa_pdf::to_markdown(&result);
    let word_count = crate::chunker::word_count(&text);
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
    // Hard cap on total decompressed bytes, measured from the actual decompression
    // stream (NOT the attacker-controlled central directory size field). This defends
    // against zip bombs that lie about entry.size() in the central directory.
    const MAX_DOCX_EXTRACTED_BYTES: u64 = 50 * 1024 * 1024;
    // Per-entry and cumulative measured caps for ODT/PPTX.
    // Named separately from the DOCX constants so each format's limit is explicit.
    const MAX_ODT_PPTX_PER_ENTRY_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB per XML entry
    const MAX_ODT_PPTX_TOTAL_BYTES: u64 = 50 * 1024 * 1024; // 50 MiB cumulative

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
        // Fast-path pre-scan using declared sizes. This is advisory only — the
        // central directory is attacker-controlled, so we additionally bound
        // decompression by measured bytes below.
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

        // Authoritative guard: actually decompress each entry with a hard byte
        // cap enforced on the measured stream. This catches zip bombs that
        // under-report size() in the central directory.
        let mut measured_total: u64 = 0;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| RagError::Parse(format!("docx entry {i}: {e}")))?;
            let entry_name = entry.name().to_string();
            let remaining = MAX_DOCX_EXTRACTED_BYTES.saturating_sub(measured_total);
            // Read at most `remaining + 1` bytes so we can distinguish "exactly at
            // the budget" from "overran the budget".
            let read_cap = remaining.saturating_add(1);
            let mut sink = std::io::sink();
            let copied = std::io::copy(&mut (&mut entry).take(read_cap), &mut sink)
                .map_err(|e| RagError::Parse(format!("docx decompress '{entry_name}': {e}")))?;
            if copied > remaining {
                return Err(RagError::Parse(
                    "DOCX entry exceeds 50MB decompressed limit — possible zip bomb".to_string(),
                ));
            }
            measured_total = measured_total.saturating_add(copied);
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
    // Authoritative measured total for ODT/PPTX decompressed bytes.
    // This is incremented from the actual read count, NOT the advisory entry.size().
    let mut odt_pptx_measured_total: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| RagError::Parse(format!("{ext} entry {i}: {e}")))?;

        // Advisory pre-checks using the central directory declared size.
        // These are fast-path guards only — the central directory is attacker-controlled
        // so they must be backed up by the measured read below.
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

        // Authoritative measured guard: cap the actual decompression stream.
        // A crafted file can declare a tiny entry.size() in the central directory
        // while the actual decompressed content expands to gigabytes. We read at most
        // (cap + 1) bytes so we can distinguish "exactly at budget" from "overran".
        let remaining = MAX_ODT_PPTX_TOTAL_BYTES.saturating_sub(odt_pptx_measured_total);
        let per_entry_cap = MAX_ODT_PPTX_PER_ENTRY_BYTES.min(remaining);
        let read_cap = per_entry_cap.saturating_add(1);
        let mut xml_buf = String::new();
        (&mut entry)
            .take(read_cap)
            .read_to_string(&mut xml_buf)
            .map_err(|e| RagError::Parse(format!("{ext} read '{name}': {e}")))?;
        let read_bytes = xml_buf.len() as u64;
        if read_bytes > per_entry_cap {
            // Distinguish which cap fired: per-entry or cumulative.
            let reason = if per_entry_cap < MAX_ODT_PPTX_PER_ENTRY_BYTES {
                format!(
                    "cumulative budget ({} MiB total) reached",
                    MAX_ODT_PPTX_TOTAL_BYTES / (1024 * 1024)
                )
            } else {
                format!(
                    "per-entry limit ({} MiB) exceeded",
                    MAX_ODT_PPTX_PER_ENTRY_BYTES / (1024 * 1024)
                )
            };
            return Err(RagError::Parse(format!(
                "{ext}: entry '{name}' — {reason} — possible zip bomb"
            )));
        }
        odt_pptx_measured_total = odt_pptx_measured_total.saturating_add(read_bytes);

        let fragment = super::extract_xml_text(&xml_buf).unwrap_or_else(|_| xml_buf.clone());
        if !fragment.trim().is_empty() {
            text_parts.push(fragment);
        }
    }

    let text = text_parts.join("\n\n");
    let word_count = crate::chunker::word_count(&text);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an in-memory zip archive with a single entry.
    /// `entry_name`: path inside the zip (e.g. "content.xml")
    /// `content`: raw bytes to store (Deflated, which compresses repeated data heavily)
    fn make_zip_with_entry(entry_name: &str, content: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer.start_file(entry_name, options).expect("start_file");
        writer.write_all(content).expect("write_all");
        writer.finish().expect("finish").into_inner()
    }

    /// Verify that the DOCX guard is untouched: the existing DOCX path compiles and
    /// the entry-count bomb check fires correctly (the measured guard is tested by DOCX's
    /// own test suite; we just ensure structural integrity here).
    #[test]
    fn docx_entry_count_bomb_rejected() {
        // Build a zip with MAX_ENTRIES+1 (1001) empty entries as a DOCX.
        use std::io::Write;
        let buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for i in 0..1001usize {
            writer
                .start_file(format!("file{i}.txt"), options)
                .expect("start_file");
            writer.write_all(b"").expect("write_all");
        }
        let bytes = writer.finish().expect("finish").into_inner();

        let result = parse_office_zip_file(
            &bytes,
            "file:///test.docx".to_string(),
            "test".to_string(),
            "docx",
        );
        assert!(
            result.is_err(),
            "expected error for zip with 1001 entries, got Ok"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("zip bomb") || msg.contains("entries"),
            "unexpected error: {msg}"
        );
    }

    /// Core regression for noxa-5gf: an ODT file whose single content.xml entry
    /// expands to more than MAX_ODT_PPTX_PER_ENTRY_BYTES (10 MiB) must be rejected
    /// via the *measured* decompression guard — even if the advisory entry.size()
    /// value in the central directory declares a small size.
    ///
    /// We use a highly compressible XML payload (11 MiB of repeated ASCII) so the
    /// in-memory zip is only ~40 KiB but decompresses beyond the 10 MiB cap.
    #[test]
    fn odt_decompression_bomb_rejected_by_measured_guard() {
        // 11 MiB of valid-ish XML content — highly compressible.
        const BOMB_SIZE: usize = 11 * 1024 * 1024;
        let xml_content: String = std::iter::repeat("<text:p>AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA</text:p>\n")
            .flat_map(|s| s.chars())
            .take(BOMB_SIZE)
            .collect();

        // The zip crate writes the actual uncompressed size into the central directory,
        // so entry.size() will be 11 MiB. The advisory check (MAX_ENTRY_SIZE = 100 MiB)
        // would NOT fire here — only the measured per-entry cap (10 MiB) fires.
        let zip_bytes = make_zip_with_entry("content.xml", xml_content.as_bytes());

        let result = parse_office_zip_file(
            &zip_bytes,
            "file:///test.odt".to_string(),
            "Test Document".to_string(),
            "odt",
        );

        assert!(
            result.is_err(),
            "expected Err for ODT entry exceeding 10 MiB measured cap, got Ok"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("zip bomb") || msg.contains("decompression limit"),
            "expected zip bomb error message, got: {msg}"
        );
    }

    /// Verify the cumulative measured cap fires when multiple entries each stay
    /// under the per-entry cap but together exceed MAX_ODT_PPTX_TOTAL_BYTES (50 MiB).
    #[test]
    fn odt_cumulative_decompression_bomb_rejected() {
        // 6 entries × 9 MiB each = 54 MiB total > 50 MiB limit.
        // Each 9 MiB entry is under the 10 MiB per-entry cap, but cumulative > 50 MiB.
        const ENTRIES: usize = 6;
        const ENTRY_SIZE: usize = 9 * 1024 * 1024;

        use std::io::Write;
        let buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let xml_chunk: String = std::iter::repeat("<text:p>BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB</text:p>\n")
            .flat_map(|s| s.chars())
            .take(ENTRY_SIZE)
            .collect();

        for i in 0..ENTRIES {
            // All entries contain "content" in the name to pass the target_prefix filter.
            writer
                .start_file(format!("content{i}.xml"), options)
                .expect("start_file");
            writer.write_all(xml_chunk.as_bytes()).expect("write_all");
        }
        let zip_bytes = writer.finish().expect("finish").into_inner();

        let result = parse_office_zip_file(
            &zip_bytes,
            "file:///test.odt".to_string(),
            "Test Document".to_string(),
            "odt",
        );

        assert!(
            result.is_err(),
            "expected Err for cumulative ODT entries exceeding 50 MiB measured cap, got Ok"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("zip bomb") || msg.contains("decompression limit"),
            "expected zip bomb error message, got: {msg}"
        );
    }

    /// A legitimate small ODT must parse successfully.
    #[test]
    fn odt_small_legitimate_file_parses_ok() {
        let xml_content = r#"<?xml version="1.0"?><office:document-content xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"><office:body><office:text><text:p>Hello world</text:p></office:text></office:body></office:document-content>"#;
        let zip_bytes = make_zip_with_entry("content.xml", xml_content.as_bytes());

        let result = parse_office_zip_file(
            &zip_bytes,
            "file:///test.odt".to_string(),
            "Test".to_string(),
            "odt",
        );
        assert!(result.is_ok(), "expected Ok for small ODT, got: {result:?}");
    }
}
