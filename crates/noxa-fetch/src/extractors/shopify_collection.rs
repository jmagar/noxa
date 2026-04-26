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
    collection_api_url(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let api_url = collection_api_url(url).ok_or_else(|| {
        FetchError::Build(format!(
            "shopify_collection: cannot parse collection URL '{url}'"
        ))
    })?;
    let collection = client.get_json(&api_url).await?;
    Ok(json!({
        "url": url,
        "api_url": api_url,
        "products": collection.get("products").cloned().unwrap_or_else(|| json!([])),
    }))
}

fn collection_api_url(url: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    let has_collection_path = parsed.path_segments().is_some_and(|mut segments| {
        segments.next() == Some("collections") && segments.next().is_some()
    });
    if !has_collection_path {
        return None;
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(&format!("{path}/products.json"));
    Some(parsed.to_string())
}
