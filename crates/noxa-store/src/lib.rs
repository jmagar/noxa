//! Filesystem persistence layer for noxa.
//!
//! Provides:
//! - [`FilesystemContentStore`] — per-URL `.md` + `.json` sidecar storage
//! - [`FilesystemOperationsLog`] — domain-level `.operations.ndjson` append log
//! - Path utilities: [`url_to_store_path`], [`content_store_root`], [`domain_from_url`]
//! - Types: [`Op`], [`OperationEntry`], [`StoreResult`]
pub mod content_store;
pub mod operations_log;
pub mod paths;
pub mod types;
pub mod url_validation;

pub use content_store::{ChangelogEntry, FilesystemContentStore, Sidecar};
pub use operations_log::FilesystemOperationsLog;
pub use paths::{content_store_root, domain_from_url, url_to_store_path};
pub use types::{Op, OperationEntry, StoreError, StoreResult};
pub use url_validation::{
    is_private_or_reserved_ip, parse_http_url, validate_public_http_url,
    validate_public_http_url_with_resolver,
};
