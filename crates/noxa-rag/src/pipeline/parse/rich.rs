use crate::error::RagError;

use super::{
    FormatProvenance, IngestionProvenance, ParsedFile, extract_xml_text, make_text_result,
    text::contains_xml_entity_expansion_risk,
};

pub(crate) fn parse_feed_file(
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
    let (extraction, provenance) = parse_feed_text(&content, file_url, title)?;
    Ok(ParsedFile {
        extraction,
        provenance,
    })
}

pub(crate) fn parse_email_file(
    bytes: &[u8],
    file_url: String,
    title: String,
) -> Result<ParsedFile, RagError> {
    let (extraction, provenance) = parse_email_text(bytes, file_url, title)?;
    Ok(ParsedFile {
        extraction,
        provenance,
    })
}

pub(crate) fn parse_subtitle_file(bytes: Vec<u8>, file_url: String, title: String) -> ParsedFile {
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let text = strip_subtitle_timestamps(&content);
    let provenance = subtitle_provenance(&content);
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
        provenance,
    }
}

fn parse_email_text(
    bytes: &[u8],
    file_url: String,
    title: String,
) -> Result<(noxa_core::types::ExtractionResult, IngestionProvenance), RagError> {
    use mailparse::{MailHeaderMap, addrparse_header, parse_mail};

    let parsed = parse_mail(bytes).map_err(|e| RagError::Parse(format!("EML parse: {e}")))?;
    let headers = parsed.get_headers();

    let subject = headers
        .get_first_value("Subject")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| Some(title.clone()));
    let to = parsed
        .headers
        .get_first_header("To")
        .and_then(|header| addrparse_header(header).ok())
        .map(|addresses| {
            addresses
                .iter()
                .flat_map(flatten_mail_addrs)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let message_id = headers
        .get_first_value("Message-ID")
        .map(|value| normalize_message_id(&value));
    let thread_id = email_thread_id(&headers);
    let published_date = headers
        .get_first_value("Date")
        .and_then(|value| mailparse::dateparse(&value).ok())
        .and_then(|timestamp| chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0))
        .map(|dt| dt.to_rfc3339());
    let body = collect_email_body(&parsed);
    let author = parsed
        .headers
        .get_first_header("From")
        .and_then(|header| addrparse_header(header).ok())
        .and_then(|addresses| addresses.iter().flat_map(flatten_mail_addrs).next());
    let has_attachments = parsed.parts().skip(1).any(|part| {
        part.get_content_disposition().disposition == mailparse::DispositionType::Attachment
            || part
                .get_content_disposition()
                .params
                .contains_key("filename")
    });
    let word_count = body.split_whitespace().count();

    let mut extraction =
        make_text_result(body.clone(), body, file_url, subject, "email", word_count);
    extraction.metadata.author = author;
    extraction.metadata.published_date = published_date;

    Ok((
        extraction,
        IngestionProvenance {
            external_id: message_id.clone(),
            platform_url: None,
            format: FormatProvenance::Email {
                to,
                message_id,
                thread_id,
                has_attachments: Some(has_attachments),
            },
        },
    ))
}

fn parse_feed_text(
    content: &str,
    file_url: String,
    title: String,
) -> Result<(noxa_core::types::ExtractionResult, IngestionProvenance), RagError> {
    let feed = feed_rs::parser::parse(content.as_bytes())
        .map_err(|e| RagError::Parse(format!("feed parse: {e}")))?;

    let feed_title = feed
        .title
        .as_ref()
        .map(|value| value.content.clone())
        .filter(|value| !value.is_empty())
        .or_else(|| Some(title.clone()));
    let primary_entry = feed.entries.first();
    let entry_title = primary_entry
        .and_then(|entry| entry.title.as_ref())
        .map(|value| value.content.clone())
        .filter(|value| !value.is_empty());
    let primary_author = primary_entry
        .and_then(|entry| entry.authors.first())
        .map(|author| author.name.clone())
        .or_else(|| feed.authors.first().map(|author| author.name.clone()));
    let published_date = primary_entry
        .and_then(|entry| entry.published.or(entry.updated))
        .or(feed.published.or(feed.updated))
        .map(|dt| dt.to_rfc3339());
    let feed_url = feed
        .links
        .first()
        .map(|link| link.href.clone())
        .or_else(|| {
            primary_entry
                .and_then(|entry| entry.links.first())
                .map(|link| link.href.clone())
        });
    let feed_item_id = primary_entry
        .map(|entry| entry.id.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut parts = Vec::new();
    for entry in &feed.entries {
        if let Some(value) = entry
            .title
            .as_ref()
            .map(|text| text.content.trim())
            .filter(|v| !v.is_empty())
        {
            parts.push(value.to_string());
        }
        if let Some(value) = entry
            .summary
            .as_ref()
            .map(|text| text.content.trim())
            .filter(|v| !v.is_empty())
        {
            parts.push(value.to_string());
        }
        if let Some(value) = entry
            .content
            .as_ref()
            .and_then(|content| content.body.as_ref())
            .map(|body| body.trim())
            .filter(|v| !v.is_empty())
        {
            parts.push(value.to_string());
        }
    }
    if parts.is_empty() {
        parts.push(extract_xml_text(content).unwrap_or_else(|_| content.to_string()));
    }
    let text = parts.join("\n\n");
    let word_count = text.split_whitespace().count();

    let mut extraction = make_text_result(
        text.clone(),
        text,
        file_url,
        entry_title.or_else(|| feed_title.clone()),
        "file",
        word_count,
    );
    extraction.metadata.author = primary_author;
    extraction.metadata.published_date = published_date;
    extraction.metadata.site_name = feed_title;
    extraction.metadata.language = feed.language.clone();

    Ok((
        extraction,
        IngestionProvenance {
            external_id: feed_item_id.clone().or_else(|| {
                let id = feed.id.trim();
                (!id.is_empty()).then(|| id.to_string())
            }),
            platform_url: None,
            format: FormatProvenance::Feed {
                feed_url,
                item_id: feed_item_id,
            },
        },
    ))
}

fn flatten_mail_addrs(addrs: &mailparse::MailAddr) -> Vec<String> {
    match addrs {
        mailparse::MailAddr::Single(info) => vec![info.addr.clone()],
        mailparse::MailAddr::Group(group) => group
            .addrs
            .iter()
            .map(|info| info.addr.clone())
            .collect::<Vec<_>>(),
    }
}

fn normalize_message_id(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string()
}

fn email_thread_id(headers: &mailparse::headers::Headers<'_>) -> Option<String> {
    use mailparse::MailHeaderMap;

    fn parse_first_message_id(value: &str) -> Option<String> {
        mailparse::msgidparse(value)
            .ok()
            .and_then(|ids| ids.first().cloned())
            .map(|value| normalize_message_id(&value))
    }

    headers
        .get_first_header("References")
        .and_then(|header| parse_first_message_id(&header.get_value()))
        .or_else(|| {
            headers
                .get_first_header("In-Reply-To")
                .and_then(|header| parse_first_message_id(&header.get_value()))
        })
}

fn collect_email_body(parsed: &mailparse::ParsedMail<'_>) -> String {
    let mut plain_parts = Vec::new();
    let mut html_parts = Vec::new();
    for part in parsed.parts() {
        if part.ctype.mimetype == "text/plain" {
            if let Ok(body) = part.get_body() {
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    plain_parts.push(trimmed.to_string());
                }
            }
        } else if part.ctype.mimetype == "text/html"
            && let Ok(body) = part.get_body()
        {
            let trimmed = body.trim();
            if !trimmed.is_empty() {
                html_parts.push(trimmed.to_string());
            }
        }
    }

    if !plain_parts.is_empty() {
        plain_parts.join("\n\n")
    } else if !html_parts.is_empty() {
        html_parts.join("\n\n")
    } else {
        parsed.get_body().unwrap_or_default()
    }
}

fn subtitle_provenance(content: &str) -> IngestionProvenance {
    let mut start_s = None;
    let mut end_s = None;
    let mut source_file = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("NOTE source:")
            .or_else(|| trimmed.strip_prefix("NOTE Source:"))
            .or_else(|| trimmed.strip_prefix("SOURCE FILE:"))
        {
            let value = rest.trim();
            if !value.is_empty() {
                source_file = Some(value.to_string());
            }
        }

        if let Some((start, end)) = parse_subtitle_range(trimmed) {
            start_s = Some(start_s.map_or(start, |current: f64| current.min(start)));
            end_s = Some(end_s.map_or(end, |current: f64| current.max(end)));
        }
    }

    IngestionProvenance {
        external_id: None,
        platform_url: None,
        format: FormatProvenance::Subtitle {
            start_s,
            end_s,
            source_file,
        },
    }
}

fn parse_subtitle_range(line: &str) -> Option<(f64, f64)> {
    let (start, end) = line.split_once("-->")?;
    let start_s = parse_subtitle_timestamp(start.trim())?;
    let end_token = end.split_whitespace().next()?;
    let end_s = parse_subtitle_timestamp(end_token.trim())?;
    Some((start_s, end_s))
}

fn parse_subtitle_timestamp(value: &str) -> Option<f64> {
    let value = value.replace(',', ".");
    let parts: Vec<&str> = value.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [hours, minutes, seconds] => (
            hours.parse::<f64>().ok()?,
            minutes.parse::<f64>().ok()?,
            seconds.parse::<f64>().ok()?,
        ),
        [minutes, seconds] => (
            0.0,
            minutes.parse::<f64>().ok()?,
            seconds.parse::<f64>().ok()?,
        ),
        _ => return None,
    };
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn strip_subtitle_timestamps(content: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let content_lines: Vec<&str> = content.lines().collect();
    for (index, line) in content_lines.iter().enumerate() {
        let trimmed = line.trim();
        let next = content_lines
            .get(index + 1)
            .map(|line| line.trim())
            .unwrap_or("");
        if trimmed.is_empty()
            || trimmed.starts_with("WEBVTT")
            || trimmed.starts_with("NOTE")
            || trimmed.starts_with("STYLE")
            || trimmed.starts_with("REGION")
            || trimmed.contains("-->")
            || is_subtitle_sequence_marker(trimmed, next)
        {
            continue;
        }
        lines.push(trimmed);
    }
    lines.join(" ")
}

fn is_subtitle_sequence_marker(line: &str, next_line: &str) -> bool {
    line.chars().all(|c| c.is_ascii_digit()) && next_line.contains("-->")
}
