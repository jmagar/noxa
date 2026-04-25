use std::time::{Duration, Instant};

use chrono::Utc;
use tracing::{debug, instrument, warn};

use crate::client::{FetchClient, FetchResult, Response};
use crate::error::FetchError;

impl FetchClient {
    pub async fn map_site(&self, url: &str) -> Result<Vec<crate::SitemapEntry>, String> {
        let entries = crate::sitemap::discover(self, url)
            .await
            .map_err(|e| format!("sitemap discovery failed: {e}"))?;

        if let Some(ref log) = self.ops_log {
            let domain = noxa_store::domain_from_url(url);
            let entry = noxa_store::OperationEntry {
                op: noxa_store::Op::Map,
                at: chrono::Utc::now(),
                url: url.to_string(),
                input: serde_json::json!({}),
                output: serde_json::json!({
                    "count": entries.len(),
                    "urls": entries.iter().map(|e| e.url.clone()).collect::<Vec<_>>()
                }),
            };
            if let Err(error) = log.append(&domain, &entry).await {
                tracing::warn!("ops log append failed for map: {error}");
            }
        }

        Ok(entries)
    }

    #[instrument(skip(self), fields(url = %url))]
    pub async fn fetch(&self, url: &str) -> Result<FetchResult, FetchError> {
        let delays = [Duration::ZERO, Duration::from_secs(1)];
        let mut last_err = None;

        for (attempt, delay) in delays.iter().enumerate() {
            if attempt > 0 {
                tokio::time::sleep(*delay).await;
            }

            match self.fetch_once(url).await {
                Ok(result) => {
                    if is_retryable_status(result.status) {
                        warn!(
                            url,
                            status = result.status,
                            attempt = attempt + 1,
                            "retryable status, will retry"
                        );
                        last_err = Some(FetchError::HttpStatus(result.status));
                        continue;
                    }
                    if attempt > 0 {
                        debug!(url, attempt = attempt + 1, "retry succeeded");
                    }
                    return Ok(result);
                }
                Err(error) => {
                    if !is_retryable_error(&error) || attempt == delays.len() - 1 {
                        return Err(error);
                    }
                    warn!(
                        url,
                        error = %error,
                        attempt = attempt + 1,
                        "transient error, will retry"
                    );
                    last_err = Some(error);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| FetchError::Build("all retries exhausted".into())))
    }

    async fn fetch_once(&self, url: &str) -> Result<FetchResult, FetchError> {
        let start = Instant::now();
        let client = self.pick_client(url);
        let resp = client.get(url).send().await?;
        let response = Response::from_wreq(resp).await?;
        response_to_result(response, start)
    }

    #[instrument(skip(self), fields(url = %url))]
    pub async fn fetch_and_extract(
        &self,
        url: &str,
    ) -> Result<noxa_core::ExtractionResult, FetchError> {
        self.fetch_and_extract_with_options(url, &noxa_core::ExtractionOptions::default())
            .await
    }

    #[instrument(skip(self, options), fields(url = %url))]
    pub async fn fetch_and_extract_with_options(
        &self,
        url: &str,
        options: &noxa_core::ExtractionOptions,
    ) -> Result<noxa_core::ExtractionResult, FetchError> {
        let mut result = self.fetch_and_extract_inner(url, options).await?;
        result.metadata.fetched_at = Some(Utc::now().to_rfc3339());

        if let Some(ref store) = self.store
            && let Err(error) = store.write(url, &result).await
        {
            warn!(url, error = %error, "content store write failed");
        }

        Ok(result)
    }

    async fn fetch_and_extract_inner(
        &self,
        url: &str,
        options: &noxa_core::ExtractionOptions,
    ) -> Result<noxa_core::ExtractionResult, FetchError> {
        if crate::reddit::is_reddit_url(url) {
            let json_url = crate::reddit::json_url(url);
            debug!("reddit detected, fetching {json_url}");

            let client = self.pick_client(url);
            let resp = client
                .get(&json_url)
                .header("User-Agent", crate::reddit::json_api_user_agent())
                .header("Accept", "application/json")
                .send()
                .await?;
            let response = Response::from_wreq(resp).await?;
            if response.is_success() {
                let bytes = response.body();
                if crate::reddit::is_reddit_verify_wall_html(bytes) {
                    return Err(FetchError::BodyDecode(
                        "reddit json endpoint returned verification page".to_string(),
                    ));
                }
                match crate::reddit::parse_reddit_json(bytes, url) {
                    Ok(result) => return Ok(result),
                    Err(error) => {
                        warn!("reddit json fallback failed: {error}, falling back to HTML")
                    }
                }
            }
        }

        let start = Instant::now();
        let client = self.pick_client(url);

        // Apply the same retry policy used by `fetch()` so that transient
        // errors and retryable statuses (429/5xx) are handled here too.
        let delays = [Duration::ZERO, Duration::from_secs(1)];
        let mut response = {
            let mut last_err: Option<FetchError> = None;
            let mut result = None;
            'retry: for (attempt, delay) in delays.iter().enumerate() {
                if attempt > 0 {
                    tokio::time::sleep(*delay).await;
                }
                match client.get(url).send().await {
                    Ok(resp) => match Response::from_wreq(resp).await {
                        Ok(r) => {
                            if is_retryable_status(r.status()) && attempt < delays.len() - 1 {
                                warn!(
                                    url,
                                    status = r.status(),
                                    attempt = attempt + 1,
                                    "retryable status, will retry"
                                );
                                last_err = Some(FetchError::HttpStatus(r.status()));
                                continue 'retry;
                            }
                            result = Some(r);
                            break 'retry;
                        }
                        Err(e) => {
                            if !is_retryable_error(&e) || attempt == delays.len() - 1 {
                                return Err(e);
                            }
                            warn!(url, error = %e, attempt = attempt + 1, "transient error, will retry");
                            last_err = Some(e);
                        }
                    },
                    Err(e) => {
                        let fe = FetchError::Request(e);
                        if !is_retryable_error(&fe) || attempt == delays.len() - 1 {
                            return Err(fe);
                        }
                        warn!(url, error = %fe, attempt = attempt + 1, "transient error, will retry");
                        last_err = Some(fe);
                    }
                }
            }
            result.ok_or_else(|| {
                last_err.unwrap_or_else(|| FetchError::Build("all retries exhausted".into()))
            })?
        };

        if is_challenge_response(&response)
            && let Some(homepage) = extract_homepage(url)
        {
            debug!("challenge detected, warming cookies via {homepage}");
            let _ = client.get(&homepage).send().await;
            let resp = client.get(url).send().await?;
            response = Response::from_wreq(resp).await?;
            debug!("retried after cookie warmup: status={}", response.status());
        }

        let status = response.status();
        let final_url = response.url().to_string();

        if !response.is_success() {
            return Err(FetchError::HttpStatus(status));
        }

        let headers = response.headers().clone();

        if is_pdf_content_type(&headers) {
            debug!(status, "detected PDF response, using pdf extraction");
            let bytes = response.body().to_vec();
            let elapsed = start.elapsed();
            debug!(
                status,
                bytes = bytes.len(),
                elapsed_ms = %elapsed.as_millis(),
                "PDF fetch complete"
            );

            let pdf_mode = self.pdf_mode.clone();
            let final_url_clone = final_url.clone();
            let pdf_result =
                tokio::task::spawn_blocking(move || noxa_pdf::extract_pdf(&bytes, pdf_mode))
                    .await
                    .map_err(|e| FetchError::Build(format!("PDF spawn_blocking panic: {e}")))?
                    .map_err(FetchError::Pdf)?;
            Ok(pdf_to_extraction_result(&pdf_result, &final_url_clone))
        } else if let Some(doc_type) =
            crate::document::is_document_content_type(&headers, &final_url)
        {
            debug!(status, doc_type = ?doc_type, "detected document response, extracting");
            let bytes = response.body().to_vec();
            let elapsed = start.elapsed();
            debug!(
                status,
                bytes = bytes.len(),
                elapsed_ms = %elapsed.as_millis(),
                "document fetch complete"
            );

            let final_url_clone = final_url.clone();
            let mut result = tokio::task::spawn_blocking(move || {
                crate::document::extract_document(&bytes, doc_type)
            })
            .await
            .map_err(|e| FetchError::Build(format!("document spawn_blocking panic: {e}")))??;
            result.metadata.url = Some(final_url_clone);
            Ok(result)
        } else {
            let html = response.into_text();
            let elapsed = start.elapsed();
            debug!(status, elapsed_ms = %elapsed.as_millis(), "fetch complete");

            if crate::linkedin::is_linkedin_post(&final_url) {
                if let Some(result) = crate::linkedin::extract_linkedin_post(&html, &final_url) {
                    debug!("linkedin extraction succeeded");
                    return Ok(result);
                }
                debug!("linkedin extraction failed, falling back to standard");
            }

            Ok(noxa_core::extract_with_options(
                &html,
                Some(&final_url),
                options,
            )?)
        }
    }
}

pub(super) fn response_to_result(
    response: Response,
    start: Instant,
) -> Result<FetchResult, FetchError> {
    let status = response.status();
    let final_url = response.url().to_string();
    let headers = response.headers().clone();
    let html = response.into_text();
    let elapsed = start.elapsed();

    debug!(status, elapsed_ms = %elapsed.as_millis(), "fetch complete");

    Ok(FetchResult {
        html,
        status,
        url: final_url,
        headers,
        elapsed,
    })
}

pub(super) fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504 | 520 | 521 | 522 | 523 | 524)
}

pub(super) fn is_retryable_error(err: &FetchError) -> bool {
    matches!(err, FetchError::Request(_) | FetchError::BodyDecode(_))
}

pub(super) fn is_pdf_content_type(headers: &http::HeaderMap) -> bool {
    headers
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| {
            let mime = ct.split(';').next().unwrap_or("").trim();
            mime.eq_ignore_ascii_case("application/pdf")
        })
        .unwrap_or(false)
}

pub(super) fn is_challenge_response(response: &Response) -> bool {
    let len = response.body().len();
    if len > 15_000 || len == 0 {
        return false;
    }

    let lower = response.text().to_lowercase();
    lower.contains("<title>challenge page</title>")
        || (lower.contains("bazadebezolkohpepadr") && len < 5_000)
}

pub(super) fn extract_homepage(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let host = u.host_str()?;
    match u.port() {
        Some(port) => Some(format!("{}://{}:{}/", u.scheme(), host, port)),
        None => Some(format!("{}://{}/", u.scheme(), host)),
    }
}

pub(super) fn pdf_to_extraction_result(
    pdf: &noxa_pdf::PdfResult,
    url: &str,
) -> noxa_core::ExtractionResult {
    let markdown = noxa_pdf::to_markdown(pdf);
    let word_count = markdown.split_whitespace().count();

    noxa_core::ExtractionResult {
        metadata: noxa_core::Metadata {
            title: pdf.metadata.title.clone(),
            description: pdf.metadata.subject.clone(),
            author: pdf.metadata.author.clone(),
            published_date: None,
            language: None,
            url: Some(url.to_string()),
            site_name: None,
            image: None,
            favicon: None,
            word_count,
            content_hash: None,
            source_type: Some("web".into()),
            file_path: None,
            last_modified: None,
            is_truncated: None,
            technologies: Vec::new(),
            seed_url: None,
            crawl_depth: None,
            search_query: None,
            fetched_at: None,
        },
        content: noxa_core::Content {
            markdown,
            plain_text: pdf.text.clone(),
            links: Vec::new(),
            images: Vec::new(),
            code_blocks: Vec::new(),
            raw_html: None,
        },
        domain_data: None,
        structured_data: vec![],
    }
}
