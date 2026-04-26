use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "trustpilot_reviews",
    label: "Trustpilot Reviews",
    description: "Extract review data from Trustpilot.",
    url_patterns: &["https://www.trustpilot.com/review/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "trustpilot.com") && url.contains("/review/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
