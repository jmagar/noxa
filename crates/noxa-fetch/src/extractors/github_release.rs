use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "github_release",
    label: "GitHub Release",
    description: "Extract release metadata from GitHub.",
    url_patterns: &["https://github.com/*/*/releases/tag/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "github.com") && url.contains("/releases/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
