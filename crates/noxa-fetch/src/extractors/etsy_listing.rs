use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "etsy_listing",
    label: "Etsy Listing",
    description: "Extract listing details from Etsy.",
    url_patterns: &["https://www.etsy.com/listing/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "etsy.com") && url.contains("/listing/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
