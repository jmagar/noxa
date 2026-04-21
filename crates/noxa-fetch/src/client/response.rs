use std::borrow::Cow;

use crate::client::Response;
use crate::error::FetchError;

const MAX_HTML_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const MAX_JSON_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const MAX_DOCUMENT_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_PDF_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

impl Response {
    pub(super) async fn from_wreq(mut resp: wreq::Response) -> Result<Self, FetchError> {
        let status = resp.status().as_u16();
        let url = resp.uri().to_string();
        let headers = resp.headers().clone();
        let limit = response_body_limit(&headers, &url);

        if resp.content_length().is_some_and(|len| len > limit as u64) {
            return Err(FetchError::Limit(format!(
                "response body too large for {}: {} > {limit} bytes",
                response_kind(&headers, &url),
                resp.content_length().unwrap_or_default()
            )));
        }

        let mut body = bytes::BytesMut::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| FetchError::BodyDecode(e.to_string()))?
        {
            if body.len() + chunk.len() > limit {
                return Err(FetchError::Limit(format!(
                    "response body too large for {}: {} > {limit} bytes",
                    response_kind(&headers, &url),
                    body.len() + chunk.len()
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Self {
            status,
            url,
            headers,
            body: body.freeze(),
        })
    }

    pub(super) fn status(&self) -> u16 {
        self.status
    }

    pub(super) fn url(&self) -> &str {
        &self.url
    }

    pub(super) fn headers(&self) -> &http::HeaderMap {
        &self.headers
    }

    pub(super) fn body(&self) -> &[u8] {
        &self.body
    }

    pub(super) fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub(super) fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }

    pub(super) fn into_text(self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn response_body_limit(headers: &http::HeaderMap, url: &str) -> usize {
    if is_pdf_content_type(headers) {
        MAX_PDF_RESPONSE_BYTES
    } else if crate::document::is_document_content_type(headers, url).is_some() {
        MAX_DOCUMENT_RESPONSE_BYTES
    } else if is_json_content_type(headers, url) {
        MAX_JSON_RESPONSE_BYTES
    } else {
        MAX_HTML_RESPONSE_BYTES
    }
}

fn response_kind(headers: &http::HeaderMap, url: &str) -> &'static str {
    if is_pdf_content_type(headers) {
        "pdf"
    } else if crate::document::is_document_content_type(headers, url).is_some() {
        "document"
    } else if is_json_content_type(headers, url) {
        "json"
    } else {
        "html"
    }
}

fn is_pdf_content_type(headers: &http::HeaderMap) -> bool {
    headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("application/pdf"))
}

fn is_json_content_type(headers: &http::HeaderMap, url: &str) -> bool {
    headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("json"))
        || url.ends_with(".json")
}
