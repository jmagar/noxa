use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NoxaMcpError {
    #[error("{0}")]
    Message(String),

    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("URL validation failed for {url}: {reason}")]
    UrlValidation { url: String, reason: String },

    #[error("failed to create directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to initialize content store: {0}")]
    ContentStoreInit(#[source] noxa_store::StoreError),

    #[error("failed to access content store: {0}")]
    ContentStore(#[from] noxa_store::StoreError),

    #[error("failed to build fetch client: {0}")]
    FetchClientInit(#[source] noxa_fetch::FetchError),

    #[error("failed to load proxy pool from {path}: {source}")]
    ProxyPool {
        path: PathBuf,
        #[source]
        source: noxa_fetch::FetchError,
    },

    #[error("fetch failed: {0}")]
    Fetch(#[from] noxa_fetch::FetchError),

    #[error("extraction failed: {0}")]
    Extract(#[from] noxa_core::ExtractError),

    #[error("failed to build cloud client: {0}")]
    CloudClientInit(#[source] reqwest::Error),

    #[error("cloud API error: {0}")]
    Cloud(String),

    #[error("LLM operation failed: {0}")]
    Llm(String),

    #[error("failed to serialize {context}: {source}")]
    Serialization {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid SEARXNG_URL: {0}")]
    InvalidSearxngUrl(String),

    #[error("failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse {path}: {source}")]
    ParseFile {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to write {path}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl NoxaMcpError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn invalid_parameter(message: impl Into<String>) -> Self {
        Self::InvalidParameter(message.into())
    }

    pub fn llm(message: impl Into<String>) -> Self {
        Self::Llm(message.into())
    }

    pub fn cloud(message: impl Into<String>) -> Self {
        Self::Cloud(message.into())
    }
}
