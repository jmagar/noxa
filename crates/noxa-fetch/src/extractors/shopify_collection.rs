use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "shopify_collection",
    label: "Shopify Collection",
    description: "Extract collection metadata from Shopify storefronts.",
    url_patterns: &["*/collections/*"],
};

pub fn matches(url: &str) -> bool {
    url.contains("/collections/")
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let api_url = format!("{}/products.json", url.trim_end_matches('/'));
    let collection = client.get_json(&api_url).await?;
    Ok(json!({
        "url": url,
        "api_url": api_url,
        "products": collection.get("products").cloned().unwrap_or_else(|| json!([])),
    }))
}
