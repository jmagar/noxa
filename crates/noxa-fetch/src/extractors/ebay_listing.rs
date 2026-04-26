use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "ebay_listing",
    label: "eBay Listing",
    description: "Extract listing details from eBay.",
    url_patterns: &["https://*.ebay.*/itm/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "ebay.com") && url.contains("/itm/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
