use serde_json::Value;

use super::{ExtractorInfo, host_matches, http::ExtractorHttp, stub_error};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "huggingface_model",
    label: "Hugging Face Model",
    description: "Extract model metadata from Hugging Face.",
    url_patterns: &["https://huggingface.co/*/*"],
};

pub fn matches(url: &str) -> bool {
    host_matches(url, "huggingface.co") && !url.contains("/datasets/")
}

pub async fn extract(_client: &dyn ExtractorHttp, _url: &str) -> Result<Value, FetchError> {
    Err(stub_error(INFO.name))
}
