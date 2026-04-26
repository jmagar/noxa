use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, product};
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

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let html = client.get_text(url).await?;
    Ok(product::parse_product_page(url, &html, INFO.name))
}
