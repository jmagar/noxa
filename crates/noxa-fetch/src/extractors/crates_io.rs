use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "crates_io",
    label: "crates.io Crate",
    description: "Extract package metadata from crates.io.",
    url_patterns: &["https://crates.io/crates/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "crates.io") && url.contains("/crates/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
