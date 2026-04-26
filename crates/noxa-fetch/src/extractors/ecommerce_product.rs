use serde_json::Value;

use super::{ExtractorInfo, http::ExtractorHttp, product};
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

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let html = client.get_text(url).await?;
    Ok(product::parse_product_page(url, &html, INFO.name))
}
