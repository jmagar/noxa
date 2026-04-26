use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "linkedin_post",
    label: "LinkedIn Post",
    description: "Extract post metadata from LinkedIn.",
    url_patterns: &["https://www.linkedin.com/posts/*", "https://www.linkedin.com/feed/update/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "linkedin.com") && (url.contains("/posts/") || url.contains("/feed/update/"))
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
