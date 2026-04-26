use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "pypi",
    label: "PyPI Package",
    description: "Extract package metadata from PyPI.",
    url_patterns: &["https://pypi.org/project/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "pypi.org") && url.contains("/project/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
