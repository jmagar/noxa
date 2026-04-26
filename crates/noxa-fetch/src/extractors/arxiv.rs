use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "arxiv",
    label: "arXiv Paper",
    description: "Extract paper metadata from arXiv pages.",
    url_patterns: &["https://arxiv.org/abs/*", "https://arxiv.org/pdf/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "arxiv.org") && (url.contains("/abs/") || url.contains("/pdf/"))
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
