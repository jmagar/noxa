use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "docker_hub",
    label: "Docker Hub Repository",
    description: "Extract repository metadata from Docker Hub.",
    url_patterns: &["https://hub.docker.com/r/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "hub.docker.com") && url.contains("/r/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
