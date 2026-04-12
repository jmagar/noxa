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

/// Build an overlap prefix from the end of `prev_text`, capped at `overlap_tokens` tokens.
///
/// Scans backwards through whitespace-separated words, checking the budget before
/// adding each word (so we never exceed `overlap_tokens`). O(n) via a reversed
/// accumulator that is flipped at the end.
fn overlap_prefix(prev_text: &str, overlap_tokens: usize, tokenizer: &Tokenizer) -> String {
    if overlap_tokens == 0 || prev_text.is_empty() {
        return String::new();
    }

    let words: Vec<&str> = prev_text.split_whitespace().collect();
    if words.is_empty() {
        return String::new();
    }

    let mut selected_rev: Vec<&str> = Vec::new();
    let mut token_count = 0usize;

    for &word in words.iter().rev() {
        let word_tokens = token_estimate(word, tokenizer);
        if token_count + word_tokens > overlap_tokens {
            break;
        }
        token_count += word_tokens;
        selected_rev.push(word);
    }

    selected_rev.reverse();
    selected_rev.join(" ")
}

/// Chunk an `ExtractionResult` into a `Vec<Chunk>`.
///
/// - Uses `content.markdown` if non-empty, otherwise `content.plain_text`.
/// - Empty content (both empty) → `Vec::new()`.
/// - Implements manual sliding-window overlap (text-splitter has no built-in overlap).
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
    let source_url: String = result
        .metadata
        .url
        .as_deref()
        .unwrap_or("")
        .to_string();
    let domain = extract_domain(&source_url);

    // Build the splitter with a token-range chunk config.
    // Use (target - 112)..target as the range; handle pathological configs safely.
    let upper = config.target_tokens.max(2);
    let lower = upper.saturating_sub(112).max(1);
    // Ensure lower < upper so the range is valid.
    let lower = lower.min(upper - 1);

    let splitter = MarkdownSplitter::new(
        ChunkConfig::new(lower..upper).with_sizer(tokenizer.clone()),
    );

    // Split and collect (char_offset, chunk_text) pairs via chunk_char_indices.
    let raw_chunks: Vec<(usize, String)> = splitter
        .chunk_char_indices(text)
        .map(|ci| (ci.char_offset, ci.chunk.to_string()))
        .collect();

    if raw_chunks.is_empty() {
        return Vec::new();
    }

    // Apply sliding-window overlap: each chunk (except the first) gets a prefix
    // consisting of the last `overlap_tokens` tokens of the previous raw chunk text.
    let mut chunks_with_overlap: Vec<(usize, String)> = Vec::with_capacity(raw_chunks.len());

    for (i, (offset, chunk_text)) in raw_chunks.iter().enumerate() {
        let text_with_overlap: String = if i == 0 || config.overlap_tokens == 0 {
            chunk_text.clone()
        } else {
            let prev_text = &raw_chunks[i - 1].1;
            let prefix = overlap_prefix(prev_text, config.overlap_tokens, tokenizer);
            if prefix.is_empty() {
                chunk_text.clone()
            } else {
                format!("{}\n\n{}", prefix, chunk_text)
            }
        };
        chunks_with_overlap.push((*offset, text_with_overlap));
    }

    // Filter by min_words, then cap at max_chunks_per_page.
    let filtered: Vec<(usize, String)> = chunks_with_overlap
        .into_iter()
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
