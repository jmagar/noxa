use serde_json::Value;

use super::{ExtractorInfo, http::ExtractorHttp, product};
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

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let html = client.get_text(url).await?;
    Ok(product::parse_product_page(url, &html, INFO.name))
}
