use async_trait::async_trait;

use crate::client::FetchClient;
use crate::error::FetchError;

#[async_trait]
pub trait ExtractorHttp: Send + Sync {
    async fn get_text(&self, url: &str) -> Result<String, FetchError>;
    async fn get_json(&self, url: &str) -> Result<serde_json::Value, FetchError>;
}

#[async_trait]
impl ExtractorHttp for FetchClient {
    async fn get_text(&self, url: &str) -> Result<String, FetchError> {
        self.fetch(url).await.map(|result| result.html)
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, FetchError> {
        let text = self.get_text(url).await?;
        serde_json::from_str(&text).map_err(|error| FetchError::BodyDecode(error.to_string()))
    }
}
