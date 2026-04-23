use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use dashmap::DashMap;
use tokenizers::Tokenizer;
use tokio::io::AsyncReadExt;

use crate::config::RagConfig;
use crate::embed::DynEmbedProvider;
use crate::error::RagError;
use crate::store::DynVectorStore;
use crate::types::Point;

use super::parse;
use super::scan;
use super::{DeleteJob, IndexJob, JobStats, SessionCounters};

/// Maximum size of the failed-jobs log before it is rotated (10 MiB).
const FAILED_JOBS_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;

/// Validate that `url` is a permitted indexing target.
///
/// - `http`/`https`: delegates to `noxa_store::url_validation::validate_public_http_url`, which
///   resolves hostnames via DNS and rejects all private/reserved IP ranges. This closes the
///   SSRF gap where hostname-based internal addresses (e.g. `qdrant.internal`) bypass a
///   numeric-IP-only check.
/// - `file`: allowed only for local paths (no remote host).
/// - All other schemes: rejected.
pub(crate) async fn validate_url_scheme(url: &str) -> Result<(), RagError> {
    if url.is_empty() {
        return Err(RagError::Generic(
            "extraction result has no URL".to_string(),
        ));
    }
    let parsed =
        url::Url::parse(url).map_err(|e| RagError::Generic(format!("invalid URL {url:?}: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {
            noxa_store::url_validation::validate_public_http_url(url)
                .await
                .map_err(|e| RagError::Generic(format!("URL {url:?} blocked: {e}")))?;
        }
        "file" => match parsed.host_str() {
            None | Some("") | Some("localhost") => {}
            Some(host) => {
                return Err(RagError::Generic(format!(
                    "file:// URL with remote host {host:?} is not allowed (only local paths)"
                )));
            }
        },
        other => {
            return Err(RagError::Generic(format!(
                "URL scheme {other:?} is not allowed (only http/https/file)"
            )));
        }
    }

    Ok(())
}

/// Append one NDJSON failure entry to the failed-jobs log.
///
/// Performs size-based rotation under `log_lock`: if the log exceeds
/// `FAILED_JOBS_LOG_MAX_BYTES`, the existing file is renamed to `<path>.1`
/// (overwriting any prior `.1` backup) and a fresh log is started.
///
/// The entire check-rotate-append sequence is serialised by `log_lock` so
/// concurrent workers cannot race on the rename.
async fn append_failed_job(
    path: &Path,
    error: &impl std::fmt::Display,
    config: &RagConfig,
    counters: &Arc<SessionCounters>,
    log_lock: &Arc<tokio::sync::Mutex<()>>,
) {
    // Increment the parse-failure counter regardless of whether a log path is
    // configured — this ensures the heartbeat metric is always accurate.
    counters.parse_failures.fetch_add(1, Ordering::Relaxed);

    let Some(ref log_path) = config.pipeline.failed_jobs_log else {
        return;
    };

    let entry = serde_json::json!({
        "path": path.to_string_lossy(),
        "error": error.to_string(),
        "ts": chrono::Utc::now().to_rfc3339(),
    });
    let line = format!("{entry}\n");

    // Serialise the check-rotate-append sequence across concurrent workers.
    let _guard = log_lock.lock().await;

    // Rotate if the log has grown past the cap.
    if let Ok(meta) = tokio::fs::metadata(log_path).await {
        if meta.len() >= FAILED_JOBS_LOG_MAX_BYTES {
            let mut rotated = log_path.to_path_buf();
            rotated.as_mut_os_string().push(".1");
            // Remove any existing backup first; rename fails on Windows if the
            // destination already exists.
            let _ = tokio::fs::remove_file(&rotated).await;
            if let Err(e) = tokio::fs::rename(log_path, &rotated).await {
                tracing::warn!(
                    log = %log_path.display(),
                    error = %e,
                    "failed to rotate failed-jobs log; continuing with existing file"
                );
            } else {
                tracing::info!(
                    log = %log_path.display(),
                    max_bytes = FAILED_JOBS_LOG_MAX_BYTES,
                    "rotated failed-jobs log"
                );
            }
        }
    }

    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await
    {
        use tokio::io::AsyncWriteExt;
        let _ = file.write_all(line.as_bytes()).await;
    }
}

pub(crate) async fn process_job(
    job: IndexJob,
    embed: &DynEmbedProvider,
    store: &DynVectorStore,
    tokenizer: &Arc<Tokenizer>,
    config: &RagConfig,
    url_locks: &Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    git_branch_cache: &Arc<DashMap<PathBuf, Option<String>>>,
    watch_roots: &[PathBuf],
    counters: &Arc<SessionCounters>,
    failed_jobs_log_lock: &Arc<tokio::sync::Mutex<()>>,
) -> Result<JobStats, RagError> {
    let job_start = std::time::Instant::now();

    let t0 = std::time::Instant::now();
    let canonical = tokio::fs::canonicalize(&job.path).await.map_err(|e| {
        RagError::Generic(format!(
            "canonicalize failed for {}: {e}",
            job.path.display()
        ))
    })?;
    if !scan::path_is_within_any_watch_root(&canonical, watch_roots) {
        tracing::warn!(
            path = %job.path.display(),
            "path outside watch_dir — skipping (potential TOCTOU attack)"
        );
        return Ok(JobStats {
            chunks: 0,
            embed_ms: 0,
            upsert_ms: 0,
        });
    }
    let mut file = tokio::fs::File::open(&canonical).await?;
    let file_meta = file.metadata().await?;
    let size = file_meta.len();

    const MAX_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024;
    if size > MAX_FILE_SIZE_BYTES {
        tracing::warn!(path = ?job.path, size, "file too large (>50MB), skipping");
        return Ok(JobStats {
            chunks: 0,
            embed_ms: 0,
            upsert_ms: 0,
        });
    }

    let mut file_bytes: Vec<u8> = Vec::with_capacity(size as usize);
    file.read_to_end(&mut file_bytes).await?;
    let parse_ms = t0.elapsed().as_millis() as u64;

    let parsed = match parse::parse_file(&job.path, file_bytes).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(path = ?job.path, error = %e, "parse failed, skipping");
            append_failed_job(&job.path, &e, config, counters, failed_jobs_log_lock).await;
            return Ok(JobStats {
                chunks: 0,
                embed_ms: 0,
                upsert_ms: 0,
            });
        }
    };
    let mut result = parsed.extraction;

    if result.metadata.file_path.is_none() {
        result.metadata.file_path = Some(canonical.to_string_lossy().into_owned());
    }
    if result.metadata.last_modified.is_none()
        && let Ok(mtime) = file_meta.modified()
    {
        result.metadata.last_modified =
            Some(chrono::DateTime::<chrono::Utc>::from(mtime).to_rfc3339());
    }
    let git_branch = {
        let path = job.path.clone();
        let cache = git_branch_cache.clone();
        tokio::task::spawn_blocking(move || {
            // Walk up to find the git root first so we can use it as a stable cache key.
            if let Some((git_root, branch)) = scan::detect_git_root_and_branch(&path) {
                cache
                    .entry(git_root)
                    .or_insert_with(|| Some(branch.clone()))
                    .clone()
            } else {
                None
            }
        })
        .await
        .ok()
        .flatten()
    };

    let raw_url = result.metadata.url.clone().unwrap_or_else(|| {
        url::Url::from_file_path(&canonical)
            .map(|url| url.to_string())
            .unwrap_or_else(|_| canonical.to_string_lossy().into_owned())
    });
    if let Err(e) = validate_url_scheme(&raw_url).await {
        tracing::warn!(path = ?job.path, error = %e, "url validation failed, skipping");
        return Ok(JobStats {
            chunks: 0,
            embed_ms: 0,
            upsert_ms: 0,
        });
    }
    let url = crate::url_util::normalize_url(&raw_url);

    let t1 = std::time::Instant::now();
    // KNOWLEDGE: chunker tokenization is CPU-bound (HuggingFace BPE). Wrap in
    // spawn_blocking to avoid blocking async Tokio worker threads — same pattern
    // as parse/mod.rs for PDF/DOCX. See bead noxa-3fi.2.
    let chunks = {
        let result_clone = result.clone();
        let config_chunker = config.chunker.clone();
        let tokenizer = Arc::clone(tokenizer);
        tokio::task::spawn_blocking(move || {
            crate::chunker::chunk(&result_clone, &config_chunker, &tokenizer)
        })
        .await
        .map_err(|e| RagError::Generic(format!("chunker panicked: {e}")))?
    };
    if chunks.is_empty() {
        tracing::info!(url = %url, "no indexable content after chunking");
        return Ok(JobStats {
            chunks: 0,
            embed_ms: 0,
            upsert_ms: 0,
        });
    }
    let chunk_ms = t1.elapsed().as_millis() as u64;

    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let total_tokens: u64 = chunks.iter().map(|c| c.token_estimate as u64).sum();
    let t2 = std::time::Instant::now();
    let vectors = embed.embed(&texts).await?;
    let embed_ms = t2.elapsed().as_millis() as u64;
    let embed_tokens_per_sec = if embed_ms > 0 {
        total_tokens * 1_000 / embed_ms
    } else {
        0
    };

    if vectors.len() != chunks.len() {
        return Err(RagError::Embed {
            message: format!(
                "embed returned {} vectors for {} chunks",
                vectors.len(),
                chunks.len()
            ),
            status: None,
        });
    }

    let n_chunks = chunks.len();
    let points: Vec<Point> = chunks
        .iter()
        .zip(vectors.iter())
        .enumerate()
        .map(|(i, (chunk, vector))| {
            let id = uuid::Uuid::new_v5(
                &config.uuid_namespace,
                format!("{}#chunk{}", url, i).as_bytes(),
            );
            Point {
                id,
                vector: vector.clone(),
                payload: parse::build_point_payload(
                    chunk,
                    &result,
                    git_branch.clone(),
                    &parsed.provenance,
                    &url,
                ),
            }
        })
        .collect();

    let url_lock = url_locks
        .entry(url.clone())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _guard = url_lock.lock().await;

    let new_ids: Vec<uuid::Uuid> = points.iter().map(|p| p.id).collect();
    let t3 = std::time::Instant::now();
    let store_result: Result<u64, RagError> = async {
        let t4 = std::time::Instant::now();
        let upserted = store.upsert(points).await.map_err(|e| {
            tracing::error!(url = %url, error = %e, "upsert failed");
            e
        })?;
        let upsert_ms = t4.elapsed().as_millis() as u64;

        store
            .delete_stale_by_url(&url, &new_ids)
            .await
            .map_err(|e| {
                tracing::warn!(
                    url = %url,
                    error = %e,
                    "stale cleanup failed after upsert — duplicate chunks until next file event"
                );
                e
            })?;
        let delete_ms = t3.elapsed().as_millis() as u64 - upsert_ms;

        tracing::info!(
            url = %url,
            chunks = upserted,
            embed_tokens = total_tokens,
            embed_tokens_per_sec,
            parse_ms,
            chunk_ms,
            embed_ms,
            delete_ms,
            upsert_ms,
            total_ms = job_start.elapsed().as_millis() as u64,
            "indexed"
        );

        Ok(upsert_ms)
    }
    .await;

    drop(_guard);
    drop(url_lock);
    url_locks.remove_if(&url, |_, v| Arc::strong_count(v) == 1);

    let upsert_ms = store_result?;

    Ok(JobStats {
        chunks: n_chunks,
        embed_ms,
        upsert_ms,
    })
}

/// Remove all Qdrant chunks for a file that was deleted from disk.
///
/// The file no longer exists so we cannot canonicalize its path — we derive a
/// `file://` URL directly from the raw watcher-reported path instead.
pub(crate) async fn process_delete_job(job: DeleteJob, store: &DynVectorStore) {
    let url = url::Url::from_file_path(&job.path)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| job.path.to_string_lossy().into_owned());
    let url = crate::url_util::normalize_url(&url);
    match store.delete_by_url(&url).await {
        Ok(()) => tracing::info!(url = %url, "deleted chunks for removed file"),
        Err(e) => tracing::warn!(url = %url, error = %e, "failed to delete chunks for removed file"),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_url_scheme;

    #[tokio::test]
    async fn validate_url_scheme_accepts_file_local_path() {
        assert!(validate_url_scheme("file:///tmp/foo.md").await.is_ok());
    }

    #[tokio::test]
    async fn validate_url_scheme_accepts_file_localhost_host() {
        assert!(validate_url_scheme("file://localhost/tmp/foo.md").await.is_ok());
    }

    #[tokio::test]
    async fn validate_url_scheme_rejects_file_with_remote_host() {
        assert!(
            validate_url_scheme("file://remoteserver/share/doc.txt")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn validate_url_scheme_rejects_ftp() {
        assert!(
            validate_url_scheme("ftp://example.com/file.txt")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn validate_url_scheme_rejects_empty_url() {
        assert!(validate_url_scheme("").await.is_err());
    }
}
