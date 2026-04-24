use serde_json::json;
use std::fs;

use super::{FormatProvenance, IngestionProvenance, build_point_payload, parse_file};

#[tokio::test]
async fn parse_file_json_recovers_provenance_fields() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("page.json");
    let body = json!({
        "metadata": {
            "title": "Seeded document",
            "description": null,
            "author": "Alice",
            "published_date": null,
            "language": "en",
            "url": "https://example.com/article",
            "site_name": null,
            "image": null,
            "favicon": null,
            "word_count": 3,
            "content_hash": null,
            "source_type": "web",
            "file_path": null,
            "last_modified": null,
            "is_truncated": null,
            "technologies": [],
            "seed_url": "https://seed.example/",
            "crawl_depth": 2,
            "search_query": "rust agent",
            "fetched_at": null
        },
        "content": {
            "markdown": "hello world",
            "plain_text": "hello world",
            "links": [],
            "images": [],
            "code_blocks": [],
            "raw_html": null
        },
        "domain_data": null,
        "structured_data": [],
        "external_id": "linkding:42",
        "platform_url": "https://platform.example/items/42"
    });
    let bytes = serde_json::to_vec(&body).expect("serialize json");
    fs::write(&path, &bytes).expect("write file");

    let parsed = parse_file(&path, bytes).await.expect("parse json");

    assert_eq!(
        parsed.extraction.metadata.seed_url.as_deref(),
        Some("https://seed.example/")
    );
    assert_eq!(parsed.extraction.metadata.crawl_depth, Some(2));
    assert_eq!(
        parsed.extraction.metadata.search_query.as_deref(),
        Some("rust agent")
    );
    assert_eq!(
        parsed.provenance.external_id.as_deref(),
        Some("linkding:42")
    );
    assert_eq!(
        parsed.provenance.platform_url.as_deref(),
        Some("https://platform.example/items/42")
    );
    match &parsed.provenance.format {
        FormatProvenance::Web {
            seed_url,
            search_query,
            crawl_depth,
        } => {
            assert_eq!(seed_url.as_deref(), Some("https://seed.example/"));
            assert_eq!(search_query.as_deref(), Some("rust agent"));
            assert_eq!(*crawl_depth, Some(2));
        }
        other => panic!("expected FormatProvenance::Web, got {other:?}"),
    }
}

#[tokio::test]
async fn parse_file_json_keeps_crawler_provenance_in_point_payload() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("crawl.json");
    let body = json!({
        "metadata": {
            "title": "Crawled page",
            "description": null,
            "author": "Crawler",
            "published_date": null,
            "language": "en",
            "url": "https://example.com/articles/42",
            "site_name": "Example",
            "image": null,
            "favicon": null,
            "word_count": 6,
            "content_hash": null,
            "source_type": "web",
            "file_path": null,
            "last_modified": null,
            "is_truncated": null,
            "technologies": [],
            "seed_url": "https://seed.example/",
            "crawl_depth": 2,
            "search_query": null,
            "fetched_at": null
        },
        "content": {
            "markdown": "alpha beta gamma delta epsilon zeta",
            "plain_text": "alpha beta gamma delta epsilon zeta",
            "links": [],
            "images": [],
            "code_blocks": [],
            "raw_html": null
        },
        "domain_data": null,
        "structured_data": []
    });
    let bytes = serde_json::to_vec(&body).expect("serialize json");
    fs::write(&path, &bytes).expect("write file");

    let parsed = parse_file(&path, bytes).await.expect("parse json");
    let url = parsed
        .extraction
        .metadata
        .url
        .as_deref()
        .expect("parser should set file url");
    let chunk = crate::types::Chunk {
        text: parsed.extraction.content.markdown.clone(),
        source_url: url.to_string(),
        domain: "example.com".to_string(),
        chunk_index: 0,
        total_chunks: 1,
        char_offset: 0,
        token_estimate: 6,
        section_header: None,
    };

    let payload = build_point_payload(&chunk, &parsed.extraction, None, &parsed.provenance, url, None);
    let json = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(
        json.get("seed_url").and_then(|v| v.as_str()),
        Some("https://seed.example/")
    );
    assert_eq!(json.get("crawl_depth").and_then(|v| v.as_u64()), Some(2));
    assert!(
        json.get("search_query").is_none(),
        "crawler provenance should not invent a search query"
    );
}

fn sample_chunk() -> crate::types::Chunk {
    crate::types::Chunk {
        text: "chunk text".to_string(),
        source_url: "https://example.com/article".to_string(),
        domain: "example.com".to_string(),
        chunk_index: 0,
        total_chunks: 1,
        char_offset: 0,
        token_estimate: 2,
        section_header: None,
    }
}

fn sample_extraction_with_metadata() -> noxa_core::ExtractionResult {
    noxa_core::ExtractionResult {
        metadata: noxa_core::Metadata {
            title: Some("Seeded document".to_string()),
            description: None,
            author: Some("Alice".to_string()),
            published_date: None,
            language: Some("en".to_string()),
            url: Some("https://example.com/article".to_string()),
            site_name: None,
            image: None,
            favicon: None,
            word_count: 3,
            content_hash: None,
            source_type: Some("web".to_string()),
            file_path: None,
            last_modified: None,
            is_truncated: None,
            technologies: Vec::new(),
            seed_url: Some("https://seed.example/".to_string()),
            crawl_depth: Some(2),
            search_query: Some("rust agent".to_string()),
            fetched_at: None,
        },
        content: noxa_core::Content {
            markdown: "chunk text".to_string(),
            plain_text: "chunk text".to_string(),
            links: Vec::new(),
            images: Vec::new(),
            code_blocks: Vec::new(),
            raw_html: None,
        },
        domain_data: None,
        structured_data: Vec::new(),
    }
}

/// Web variant: external_id/platform_url at the top level, plus seed_url
/// and friends falling back to metadata when the variant fields are None.
#[test]
fn build_point_payload_serializes_web_variant() {
    let chunk = sample_chunk();
    let extraction = sample_extraction_with_metadata();
    let provenance = IngestionProvenance {
        external_id: Some("linkding:42".to_string()),
        platform_url: Some("https://platform.example/items/42".to_string()),
        format: FormatProvenance::Web {
            seed_url: None,
            search_query: None,
            crawl_depth: None,
        },
    };

    let payload = build_point_payload(&chunk, &extraction, None, &provenance, &chunk.source_url, None);
    let json = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(
        json.get("external_id").and_then(|v| v.as_str()),
        Some("linkding:42")
    );
    assert_eq!(
        json.get("platform_url").and_then(|v| v.as_str()),
        Some("https://platform.example/items/42")
    );
    // Fallback from result.metadata triggers when the variant's own fields are None.
    assert_eq!(
        json.get("seed_url").and_then(|v| v.as_str()),
        Some("https://seed.example/")
    );
    assert_eq!(
        json.get("search_query").and_then(|v| v.as_str()),
        Some("rust agent")
    );
    assert_eq!(json.get("crawl_depth").and_then(|v| v.as_u64()), Some(2));
    // Non-web variant fields stay at their defaults; PointPayload's
    // skip_serializing_if means absent keys in the JSON.
    assert!(json.get("email_to").is_none());
    assert!(json.get("feed_url").is_none());
    assert!(json.get("pptx_slide_count").is_none());
}

/// Email variant: all email_* fields populated, plus the external_id the
/// email parser sets from Message-ID.
#[test]
fn build_point_payload_serializes_email_variant() {
    let chunk = sample_chunk();
    let extraction = sample_extraction_with_metadata();
    let provenance = IngestionProvenance {
        external_id: Some("msg@example.com".to_string()),
        platform_url: None,
        format: FormatProvenance::Email {
            to: vec!["team@example.com".to_string()],
            message_id: Some("msg@example.com".to_string()),
            thread_id: Some("thread@example.com".to_string()),
            has_attachments: Some(true),
        },
    };

    let payload = build_point_payload(&chunk, &extraction, None, &provenance, &chunk.source_url, None);
    let json = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(
        json.get("email_to")
            .and_then(|v| v.as_array())
            .map(|values| values.len()),
        Some(1)
    );
    assert_eq!(
        json.get("email_message_id").and_then(|v| v.as_str()),
        Some("msg@example.com")
    );
    assert_eq!(
        json.get("email_thread_id").and_then(|v| v.as_str()),
        Some("thread@example.com")
    );
    assert_eq!(
        json.get("email_has_attachments").and_then(|v| v.as_bool()),
        Some(true)
    );
}

/// Feed variant: feed_url + feed_item_id.
#[test]
fn build_point_payload_serializes_feed_variant() {
    let chunk = sample_chunk();
    let extraction = sample_extraction_with_metadata();
    let provenance = IngestionProvenance {
        external_id: Some("entry-1".to_string()),
        platform_url: None,
        format: FormatProvenance::Feed {
            feed_url: Some("https://example.com/feed.xml".to_string()),
            item_id: Some("entry-1".to_string()),
        },
    };

    let payload = build_point_payload(&chunk, &extraction, None, &provenance, &chunk.source_url, None);
    let json = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(
        json.get("feed_url").and_then(|v| v.as_str()),
        Some("https://example.com/feed.xml")
    );
    assert_eq!(
        json.get("feed_item_id").and_then(|v| v.as_str()),
        Some("entry-1")
    );
}

/// Presentation variant: pptx_slide_count + pptx_has_notes.
#[test]
fn build_point_payload_serializes_presentation_variant() {
    let chunk = sample_chunk();
    let extraction = sample_extraction_with_metadata();
    let provenance = IngestionProvenance {
        external_id: None,
        platform_url: None,
        format: FormatProvenance::Presentation {
            slide_count: Some(12),
            has_notes: Some(true),
        },
    };

    let payload = build_point_payload(&chunk, &extraction, None, &provenance, &chunk.source_url, None);
    let json = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(
        json.get("pptx_slide_count").and_then(|v| v.as_u64()),
        Some(12)
    );
    assert_eq!(
        json.get("pptx_has_notes").and_then(|v| v.as_bool()),
        Some(true)
    );
}

/// Subtitle variant: subtitle_start_s / subtitle_end_s / subtitle_source_file.
#[test]
fn build_point_payload_serializes_subtitle_variant() {
    let chunk = sample_chunk();
    let extraction = sample_extraction_with_metadata();
    let provenance = IngestionProvenance {
        external_id: None,
        platform_url: None,
        format: FormatProvenance::Subtitle {
            start_s: Some(1.25),
            end_s: Some(9.75),
            source_file: Some("demo.mp4".to_string()),
        },
    };

    let payload = build_point_payload(&chunk, &extraction, None, &provenance, &chunk.source_url, None);
    let json = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(
        json.get("subtitle_start_s").and_then(|v| v.as_f64()),
        Some(1.25)
    );
    assert_eq!(
        json.get("subtitle_end_s").and_then(|v| v.as_f64()),
        Some(9.75)
    );
    assert_eq!(
        json.get("subtitle_source_file").and_then(|v| v.as_str()),
        Some("demo.mp4")
    );
}
