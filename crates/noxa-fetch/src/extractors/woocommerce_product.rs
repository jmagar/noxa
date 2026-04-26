use serde_json::Value;

use super::{ExtractorInfo, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "woocommerce_product",
    label: "WooCommerce Product",
    description: "Extract product metadata from WooCommerce storefronts.",
    url_patterns: &["*/product/*"],
};

pub fn matches(url: &str) -> bool {
    url.contains("/product/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
