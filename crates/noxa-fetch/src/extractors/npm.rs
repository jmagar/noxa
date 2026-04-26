use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "npm",
    label: "npm Package",
    description: "Extract package metadata from npm.",
    url_patterns: &["https://www.npmjs.com/package/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "npmjs.com") && url.contains("/package/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
