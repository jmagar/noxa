// Pipeline — implemented in noxa-68r.7
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::config::RagConfig;
use crate::embed::DynEmbedProvider;
use crate::error::RagError;
use crate::store::DynVectorStore;

pub struct Pipeline {
    pub config: RagConfig,
    pub embed: DynEmbedProvider,
    pub store: DynVectorStore,
    pub tokenizer: Arc<tokenizers::Tokenizer>,
    pub shutdown: CancellationToken,
}

impl Pipeline {
    pub async fn run(&self) -> Result<(), RagError> {
        // Full implementation in noxa-68r.7
        self.shutdown.cancelled().await;
        Ok(())
    }
}
