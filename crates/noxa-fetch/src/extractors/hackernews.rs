use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "hackernews",
    label: "Hacker News Item",
    description: "Extract Hacker News story or comment metadata.",
    url_patterns: &["https://news.ycombinator.com/item?id=*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "news.ycombinator.com") && url.contains("item?id=")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
