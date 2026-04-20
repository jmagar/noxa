//! Per-URL content store: manages `.md` and `.json` sidecar files under a configurable root.
//!
//! The `.json` sidecar next to each `.md` now uses a versioned envelope format
//! (`Sidecar`) that keeps the full `ExtractionResult` in `current` plus a
//! `changelog` of every content change over time.
mod enumerate;
mod manifest;
mod migrate;
mod permissions;
mod write;

pub use enumerate::{DomainEntry, DomainUrlsResult, StoredDoc};
use manifest::ManifestCacheHandle;
use migrate::parse_sidecar_or_migrate;

use std::path::{Component, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::paths::{content_store_root, try_url_to_store_path};
use crate::types::StoreError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub at: DateTime<Utc>,
    pub word_count: usize,
    pub diff: Option<noxa_core::ContentDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sidecar {
    pub schema_version: u32,
    pub url: String,
    pub first_seen: DateTime<Utc>,
    pub last_fetched: DateTime<Utc>,
    pub fetch_count: u64,
    pub current: noxa_core::ExtractionResult,
    pub changelog: Vec<ChangelogEntry>,
}

#[derive(Debug, Clone)]
pub struct FilesystemContentStore {
    root: PathBuf,
    canonical_root: std::sync::Arc<std::sync::OnceLock<PathBuf>>,
    pub max_content_bytes: Option<usize>,
    pub max_changelog_entries: Option<usize>,
    /// Lazy-populated, TTL-backed manifest cache.  Shared across clones.
    pub(crate) manifest_cache: ManifestCacheHandle,
}

impl FilesystemContentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let canonical_root = std::sync::Arc::new(std::sync::OnceLock::new());
        if let Ok(resolved) = std::fs::canonicalize(&root) {
            let _ = canonical_root.set(resolved);
        }
        Self {
            root,
            canonical_root,
            max_content_bytes: Some(2 * 1024 * 1024),
            max_changelog_entries: Some(100),
            manifest_cache: ManifestCacheHandle::new(),
        }
    }

    pub fn open() -> Result<Self, StoreError> {
        let root = content_store_root(None)?;
        Ok(Self::new(root))
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn get_canonical_root(&self) -> Result<PathBuf, StoreError> {
        if let Some(root) = self.canonical_root.get() {
            return Ok(root.clone());
        }
        let resolved = canonicalize_with_missing_components(&self.root)?;
        let _ = self.canonical_root.set(resolved.clone());
        Ok(resolved)
    }

    fn resolve_path(&self, url: &str) -> Result<PathBuf, StoreError> {
        let rel = try_url_to_store_path(url)?;
        if rel
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(StoreError::PathEscape(url.to_string()));
        }

        let base = self.root.join(&rel);
        if !base.starts_with(&self.root) {
            return Err(StoreError::PathEscape(url.to_string()));
        }

        let canonical_root = self.get_canonical_root()?;
        let parent = base.parent().unwrap_or(&base);
        let resolved_parent = canonicalize_with_missing_components(parent)?;
        if !resolved_parent.starts_with(&canonical_root) {
            return Err(StoreError::PathEscape(url.to_string()));
        }

        let file_name = base
            .file_name()
            .ok_or_else(|| StoreError::PathEscape(url.to_string()))?;
        Ok(resolved_parent.join(file_name))
    }

    pub async fn read(&self, url: &str) -> Result<Option<noxa_core::ExtractionResult>, StoreError> {
        let base = match self.resolve_path(url) {
            Ok(path) => path,
            Err(StoreError::PathEscape(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let json_path = base.with_extension("json");
        match tokio::fs::read_to_string(&json_path).await {
            Ok(contents) => {
                let json_path_for_error = json_path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    migrate::parse_sidecar_or_legacy(&contents).map(|sidecar| sidecar.current)
                })
                .await?
                .map_err(|source| StoreError::CorruptSidecar {
                    path: json_path_for_error,
                    source,
                })?;
                Ok(Some(result))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn read_sidecar(&self, url: &str) -> Result<Option<Sidecar>, StoreError> {
        let base = match self.resolve_path(url) {
            Ok(path) => path,
            Err(StoreError::PathEscape(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let json_path = base.with_extension("json");
        match tokio::fs::read_to_string(&json_path).await {
            Ok(contents) => {
                let mtime = tokio::fs::metadata(&json_path)
                    .await
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(DateTime::<Utc>::from)
                    .unwrap_or_else(Utc::now);
                let json_path_for_error = json_path.clone();
                let result =
                    tokio::task::spawn_blocking(move || parse_sidecar_or_migrate(&contents, mtime))
                        .await?
                        .map_err(|source| StoreError::CorruptSidecar {
                            path: json_path_for_error,
                            source,
                        })?;
                Ok(Some(result))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

fn canonicalize_with_missing_components(path: &std::path::Path) -> Result<PathBuf, StoreError> {
    let mut existing_ancestor = path.to_path_buf();
    let mut suffix = PathBuf::new();

    while !existing_ancestor.exists() {
        if let Some(name) = existing_ancestor.file_name() {
            suffix = PathBuf::from(name).join(&suffix);
        }
        match existing_ancestor.parent() {
            Some(parent) => existing_ancestor = parent.to_path_buf(),
            None => break,
        }
    }

    let canonical_parent =
        std::fs::canonicalize(&existing_ancestor).map_err(|source| StoreError::IoPath {
            source,
            path: existing_ancestor.clone(),
        })?;
    Ok(canonical_parent.join(&suffix))
}

#[cfg(test)]
mod tests;
