use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "instagram_post",
    label: "Instagram Post",
    description: "Extract post metadata from Instagram.",
    url_patterns: &["https://www.instagram.com/p/*", "https://www.instagram.com/reel/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "instagram.com") && (url.contains("/p/") || url.contains("/reel/"))
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
