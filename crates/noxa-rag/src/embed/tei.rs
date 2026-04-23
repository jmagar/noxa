// TeiProvider — TEI (Text Embeddings Inference) embed provider
// Targets Qwen3-0.6B (1024-dim) served via Hugging Face TEI.
use crate::embed::EmbedProvider;
use crate::error::RagError;
use async_trait::async_trait;
use futures::StreamExt;

/// Batch size tuned for RTX 4070 (~3x throughput vs default 32).
const BATCH_SIZE: usize = 96;
/// Concurrent in-flight embed batches — overlaps HTTP latency with GPU compute.
const EMBED_PIPELINE_DEPTH: usize = 3;
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
    // "Right" drops from the tail of the user content, preserving the beginning of the
    // document AND ensuring EOS remains the final token — required for Qwen3 last-token
    // (pooling_mode_lasttoken = true) pooling.
    truncation_direction: &'static str,
    normalize: bool,
}

pub struct TeiProvider {
    pub(crate) client: reqwest::Client,
    pub(crate) url: String,
    pub(crate) dimensions: usize,
}

impl TeiProvider {
    /// Construct with hardcoded dimensions (1024 for Qwen3-0.6B).
    pub fn new(url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
            dimensions: DEFAULT_DIMENSIONS,
        }
    }

    /// Construct by probing /embed with a single dummy text to discover dimensions.
    pub async fn new_with_probe(url: String, client: reqwest::Client) -> Result<Self, RagError> {
        let dummy = vec!["probe".to_string()];
        let req = EmbedRequest {
            inputs: &dummy,
            truncate: true,
            truncation_direction: "Right",
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
            truncation_direction: "Right",
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

    async fn embed_batch_adaptive(
        &self,
        batch: &[String],
        batch_idx: usize,
        total_batches: usize,
    ) -> Result<Vec<Vec<f32>>, RagError> {
        let mut pending = vec![batch];
        let mut results = Vec::with_capacity(batch.len());

        while let Some(current) = pending.pop() {
            match self.embed_batch(current, batch_idx, total_batches).await {
                Ok(vecs) => results.extend(vecs),
                Err(RagError::Embed {
                    status: Some(413), ..
                }) if current.len() > 1 => {
                    let split_at = current.len().div_ceil(2);
                    let (left, right) = current.split_at(split_at);
                    tracing::warn!(
                        batch = batch_idx + 1,
                        total_batches,
                        chunks = current.len(),
                        left_chunks = left.len(),
                        right_chunks = right.len(),
                        "TEI 413 persisted after retry; splitting batch again"
                    );
                    pending.push(right);
                    pending.push(left);
                }
                Err(error) => return Err(error),
            }
        }

        Ok(results)
    }
}

#[async_trait]
impl EmbedProvider for TeiProvider {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RagError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let total_batches = texts.len().div_ceil(BATCH_SIZE);

        // Keep EMBED_PIPELINE_DEPTH batches in-flight concurrently so HTTP
        // round-trip latency overlaps with GPU compute on the TEI server.
        // buffered() preserves batch ordering.
        let batches: Vec<(usize, Vec<String>)> = texts
            .chunks(BATCH_SIZE)
            .enumerate()
            .map(|(i, chunk)| (i, chunk.to_vec()))
            .collect();

        let results: Vec<Vec<Vec<f32>>> = futures::stream::iter(batches)
            .map(|(batch_idx, batch)| async move {
                self.embed_batch_adaptive(&batch, batch_idx, total_batches)
                    .await
            })
            .buffered(EMBED_PIPELINE_DEPTH)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()?;

        Ok(results.into_iter().flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_DIMENSIONS, MAX_RETRIES, TeiProvider, should_retry};
    use crate::embed::EmbedProvider;
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn retry_limit_counts_retries_not_total_attempts() {
        assert!(should_retry(429, 0));
        assert!(should_retry(503, MAX_RETRIES - 1));
        assert!(!should_retry(429, MAX_RETRIES));
        assert!(!should_retry(500, 0));
    }

    #[derive(Clone, Debug)]
    struct RecordedRequest {
        body: String,
    }

    async fn spawn_embed_server(
        max_batch_size: usize,
    ) -> (
        String,
        Arc<Mutex<Vec<RecordedRequest>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);

        let handle = tokio::spawn(async move {
            'connection: loop {
                let Ok((mut stream, _peer)) = listener.accept().await else {
                    break;
                };

                let mut buffer = Vec::new();
                let header_end = loop {
                    let mut chunk = [0u8; 1024];
                    let n = match stream.read(&mut chunk).await {
                        Ok(n) => n,
                        Err(_) => continue 'connection,
                    };
                    if n == 0 {
                        continue 'connection;
                    }
                    buffer.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                        break pos + 4;
                    }
                };

                let headers = String::from_utf8_lossy(&buffer[..header_end]);
                let mut content_length = 0usize;
                for line in headers.lines().skip(1) {
                    if let Some((name, value)) = line.split_once(':')
                        && name.trim().eq_ignore_ascii_case("content-length")
                    {
                        content_length = value.trim().parse().unwrap_or(0);
                    }
                }

                while buffer.len() < header_end + content_length {
                    let mut chunk = [0u8; 1024];
                    let n = match stream.read(&mut chunk).await {
                        Ok(n) => n,
                        Err(_) => continue 'connection,
                    };
                    if n == 0 {
                        continue 'connection;
                    }
                    buffer.extend_from_slice(&chunk[..n]);
                }

                let body =
                    String::from_utf8_lossy(&buffer[header_end..header_end + content_length])
                        .to_string();
                recorded
                    .lock()
                    .unwrap()
                    .push(RecordedRequest { body: body.clone() });

                let request_json: Value = serde_json::from_str(&body).expect("json body");
                let batch_size = request_json["inputs"].as_array().map_or(0, Vec::len);
                let (status, response_body) = if batch_size > max_batch_size {
                    (413, "{\"error\":\"too large\"}".to_string())
                } else {
                    let embeddings: Vec<Vec<f32>> =
                        (0..batch_size).map(|_| vec![0.5, 0.25]).collect();
                    (
                        200,
                        serde_json::to_string(&embeddings).expect("serialize embeddings"),
                    )
                };

                let response = format!(
                    "HTTP/1.1 {status} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    if status == 200 {
                        "OK"
                    } else {
                        "Payload Too Large"
                    },
                    response_body.len(),
                    response_body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        (format!("http://{}", addr), requests, handle)
    }

    #[tokio::test]
    async fn embed_adapts_413_retries_until_batches_fit() {
        let (base_url, requests, handle) = spawn_embed_server(5).await;
        let provider = TeiProvider {
            client: reqwest::Client::new(),
            url: base_url,
            dimensions: DEFAULT_DIMENSIONS,
        };
        let texts: Vec<String> = (0..20).map(|index| format!("chunk {index}")).collect();

        let vectors = provider.embed(&texts).await.expect("adaptive embed");

        handle.abort();

        assert_eq!(vectors.len(), texts.len());

        let batch_sizes: Vec<usize> = requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| {
                let request_json: Value = serde_json::from_str(&request.body).expect("json");
                request_json["inputs"].as_array().map_or(0, Vec::len)
            })
            .collect();
        assert!(
            batch_sizes.contains(&20),
            "expected initial oversized batch, got: {batch_sizes:?}"
        );
        assert!(
            batch_sizes.iter().any(|size| *size > 5),
            "expected one or more 413 retries before success, got: {batch_sizes:?}"
        );
        assert!(
            batch_sizes.contains(&5),
            "expected recursive splits down to server limit, got: {batch_sizes:?}"
        );
    }
}
