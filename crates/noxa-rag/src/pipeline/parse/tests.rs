use serde_json::json;
use std::fs;

use super::{IngestionProvenance, build_point_payload, parse_file};

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
    assert_eq!(
        parsed.provenance.seed_url.as_deref(),
        Some("https://seed.example/")
    );
    assert_eq!(
        parsed.provenance.search_query.as_deref(),
        Some("rust agent")
    );
    assert_eq!(parsed.provenance.crawl_depth, Some(2));
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

#[test]
fn build_point_payload_serializes_provenance_fields_when_present() {
    let chunk = crate::types::Chunk {
        text: "chunk text".to_string(),
        source_url: "https://example.com/article".to_string(),
        domain: "example.com".to_string(),
        chunk_index: 0,
        total_chunks: 1,
        char_offset: 0,
        token_estimate: 2,
        section_header: None,
    };
    let extraction = noxa_core::ExtractionResult {
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
    };
    let provenance = IngestionProvenance {
        external_id: Some("linkding:42".to_string()),
        platform_url: Some("https://platform.example/items/42".to_string()),
        seed_url: None,
        search_query: None,
        crawl_depth: None,
        email_to: vec!["team@example.com".to_string()],
        email_message_id: Some("msg@example.com".to_string()),
        email_thread_id: Some("thread@example.com".to_string()),
        email_has_attachments: Some(true),
        feed_url: Some("https://example.com/feed.xml".to_string()),
        feed_item_id: Some("entry-1".to_string()),
        pptx_slide_count: Some(12),
        pptx_has_notes: Some(true),
        subtitle_start_s: Some(1.25),
        subtitle_end_s: Some(9.75),
        subtitle_source_file: Some("demo.mp4".to_string()),
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
    assert_eq!(
        json.get("seed_url").and_then(|v| v.as_str()),
        Some("https://seed.example/")
    );
    assert_eq!(
        json.get("search_query").and_then(|v| v.as_str()),
        Some("rust agent")
    );
    assert_eq!(json.get("crawl_depth").and_then(|v| v.as_u64()), Some(2));
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
        json.get("feed_url").and_then(|v| v.as_str()),
        Some("https://example.com/feed.xml")
    );
    assert_eq!(
        json.get("pptx_slide_count").and_then(|v| v.as_u64()),
        Some(12)
    );
    assert_eq!(
        json.get("subtitle_source_file").and_then(|v| v.as_str()),
        Some("demo.mp4")
    );
}
