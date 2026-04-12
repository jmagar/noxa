// Chunker — implemented in noxa-68r.2
use noxa_core::types::ExtractionResult;
use crate::config::ChunkerConfig;
use crate::types::Chunk;

pub fn chunk(
    _result: &ExtractionResult,
    _config: &ChunkerConfig,
    _tokenizer: &tokenizers::Tokenizer,
) -> Vec<Chunk> {
    // Full implementation in noxa-68r.2
    vec![]
}
