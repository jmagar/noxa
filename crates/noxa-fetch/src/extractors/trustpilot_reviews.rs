use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, product};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "trustpilot_reviews",
    label: "Trustpilot Reviews",
    description: "Extract review data from Trustpilot.",
    url_patterns: &["https://www.trustpilot.com/review/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "trustpilot.com") && url.contains("/review/")
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let html = client.get_text(url).await?;
    Ok(product::parse_trustpilot_page(url, &html))
}
