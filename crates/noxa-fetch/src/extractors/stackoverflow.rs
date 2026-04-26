use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "stackoverflow",
    label: "Stack Overflow Question",
    description: "Extract question metadata from Stack Overflow.",
    url_patterns: &["https://stackoverflow.com/questions/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "stackoverflow.com") && url.contains("/questions/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
