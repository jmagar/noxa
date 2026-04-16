use std::sync::Arc;

use tokio::sync::Semaphore;
use tracing::warn;

use crate::client::{BatchExtractResult, BatchResult, FetchClient};

impl FetchClient {
    pub async fn fetch_batch(
        self: &Arc<Self>,
        urls: &[&str],
        concurrency: usize,
    ) -> Vec<BatchResult> {
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let mut handles = Vec::with_capacity(urls.len());

        for (idx, url) in urls.iter().enumerate() {
            let permit = Arc::clone(&semaphore);
            let client = Arc::clone(self);
            let url = url.to_string();

            handles.push(tokio::spawn(async move {
                let _permit = permit.acquire().await.expect("semaphore closed");
                let result = client.fetch(&url).await;
                (idx, BatchResult { url, result })
            }));
        }

        collect_ordered(handles, urls.len()).await
    }

    pub async fn fetch_and_extract_batch(
        self: &Arc<Self>,
        urls: &[&str],
        concurrency: usize,
    ) -> Vec<BatchExtractResult> {
        self.fetch_and_extract_batch_with_options(
            urls,
            concurrency,
            &noxa_core::ExtractionOptions::default(),
        )
        .await
    }

    pub async fn fetch_and_extract_batch_with_options(
        self: &Arc<Self>,
        urls: &[&str],
        concurrency: usize,
        options: &noxa_core::ExtractionOptions,
    ) -> Vec<BatchExtractResult> {
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let mut handles = Vec::with_capacity(urls.len());

        for (idx, url) in urls.iter().enumerate() {
            let permit = Arc::clone(&semaphore);
            let client = Arc::clone(self);
            let url = url.to_string();
            let opts = options.clone();

            handles.push(tokio::spawn(async move {
                let _permit = permit.acquire().await.expect("semaphore closed");
                let result = client.fetch_and_extract_with_options(&url, &opts).await;
                (idx, BatchExtractResult { url, result })
            }));
        }

        collect_ordered(handles, urls.len()).await
    }
}

pub(super) async fn collect_ordered<T>(
    handles: Vec<tokio::task::JoinHandle<(usize, T)>>,
    len: usize,
) -> Vec<T> {
    let mut slots: Vec<Option<T>> = (0..len).map(|_| None).collect();

    for handle in handles {
        match handle.await {
            Ok((idx, result)) => slots[idx] = Some(result),
            Err(error) => warn!(error = %error, "batch task panicked"),
        }
    }

    slots.into_iter().flatten().collect()
}
