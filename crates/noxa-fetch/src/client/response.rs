use std::borrow::Cow;

use crate::client::Response;
use crate::error::FetchError;

impl Response {
    pub(super) async fn from_wreq(resp: wreq::Response) -> Result<Self, FetchError> {
        let status = resp.status().as_u16();
        let url = resp.uri().to_string();
        let headers = resp.headers().clone();
        let body = resp
            .bytes()
            .await
            .map_err(|e| FetchError::BodyDecode(e.to_string()))?;
        Ok(Self {
            status,
            url,
            headers,
            body,
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
