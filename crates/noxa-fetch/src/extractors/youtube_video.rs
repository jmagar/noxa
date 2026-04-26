use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "youtube_video",
    label: "YouTube Video",
    description: "Extract video metadata from YouTube.",
    url_patterns: &["https://www.youtube.com/watch?v=*", "https://youtu.be/*"],
};

pub fn matches(url: &str) -> bool {
    (host_matches(url, "youtube.com") && url.contains("watch?v=")) || host_matches(url, "youtu.be")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
