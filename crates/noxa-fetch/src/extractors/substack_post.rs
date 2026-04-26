use serde_json::Value;

use super::{ExtractorInfo, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "substack_post",
    label: "Substack Post",
    description: "Extract post metadata from Substack publications.",
    url_patterns: &["https://*.substack.com/p/*", "*/p/*"],
};

pub fn matches(url: &str) -> bool {
    url.contains("/p/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
