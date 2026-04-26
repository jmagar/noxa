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
    product_api_url(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let product_url = product_api_url(url).ok_or_else(|| {
        FetchError::Build(format!("shopify_product: cannot parse product URL '{url}'"))
    })?;
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

fn product_api_url(url: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    let has_product_path = parsed.path_segments().is_some_and(|mut segments| {
        segments.next() == Some("products") && segments.next().is_some()
    });
    if !has_product_path {
        return None;
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(&format!("{path}.js"));
    Some(parsed.to_string())
}
