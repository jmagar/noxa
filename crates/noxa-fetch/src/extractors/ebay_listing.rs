use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, product};
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

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let html = client.get_text(url).await?;
    Ok(product::parse_product_page(url, &html, INFO.name))
}
