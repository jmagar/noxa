//! Domain-level operations log in NDJSON format (`.operations.ndjson`).
//!
//! One JSON object per line. O(1) appends via `OpenOptions::append(true)`.
//!
//! **Concurrency notes:**
//! - `O_APPEND` guarantees the seek-to-end + write is atomic at the kernel
//!   level (inode lock), but `write_all()` may issue multiple `write(2)`
//!   syscalls for large buffers. If two writers (threads or processes) append
//!   entries that each exceed the OS page size, partial lines can interleave.
//! - The `OUTPUT_SIZE_LIMIT` truncation keeps most entries small, but this
//!   module does **not** guarantee corruption-free concurrent writes for
//!   entries near that limit.
//! - For the typical single-writer scenario (one CLI / one MCP server) this
//!   is safe.
use std::io::Write;
use std::path::{Component, PathBuf};

use crate::types::{OperationEntry, StoreError};

/// Maximum serialized size of `output` before it is truncated.
const OUTPUT_SIZE_LIMIT: usize = 1024 * 1024; // 1 MiB

/// Filesystem-backed operations log.
///
/// Appends one NDJSON line per operation to `<root>/<domain>/.operations.ndjson`.
#[derive(Debug)]
pub struct FilesystemOperationsLog {
    root: PathBuf,
}

impl FilesystemOperationsLog {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Append one operation entry to the domain log.
    pub async fn append(&self, domain: &str, entry: &OperationEntry) -> Result<(), StoreError> {
        // Validate that `domain` contains only normal path components to prevent
        // path traversal (e.g. `../x` or absolute paths escaping `self.root`).
        if std::path::Path::new(domain)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(StoreError::PathEscape(domain.to_string()));
        }

        let path = self.root.join(domain).join(".operations.ndjson");

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let line = build_line(entry)?;
        let path_clone = path.clone();

        tokio::task::spawn_blocking(move || -> Result<(), StoreError> {
            // Set directory permissions on first creation.
            #[cfg(unix)]
            if let Some(parent) = path_clone.parent() {
                set_dir_permissions(parent)?;
            }

            // Open for append (creates if needed). On Unix this is O_WRONLY|O_CREAT|O_APPEND.
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path_clone)?;

            #[cfg(unix)]
            set_file_permissions_if_new(&path_clone)?;

            file.write_all(line.as_bytes())?;

            // flush() on a regular file is a no-op but documents intent.
            file.flush()?;

            Ok(())
        })
        .await??;

        Ok(())
    }
}

/// Build the NDJSON line for one entry, truncating `output` if it exceeds 1 MiB.
fn build_line(entry: &OperationEntry) -> Result<String, StoreError> {
    let output_bytes = serde_json::to_vec(&entry.output)?;

    let mut line = if output_bytes.len() > OUTPUT_SIZE_LIMIT {
        let mut patched = entry.clone();
        patched.output = serde_json::json!({
            "output_truncated": true,
            "original_size_bytes": output_bytes.len()
        });
        serde_json::to_string(&patched)
    } else {
        serde_json::to_string(entry)
    }?;

    line.push('\n');
    Ok(line)
}

#[cfg(unix)]
fn set_dir_permissions(path: &std::path::Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path)
        && meta.permissions().mode() & 0o777 != 0o700
    {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_file_permissions_if_new(path: &std::path::Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path)
        && meta.permissions().mode() & 0o777 != 0o600
    {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Op;
    use chrono::Utc;

    fn make_entry(op: Op, url: &str) -> OperationEntry {
        OperationEntry {
            op,
            at: Utc::now(),
            url: url.to_string(),
            input: serde_json::json!({}),
            output: serde_json::json!({"result": "ok"}),
        }
    }

    #[test]
    fn test_build_line_contains_op() {
        let entry = make_entry(Op::Map, "https://example.com");
        let line = build_line(&entry).unwrap();
        assert!(line.contains("\"op\":\"map\""), "line was: {line}");
    }

    #[tokio::test]
    async fn test_append_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let log = FilesystemOperationsLog::new(dir.path());
        let entry = make_entry(Op::Map, "https://example.com");
        log.append("example_com", &entry).await.unwrap();

        let log_path = dir.path().join("example_com/.operations.ndjson");
        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("\"op\":\"map\""), "content was: {content}");
    }

    #[tokio::test]
    async fn test_append_multiple_entries_are_separate_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log = FilesystemOperationsLog::new(dir.path());

        for _ in 0..3 {
            log.append("example_com", &make_entry(Op::Brand, "https://example.com"))
                .await
                .unwrap();
        }

        let log_path = dir.path().join("example_com/.operations.ndjson");
        let content = std::fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3, "content was: {content}");
        // Each line must be valid JSON.
        for line in lines {
            assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
        }
    }

    #[tokio::test]
    async fn test_oversized_output_is_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let log = FilesystemOperationsLog::new(dir.path());

        let large_output = serde_json::Value::String("x".repeat(2 * 1024 * 1024));
        let entry = OperationEntry {
            op: Op::Summarize,
            at: Utc::now(),
            url: "https://example.com".to_string(),
            input: serde_json::json!({}),
            output: large_output,
        };
        log.append("example_com", &entry).await.unwrap();

        let log_path = dir.path().join("example_com/.operations.ndjson");
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            content.contains("output_truncated"),
            "content was: {content}"
        );
    }
}
