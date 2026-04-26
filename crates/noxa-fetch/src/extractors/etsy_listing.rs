use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, product};
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

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let html = client.get_text(url).await?;
    Ok(product::parse_product_page(url, &html, INFO.name))
}
