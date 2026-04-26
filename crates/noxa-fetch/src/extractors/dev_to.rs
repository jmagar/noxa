use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "dev_to",
    label: "dev.to Article",
    description: "Extract article metadata and content from dev.to.",
    url_patterns: &["https://dev.to/*/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "dev.to")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
