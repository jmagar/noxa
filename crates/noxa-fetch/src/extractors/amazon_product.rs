use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "amazon_product",
    label: "Amazon Product",
    description: "Extract product details from Amazon product pages.",
    url_patterns: &["https://*.amazon.*/dp/*", "https://*.amazon.*/gp/product/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "amazon.com") && (url.contains("/dp/") || url.contains("/gp/product/"))
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
