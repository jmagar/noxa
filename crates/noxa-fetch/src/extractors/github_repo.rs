use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "github_repo",
    label: "GitHub Repository",
    description: "Extract repository metadata from GitHub.",
    url_patterns: &["https://github.com/*/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "github.com")
        && !url.contains("/issues/")
        && !url.contains("/pull/")
        && !url.contains("/releases/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
