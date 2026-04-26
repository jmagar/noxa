use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "reddit",
    label: "Reddit Post",
    description: "Extract Reddit post and comment data.",
    url_patterns: &["https://www.reddit.com/r/*/comments/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "reddit.com") && url.contains("/comments/")
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let json_url = crate::reddit::json_url(url);
    let body = client.get_text(&json_url).await?;
    let extraction =
        crate::reddit::parse_reddit_json(body.as_bytes(), url).map_err(FetchError::BodyDecode)?;

    serde_json::to_value(extraction).map_err(|error| FetchError::BodyDecode(error.to_string()))
}
