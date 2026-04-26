use serde_json::Value;

use super::{ExtractorInfo, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "ecommerce_product",
    label: "Ecommerce Product",
    description: "Extract generic ecommerce product details.",
    url_patterns: &["*/product/*", "*/products/*"],
};

pub fn matches(url: &str) -> bool {
    url.contains("/product/") || url.contains("/products/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
