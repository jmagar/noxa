use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "reddit",
    label: "Reddit Post",
    description: "Extract Reddit post and comment data.",
    url_patterns: &["https://www.reddit.com/r/*/comments/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "reddit.com") && url.contains("/comments/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
