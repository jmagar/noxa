use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
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

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let product_url = format!("{}.js", url.trim_end_matches('/'));
    let product = client.get_json(&product_url).await?;
    Ok(json!({
        "url": url,
        "api_url": product_url,
        "id": product.get("id").cloned(),
        "title": product.get("title").cloned(),
        "handle": product.get("handle").cloned(),
        "vendor": product.get("vendor").cloned(),
        "product_type": product.get("product_type").cloned(),
        "tags": product.get("tags").cloned(),
        "variants": product.get("variants").cloned(),
        "images": product.get("images").cloned(),
        "description": product.get("description").cloned(),
    }))
}
