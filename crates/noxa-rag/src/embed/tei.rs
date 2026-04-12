// TeiProvider — implemented in noxa-68r.3
use async_trait::async_trait;
use crate::embed::EmbedProvider;
use crate::error::RagError;

pub struct TeiProvider {
    pub(crate) client: reqwest::Client,
    pub(crate) url: String,
    pub(crate) model: String,
    pub(crate) dimensions: usize,
}

impl TeiProvider {
    pub async fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/health", self.url))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn name(&self) -> &str {
        "tei"
    }
}

#[async_trait]
impl EmbedProvider for TeiProvider {
    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, RagError> {
        // Full implementation in noxa-68r.3
        Err(RagError::Embed("TeiProvider not yet implemented".to_string()))
    }
}
