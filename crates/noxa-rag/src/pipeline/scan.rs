use std::fs;
use std::path::{Path, PathBuf};

use crate::error::RagError;

/// Returns true iff the path has a supported indexable extension.
///
/// Unlike [`is_indexable`], this check does NOT require the file to exist on disk.
/// Use this when determining whether a deleted file's path is worth emitting a
/// `Delete` job for — the file is gone so `.exists()` would always return `false`.
pub(crate) fn has_indexable_extension(path: &Path) -> bool {
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
    )
}

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
    has_indexable_extension(path) && path.exists()
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

/// Canonicalizes each watch directory and returns the resulting list.
/// Returns an error if any directory cannot be canonicalized.
pub(crate) async fn canonical_watch_roots(dirs: &[PathBuf]) -> Result<Vec<PathBuf>, RagError> {
    let mut canonical = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let c = tokio::fs::canonicalize(dir)
            .await
            .map_err(|e| RagError::Generic(format!("canonicalize watch_dir failed: {e}")))?;
        canonical.push(c);
    }
    Ok(canonical)
}

/// Returns `true` iff `canonical_path` is under at least one of the given canonical roots.
pub(crate) fn path_is_within_any_watch_root(
    canonical_path: &Path,
    watch_roots: &[PathBuf],
) -> bool {
    watch_roots.iter().any(|root| canonical_path.starts_with(root))
}

/// Walk up the directory tree from `file_path` to find a `.git/HEAD` file.
///
/// Returns `(git_root, branch_name)` or `None` when not in a git repo, on detached
/// HEAD, or on any I/O error. Must be called inside `spawn_blocking` — reads from disk.
pub(crate) fn detect_git_root_and_branch(file_path: &Path) -> Option<(PathBuf, String)> {
    let mut dir = file_path.parent()?;
    loop {
        let git_entry = dir.join(".git");
        if let Some(head) = git_head_path(&git_entry) {
            let content = std::fs::read_to_string(&head).ok()?;
            let branch = content
                .trim()
                .strip_prefix("ref: refs/heads/")
                .map(str::to_string)?;
            return Some((dir.to_path_buf(), branch));
        }
        dir = dir.parent()?;
    }
}

/// Walk up the directory tree from `file_path` to find a `.git/HEAD` file.
///
/// Reads the HEAD ref to extract the branch name: `ref: refs/heads/<branch>`.
/// Returns `None` when not in a git repo, on detached HEAD, or on any I/O error.
pub(crate) fn detect_git_branch(file_path: &Path) -> Option<String> {
    detect_git_root_and_branch(file_path).map(|(_, branch)| branch)
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
        canonical_watch_roots, collect_indexable_paths, detect_git_branch, is_indexable,
        path_is_within_any_watch_root,
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
    async fn canonical_watch_roots_resolves_once_up_front() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nested = tmp.path().join("watch/../watch");
        tokio::fs::create_dir_all(&nested)
            .await
            .expect("create watch dir");

        let roots = canonical_watch_roots(&[nested.to_path_buf()])
            .await
            .expect("canonical watch roots");
        let expected = tokio::fs::canonicalize(tmp.path().join("watch"))
            .await
            .expect("expected canonical path");

        assert_eq!(roots, vec![expected]);
    }

    #[tokio::test]
    async fn canonical_watch_roots_nonexistent_dir_returns_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nonexistent = tmp.path().join("does_not_exist");
        let result = canonical_watch_roots(&[nonexistent]).await;
        assert!(result.is_err(), "nonexistent dir should return error");
    }

    #[tokio::test]
    async fn path_is_within_any_watch_root_rejects_escape() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let watch = tmp.path().join("watch");
        let outside = tmp.path().join("outside");
        tokio::fs::create_dir_all(&watch)
            .await
            .expect("create watch");
        tokio::fs::create_dir_all(&outside)
            .await
            .expect("create outside");

        let watch_roots = canonical_watch_roots(&[watch.to_path_buf()])
            .await
            .expect("watch roots");
        let outside_file = outside.join("doc.json");
        tokio::fs::write(&outside_file, "{}")
            .await
            .expect("write outside file");
        let canonical_outside = tokio::fs::canonicalize(&outside_file)
            .await
            .expect("canonical outside file");

        assert!(
            !path_is_within_any_watch_root(&canonical_outside, &watch_roots),
            "paths outside the cached watch root should be rejected"
        );
    }

    #[tokio::test]
    async fn path_is_within_any_watch_root_matches_first_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root1 = tmp.path().join("root1");
        let root2 = tmp.path().join("root2");
        tokio::fs::create_dir_all(&root1).await.expect("create root1");
        tokio::fs::create_dir_all(&root2).await.expect("create root2");

        let file1 = root1.join("doc.json");
        tokio::fs::write(&file1, "{}").await.expect("write file1");
        let canonical1 = tokio::fs::canonicalize(&file1).await.expect("canonicalize");

        let watch_roots = canonical_watch_roots(&[root1.to_path_buf(), root2.to_path_buf()])
            .await
            .expect("watch roots");

        assert!(path_is_within_any_watch_root(&canonical1, &watch_roots));
    }

    #[tokio::test]
    async fn path_is_within_any_watch_root_matches_second_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root1 = tmp.path().join("root1");
        let root2 = tmp.path().join("root2");
        tokio::fs::create_dir_all(&root1).await.expect("create root1");
        tokio::fs::create_dir_all(&root2).await.expect("create root2");

        let file2 = root2.join("doc.md");
        tokio::fs::write(&file2, "# hi").await.expect("write file2");
        let canonical2 = tokio::fs::canonicalize(&file2).await.expect("canonicalize");

        let watch_roots = canonical_watch_roots(&[root1.to_path_buf(), root2.to_path_buf()])
            .await
            .expect("watch roots");

        assert!(path_is_within_any_watch_root(&canonical2, &watch_roots));
    }

    #[tokio::test]
    async fn path_is_within_any_watch_root_rejects_no_match() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root1 = tmp.path().join("root1");
        let outside = tmp.path().join("outside");
        tokio::fs::create_dir_all(&root1).await.expect("create root1");
        tokio::fs::create_dir_all(&outside).await.expect("create outside");

        let outside_file = outside.join("secret.txt");
        tokio::fs::write(&outside_file, "data")
            .await
            .expect("write outside file");
        let canonical_outside = tokio::fs::canonicalize(&outside_file)
            .await
            .expect("canonicalize");

        let watch_roots = canonical_watch_roots(&[root1.to_path_buf()])
            .await
            .expect("watch roots");

        assert!(!path_is_within_any_watch_root(&canonical_outside, &watch_roots));
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
