// Factory — implemented in noxa-68r.6
use crate::config::RagConfig;
use crate::embed::DynEmbedProvider;
use crate::error::RagError;
use crate::store::DynVectorStore;

pub async fn build_embed_provider(_config: &RagConfig) -> Result<DynEmbedProvider, RagError> {
    // Full implementation in noxa-68r.6
    Err(RagError::Config("factory not yet implemented".to_string()))
}

pub async fn build_vector_store(
    _config: &RagConfig,
    _embed_dims: usize,
) -> Result<DynVectorStore, RagError> {
    // Full implementation in noxa-68r.6
    Err(RagError::Config("factory not yet implemented".to_string()))
}
