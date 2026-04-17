use std::fs;
use std::path::{Path, PathBuf};

use crate::error::RagError;

/// Returns true iff the path has a supported extension AND exists on disk.
///
/// We check existence because rename events (vim/emacs atomic saves) may fire for
/// temp files that are gone by the time we process them.
///
/// Deferred (no confirmed use case, would add new crate deps): .epub, .mbox
pub(crate) fn is_indexable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext,
        "json"
            | "md"
            | "txt"
            | "log"
            | "rst"
            | "org"
            | "yaml"
            | "yml"
            | "toml"
            | "html"
            | "htm"
            | "ipynb"
            | "pdf"
            | "docx"
            | "odt"
            | "pptx"
            | "jsonl"
            | "xml"
            | "opml"
            | "vtt"
            | "srt"
            | "rss"
            | "atom"
            | "eml"
    ) && path.exists()
}

pub(crate) fn collect_indexable_paths(path: &Path) -> Vec<PathBuf> {
    if is_indexable(path) {
        return vec![path.to_path_buf()];
    }

    if !path.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    collect_indexable_paths_recursive(path, &mut found);
    found.sort();
    found
}

fn collect_indexable_paths_recursive(path: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_symlink() {
            tracing::debug!(path = %entry_path.display(), "skipping symlink");
            continue;
        }
        if is_indexable(&entry_path) {
            found.push(entry_path);
        } else if entry_path.is_dir() {
            collect_indexable_paths_recursive(&entry_path, found);
        }
    }
}

/// Compute the (content_hash, url) key used by the startup delta scan.
///
/// For `.json` ExtractionResult files: peeks at `metadata.url` and `metadata.content_hash`
/// from inside the JSON (fast, avoids full deserialisation of large markdown content).
/// Falls back to file:// URL + SHA-256 of file bytes if the JSON lacks a URL.
///
/// For all other formats: returns file:// URL + SHA-256 of file bytes.
///
/// Returns `None` when the file cannot be read or a file:// URL cannot be constructed.
///
/// Must be called inside `spawn_blocking` — this function reads from disk synchronously.
pub(crate) fn startup_scan_key(path: &Path) -> Option<(String, String)> {
    use sha2::Digest;

    let bytes = std::fs::read(path).ok()?;

    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        #[derive(serde::Deserialize)]
        struct Q {
            metadata: QM,
        }
        #[derive(serde::Deserialize)]
        struct QM {
            url: Option<String>,
            content_hash: Option<String>,
        }
        if let Ok(q) = serde_json::from_slice::<Q>(&bytes) {
            let hash = q
                .metadata
                .content_hash
                .unwrap_or_else(|| format!("{:x}", sha2::Sha256::digest(&bytes)));
            if let Some(url) = q.metadata.url
                && !url.is_empty()
            {
                return Some((hash, url));
            }
        }
    }

    let hash = format!("{:x}", sha2::Sha256::digest(&bytes));
    let url = url::Url::from_file_path(path).ok()?.to_string();
    Some((hash, url))
}

/// Returns the canonical watch root computed once at startup.
pub(crate) async fn canonical_watch_root(watch_dir: &Path) -> Result<PathBuf, RagError> {
    tokio::fs::canonicalize(watch_dir)
        .await
        .map_err(|e| RagError::Generic(format!("canonicalize watch_dir failed: {e}")))
}

pub(crate) fn path_is_within_watch_root(canonical_path: &Path, watch_root: &Path) -> bool {
    canonical_path.starts_with(watch_root)
}

/// Walk up the directory tree from `file_path` to find a `.git/HEAD` file.
///
/// Reads the HEAD ref to extract the branch name: `ref: refs/heads/<branch>`.
/// Returns `None` when not in a git repo, on detached HEAD, or on any I/O error.
pub(crate) fn detect_git_branch(file_path: &Path) -> Option<String> {
    let mut dir = file_path.parent()?;
    loop {
        let git_entry = dir.join(".git");
        if let Some(head) = git_head_path(&git_entry) {
            let content = std::fs::read_to_string(&head).ok()?;
            return content
                .trim()
                .strip_prefix("ref: refs/heads/")
                .map(str::to_string);
        }
        dir = dir.parent()?;
    }
}

fn git_head_path(git_entry: &Path) -> Option<PathBuf> {
    let metadata = std::fs::symlink_metadata(git_entry).ok()?;
    if metadata.is_dir() {
        let head = git_entry.join("HEAD");
        return head.exists().then_some(head);
    }

    if metadata.is_file() {
        let contents = std::fs::read_to_string(git_entry).ok()?;
        let gitdir = contents.trim().strip_prefix("gitdir:")?.trim();
        let head = git_entry.parent()?.join(gitdir).join("HEAD");
        return head.exists().then_some(head);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_watch_root, collect_indexable_paths, detect_git_branch, is_indexable,
        path_is_within_watch_root,
    };
    use std::fs;

    #[test]
    fn collect_indexable_paths_finds_nested_supported_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let nested = root.join("docs/get-started");
        fs::create_dir_all(&nested).expect("create nested dirs");
        fs::write(root.join("top.json"), "{}").expect("write top-level json");
        fs::write(nested.join("guide.json"), "{}").expect("write nested json");
        fs::write(nested.join("ignore.epub"), "nope").expect("write deferred extension");

        let paths = collect_indexable_paths(root);
        let rendered: Vec<String> = paths
            .into_iter()
            .map(|p| p.strip_prefix(root).unwrap().display().to_string())
            .collect();

        assert_eq!(rendered, vec!["docs/get-started/guide.json", "top.json"]);
    }

    #[test]
    fn is_indexable_accepts_all_supported_extensions() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        for ext in &[
            "json", "md", "txt", "log", "rst", "org", "yaml", "yml", "toml", "html", "htm",
            "ipynb", "pdf", "docx", "odt", "pptx", "jsonl", "xml", "opml", "vtt", "srt", "rss",
            "atom", "eml",
        ] {
            let path = root.join(format!("file.{ext}"));
            fs::write(&path, "x").expect("write file");
            assert!(is_indexable(&path), ".{ext} should be indexable");
        }
    }

    #[test]
    fn is_indexable_rejects_deferred_extensions() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        for ext in &["epub", "mbox"] {
            let path = root.join(format!("file.{ext}"));
            fs::write(&path, "x").expect("write file");
            assert!(
                !is_indexable(&path),
                ".{ext} should NOT be indexable (deferred)"
            );
        }
    }

    #[tokio::test]
    async fn canonical_watch_root_resolves_once_up_front() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nested = tmp.path().join("watch/../watch");
        tokio::fs::create_dir_all(&nested)
            .await
            .expect("create watch dir");

        let canonical = canonical_watch_root(&nested)
            .await
            .expect("canonical watch root");
        let expected = tokio::fs::canonicalize(tmp.path().join("watch"))
            .await
            .expect("expected canonical path");

        assert_eq!(canonical, expected);
    }

    #[tokio::test]
    async fn path_is_within_watch_root_rejects_escape() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let watch = tmp.path().join("watch");
        let outside = tmp.path().join("outside");
        tokio::fs::create_dir_all(&watch)
            .await
            .expect("create watch");
        tokio::fs::create_dir_all(&outside)
            .await
            .expect("create outside");

        let watch_root = canonical_watch_root(&watch).await.expect("watch root");
        let outside_file = outside.join("doc.json");
        tokio::fs::write(&outside_file, "{}")
            .await
            .expect("write outside file");
        let canonical_outside = tokio::fs::canonicalize(&outside_file)
            .await
            .expect("canonical outside file");

        assert!(
            !path_is_within_watch_root(&canonical_outside, &watch_root),
            "paths outside the cached watch root should be rejected"
        );
    }

    #[test]
    fn detect_git_branch_returns_none_outside_repo() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join("foo.txt");
        fs::write(&file, "x").expect("write file");
        assert_eq!(detect_git_branch(&file), None);
    }

    #[test]
    fn detect_git_branch_reads_head_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let git_dir = tmp.path().join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature/noxa-rag\n").expect("write HEAD");
        let nested = tmp.path().join("src/foo.rs");
        fs::create_dir_all(nested.parent().unwrap()).expect("create src");
        fs::write(&nested, "x").expect("write file");
        assert_eq!(
            detect_git_branch(&nested),
            Some("feature/noxa-rag".to_string())
        );
    }

    #[test]
    fn detect_git_branch_returns_none_on_detached_head() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let git_dir = tmp.path().join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        fs::write(git_dir.join("HEAD"), "abc123def456\n").expect("write HEAD");
        let file = tmp.path().join("foo.txt");
        fs::write(&file, "x").expect("write file");
        assert_eq!(detect_git_branch(&file), None);
    }
}
