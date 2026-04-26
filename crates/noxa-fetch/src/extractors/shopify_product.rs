use serde_json::Value;

use super::{ExtractorInfo, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "shopify_product",
    label: "Shopify Product",
    description: "Extract product metadata from Shopify storefronts.",
    url_patterns: &["*/products/*"],
};

pub fn matches(url: &str) -> bool {
    url.contains("/products/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
