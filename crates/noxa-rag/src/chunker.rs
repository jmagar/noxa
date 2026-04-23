use noxa_core::types::ExtractionResult;
use text_splitter::{ChunkConfig, MarkdownSplitter};
use tokenizers::Tokenizer;

use crate::config::ChunkerConfig;
use crate::types::Chunk;

/// Count whitespace-separated words in a string.
fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Extract the domain/host from a URL string.
fn extract_domain(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default()
}

/// Approximate token count — use the tokenizer when possible, fall back to word count.
fn token_estimate(text: &str, tokenizer: &Tokenizer) -> usize {
    tokenizer
        .encode(text, false)
        .map(|enc| enc.len())
        .unwrap_or_else(|_| text.split_whitespace().count())
}

/// Chunk an `ExtractionResult` into a `Vec<Chunk>`.
///
/// - Uses `content.markdown` if non-empty, otherwise `content.plain_text`.
/// - Empty content (both empty) → `Vec::new()`.
/// - Uses `ChunkConfig::with_overlap()` for sliding-window overlap (built into text-splitter ≥0.25).
/// - Filters chunks below `config.min_words`.
/// - Caps output at `config.max_chunks_per_page`.
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
            let t_est = token_estimate(&text, tokenizer);
            Chunk {
                text,
                source_url: source_url.clone(),
                domain: domain.clone(),
                chunk_index,
                total_chunks,
                char_offset,
                token_estimate: t_est,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
