// TeiProvider — TEI (Text Embeddings Inference) embed provider
// Targets Qwen3-0.6B (1024-dim) served via Hugging Face TEI.
use crate::embed::EmbedProvider;
use crate::error::RagError;
use async_trait::async_trait;

/// Batch size tuned for RTX 4070 (~3x throughput vs default 32).
const BATCH_SIZE: usize = 96;
/// Reduced batch size on HTTP 413.
const BATCH_SIZE_REDUCED: usize = 48;
/// Default embedding dimensions for Qwen3-0.6B.
const DEFAULT_DIMENSIONS: usize = 1024;
/// Per-batch request timeout.
const BATCH_TIMEOUT_SECS: u64 = 60;
/// Max retries on 429/503.
const MAX_RETRIES: u32 = 3;

fn should_retry(status: u16, attempt: u32) -> bool {
    (status == 429 || status == 503) && attempt < MAX_RETRIES
}

#[derive(serde::Serialize)]
struct EmbedRequest<'a> {
    inputs: &'a [String],
    truncate: bool,
    normalize: bool,
}

pub struct TeiProvider {
    pub(crate) client: reqwest::Client,
    pub(crate) url: String,
    pub(crate) _model: String,
    pub(crate) dimensions: usize,
}

impl TeiProvider {
    /// Construct with hardcoded dimensions (1024 for Qwen3-0.6B).
    pub fn new(url: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
            _model: model,
            dimensions: DEFAULT_DIMENSIONS,
        }
    }

    /// Construct by probing /embed with a single dummy text to discover dimensions.
    pub async fn new_with_probe(
        url: String,
        model: String,
        client: reqwest::Client,
    ) -> Result<Self, RagError> {
        let dummy = vec!["probe".to_string()];
        let req = EmbedRequest {
            inputs: &dummy,
            truncate: true,
            normalize: true,
        };
        let resp = client
            .post(format!("{}/embed", url))
            .timeout(std::time::Duration::from_secs(10))
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(RagError::Embed {
                message: format!("TEI probe failed with status {}", resp.status()),
                status: Some(resp.status().as_u16()),
            });
        }

        let vecs: Vec<Vec<f32>> = resp.json().await?;
        let dimensions =
            vecs.into_iter()
                .next()
                .map(|v| v.len())
                .ok_or_else(|| RagError::Embed {
                    message: "TEI probe returned empty embedding response".to_string(),
                    status: None,
                })?;

        Ok(Self {
            client,
            url,
            _model: model,
            dimensions,
        })
    }

    /// GET /health — must return 200 within 2 s.
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

    /// Send one batch to POST /embed.  Handles 429/503 with exponential back-off.
    /// Returns Err(RagError::Embed { status: Some(413) }) — caller should halve the batch.
    ///
    /// `batch_idx` and `total_batches` are passed in from the caller for structured log context.
    async fn embed_batch(
        &self,
        batch: &[String],
        batch_idx: usize,
        total_batches: usize,
    ) -> Result<Vec<Vec<f32>>, RagError> {
        let url = format!("{}/embed", self.url);
        let req_body = EmbedRequest {
            inputs: batch,
            truncate: true,
            normalize: true,
        };

        let mut delay_ms: u64 = 200;
        for attempt in 0..=MAX_RETRIES {
            tracing::debug!(
                batch = batch_idx + 1,
                total_batches,
                chunks = batch.len(),
                attempt = attempt + 1,
                "embedding batch"
            );

            let resp = self
                .client
                .post(&url)
                .timeout(std::time::Duration::from_secs(BATCH_TIMEOUT_SECS))
                .json(&req_body)
                .send()
                .await?;

            let status = resp.status();
            let status_u16 = status.as_u16();

            if status.is_success() {
                let vecs: Vec<Vec<f32>> = resp.json().await?;
                return Ok(vecs);
            }

            if status_u16 == 413 {
                // Caller must halve the batch; no point retrying at this size.
                tracing::warn!(
                    batch = batch_idx + 1,
                    chunks = batch.len(),
                    reduced_to = batch.len() / 2,
                    "TEI 413: payload too large, halving batch"
                );
                return Err(RagError::Embed {
                    message: format!(
                        "TEI returned 413 (payload too large) for batch of {}",
                        batch.len()
                    ),
                    status: Some(status_u16),
                });
            }

            if should_retry(status_u16, attempt) {
                let body = resp.text().await.unwrap_or_default();
                let preview: String = body.chars().take(512).collect();
                tracing::warn!(
                    batch = batch_idx + 1,
                    attempt = attempt + 1,
                    max_attempts = MAX_RETRIES + 1,
                    status = status_u16,
                    delay_ms,
                    body = preview,
                    "TEI retry"
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                delay_ms = (delay_ms * 2).min(2_000);
                continue;
            }

            if status_u16 == 429 || status_u16 == 503 {
                break;
            }

            let body = resp.text().await.unwrap_or_default();
            let preview: String = body.chars().take(512).collect();
            return Err(RagError::Embed {
                message: format!("TEI /embed returned HTTP {status_u16}: {preview}"),
                status: Some(status_u16),
            });
        }

        Err(RagError::Embed {
            message: "TEI /embed: max retries exceeded".to_string(),
            status: None,
        })
    }
}

#[async_trait]
impl EmbedProvider for TeiProvider {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RagError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let total_batches = texts.len().div_ceil(BATCH_SIZE);
        let mut results: Vec<Vec<f32>> = Vec::with_capacity(texts.len());

        for (batch_idx, chunk) in texts.chunks(BATCH_SIZE).enumerate() {
            match self.embed_batch(chunk, batch_idx, total_batches).await {
                Ok(vecs) => results.extend(vecs),
                Err(RagError::Embed {
                    status: Some(413), ..
                }) => {
                    // Halve batch size and retry. Propagate real errors directly.
                    let sub_total = chunk.len().div_ceil(BATCH_SIZE_REDUCED);
                    let mut chunk_results: Vec<Vec<f32>> = Vec::with_capacity(chunk.len());
                    for (sub_idx, sub_chunk) in chunk.chunks(BATCH_SIZE_REDUCED).enumerate() {
                        tracing::debug!(
                            sub_batch = sub_idx + 1,
                            sub_total,
                            chunks = sub_chunk.len(),
                            "embedding reduced sub-batch after 413"
                        );
                        let vecs = self
                            .embed_batch(sub_chunk, batch_idx, total_batches)
                            .await?;
                        chunk_results.extend(vecs);
                    }
                    results.extend(chunk_results);
                }
                Err(e) => return Err(e),
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_RETRIES, should_retry};

    #[test]
    fn retry_limit_counts_retries_not_total_attempts() {
        assert!(should_retry(429, 0));
        assert!(should_retry(503, MAX_RETRIES - 1));
        assert!(!should_retry(429, MAX_RETRIES));
        assert!(!should_retry(500, 0));
    }
}
