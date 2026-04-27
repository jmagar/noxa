use noxa_core::types::ExtractionResult;
use text_splitter::{ChunkConfig, MarkdownSplitter};
use tokenizers::Tokenizer;

use crate::config::ChunkerConfig;
use crate::types::Chunk;

/// Count whitespace-separated words in a string.
pub(crate) fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Extract markdown h1–h3 headings with their byte offsets.
fn extract_headings(text: &str) -> Vec<(usize, String)> {
    let mut headings = Vec::new();
    let mut byte_offset = 0usize;
    for line in text.split('\n') {
        let content = line.trim_end_matches('\r');
        let hash_count = content.bytes().take_while(|&b| b == b'#').count();
        if (1..=3).contains(&hash_count) && content.as_bytes().get(hash_count) == Some(&b' ') {
            let header_text = content[hash_count + 1..].trim().to_string();
            if !header_text.is_empty() {
                headings.push((byte_offset, header_text));
            }
        }
        byte_offset += line.len() + 1; // +1 for the '\n' split on
    }
    headings
}

/// Find the nearest heading at or before `chunk_offset`.
fn nearest_heading(headings: &[(usize, String)], chunk_offset: usize) -> Option<&str> {
    let pos = headings.partition_point(|(offset, _)| *offset <= chunk_offset);
    pos.checked_sub(1).map(|i| headings[i].1.as_str())
}

/// Extract the domain/host from a URL string.
fn extract_domain(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default()
}

/// Chunk an `ExtractionResult` into a `Vec<Chunk>`.
///
/// - Uses `content.markdown` if non-empty, otherwise `content.plain_text`.
/// - Empty content (both empty) → `Vec::new()`.
/// - Uses `ChunkConfig::with_overlap()` for sliding-window overlap (built into text-splitter ≥0.25).
/// - Filters chunks below `config.min_words`.
/// - Caps output at `config.max_chunks_per_page`.
///
/// # Token estimate
/// The `Chunk::token_estimate` field is populated with a word-count approximation.
/// The tokenizer is still used exclusively by the splitter for accurate boundary placement
/// via `ChunkConfig::with_sizer`; re-encoding every emitted chunk would halve throughput
/// on the `spawn_blocking` hot path for no practical gain (the field is diagnostic only).
pub fn chunk(
    result: &ExtractionResult,
    config: &ChunkerConfig,
    tokenizer: &Tokenizer,
) -> Vec<Chunk> {
    // Pick input text: markdown preferred, plain_text fallback.
    let text: &str = if !result.content.markdown.is_empty() {
        &result.content.markdown
    } else if !result.content.plain_text.is_empty() {
        &result.content.plain_text
    } else {
        return Vec::new();
    };

    // Source URL and domain.
    let source_url: String = result.metadata.url.as_deref().unwrap_or("").to_string();
    let domain = extract_domain(&source_url);

    // Extract heading positions for section_header assignment.
    let headings = extract_headings(text);

    // Build the splitter with a token-range chunk config.
    // Use (target - 112)..target as the range; handle pathological configs safely.
    let upper = config.target_tokens.max(2);
    let lower = upper.saturating_sub(112).max(1);
    // Ensure lower < upper so the range is valid.
    let lower = lower.min(upper - 1);

    // `with_overlap(n)` passes n tokens of the previous chunk as a prefix to the next.
    // Returns Err only if overlap >= capacity, which cannot happen here (overlap_tokens < lower).
    let chunk_config = ChunkConfig::new(lower..upper)
        .with_sizer(tokenizer)
        .with_overlap(config.overlap_tokens)
        .unwrap_or_else(|_| ChunkConfig::new(lower..upper).with_sizer(tokenizer));

    let splitter = MarkdownSplitter::new(chunk_config);

    // Filter by min_words, then cap at max_chunks_per_page.
    let filtered: Vec<(usize, String)> = splitter
        .chunk_char_indices(text)
        .map(|ci| (ci.char_offset, ci.chunk.to_string()))
        .filter(|(_, t)| word_count(t) >= config.min_words)
        .take(config.max_chunks_per_page)
        .collect();

    if filtered.is_empty() {
        return Vec::new();
    }

    let total_chunks = filtered.len();

    filtered
        .into_iter()
        .enumerate()
        .map(|(chunk_index, (char_offset, text))| {
            let t_est = word_count(&text);
            let section_header = nearest_heading(&headings, char_offset).map(|s| s.to_string());
            Chunk {
                text,
                source_url: source_url.clone(),
                domain: domain.clone(),
                chunk_index,
                total_chunks,
                char_offset,
                token_estimate: t_est,
                section_header,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr as _;

    fn make_extraction_result(markdown: &str) -> ExtractionResult {
        crate::pipeline::parse::make_text_result(
            markdown.to_string(),
            String::new(),
            "https://example.com/test".to_string(),
            None,
            "test",
            crate::chunker::word_count(markdown),
        )
    }

    /// Build a minimal whitespace-pretokenized WordLevel tokenizer suitable for
    /// unit tests. Every word becomes a distinct token (unk token used for anything
    /// not in the small vocab), which is sufficient for splitter boundary logic.
    fn make_test_tokenizer() -> Tokenizer {
        // A minimal valid tokenizer JSON: WordLevel model with whitespace pre-tokenizer.
        // Using from_str avoids the ahash::AHashMap type constraint on WordLevelBuilder::vocab
        // and the TokenizerImpl→Tokenizer conversion from TokenizerBuilder.
        let json = serde_json::json!({
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [],
            "normalizer": null,
            "pre_tokenizer": {
                "type": "Whitespace"
            },
            "post_processor": null,
            "decoder": null,
            "model": {
                "type": "WordLevel",
                "vocab": {
                    "[UNK]": 0,
                    "the": 1,
                    "and": 2,
                    "of": 3,
                    "a": 4,
                    "to": 5,
                    "in": 6,
                    "is": 7,
                    "it": 8,
                    "that": 9
                },
                "unk_token": "[UNK]"
            }
        });
        json.to_string()
            .parse::<Tokenizer>()
            .expect("test tokenizer from JSON")
    }

    #[test]
    fn domain_extraction() {
        assert_eq!(
            extract_domain("https://docs.example.com/foo"),
            "docs.example.com"
        );
        assert_eq!(extract_domain(""), "");
        assert_eq!(extract_domain("not-a-url"), "");
    }

    #[test]
    fn word_count_basic() {
        assert_eq!(word_count("hello world foo"), 3);
        assert_eq!(word_count("  "), 0);
        assert_eq!(word_count(""), 0);
    }

    /// Verify that no double-tokenization occurs: token_estimate is populated with
    /// word_count (not from the tokenizer), and chunks are produced for non-trivial input.
    #[test]
    fn chunk_token_estimate_uses_word_count_not_tokenizer() {
        let tokenizer = make_test_tokenizer();
        let config = crate::config::ChunkerConfig {
            target_tokens: 50,
            overlap_tokens: 0,
            min_words: 1,
            max_chunks_per_page: 200,
        };

        // A body of text long enough to produce at least one chunk.
        let body = (0..200)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let result = make_extraction_result(&body);
        let chunks = chunk(&result, &config, &tokenizer);

        assert!(
            !chunks.is_empty(),
            "expected at least one chunk for a 200-word body"
        );

        for c in &chunks {
            // token_estimate must be populated (> 0 for any non-empty chunk).
            assert!(
                c.token_estimate > 0,
                "token_estimate must be > 0, got {}",
                c.token_estimate
            );
            // token_estimate == word_count of the chunk text (the word_count approximation).
            let expected = word_count(&c.text);
            assert_eq!(
                c.token_estimate, expected,
                "token_estimate should equal word_count for chunk at index {}",
                c.chunk_index
            );
        }
    }

    /// Confirm empty content returns no chunks.
    #[test]
    fn chunk_empty_content_returns_empty() {
        let tokenizer = make_test_tokenizer();
        let config = crate::config::ChunkerConfig::default();
        let result = make_extraction_result("");
        let chunks = chunk(&result, &config, &tokenizer);
        assert!(chunks.is_empty());
    }
}
