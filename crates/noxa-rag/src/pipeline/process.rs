use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::io::AsyncReadExt;

use crate::error::RagError;
use crate::store::DynVectorStore;
use crate::types::Point;

use super::parse;
use super::scan;
use super::{DeleteJob, IndexJob, JobStats, WorkerContext};

/// Walk up from `path`'s parent checking each ancestor directory against `cache`.
///
/// Returns `Some(branch)` on a cache hit (where the inner `Option<String>` is the stored
/// branch value — `None` means "not in a git repo"), or `None` when no ancestor is cached.
///
/// O(depth) DashMap lookups; typical repo depth ≤ 5, so this avoids a `spawn_blocking`
/// dispatch for every file after the first in each git repository.
fn find_cached_branch(
    path: &Path,
    cache: &DashMap<PathBuf, Option<String>>,
) -> Option<Option<String>> {
    let mut dir = path.parent()?;
    loop {
        if let Some(entry) = cache.get(dir) {
            return Some(entry.clone());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return None,
        }
    }
}

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
        return Err(RagError::UrlValidation(
            "extraction result has no URL".to_string(),
        ));
    }
    let parsed = url::Url::parse(url)
        .map_err(|e| RagError::UrlValidation(format!("invalid URL {url:?}: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {
            noxa_store::url_validation::validate_public_http_url(url)
                .await
                .map_err(|e| RagError::UrlValidation(format!("URL {url:?} blocked: {e}")))?;
        }
        "file" => match parsed.host_str() {
            None | Some("") | Some("localhost") => {}
            Some(host) => {
                return Err(RagError::UrlValidation(format!(
                    "file:// URL with remote host {host:?} is not allowed (only local paths)"
                )));
            }
        },
        other => {
            return Err(RagError::UrlValidation(format!(
                "URL scheme {other:?} is not allowed (only http/https/file)"
            )));
        }
    }

    Ok(())
}

/// Append one NDJSON failure entry to the failed-jobs log.
///
/// Performs size-based rotation under `log_lock`: if the log exceeds
/// `config.pipeline.failed_jobs_log_max_bytes`, the existing file is renamed to
/// `<path>.1` (overwriting any prior `.1` backup) and a fresh log is started.
///
/// The entire check-rotate-append sequence is serialised by `log_lock` so
/// concurrent workers cannot race on the rename.
async fn append_failed_job(path: &Path, error: &impl std::fmt::Display, ctx: &WorkerContext) {
    // Increment the parse-failure counter regardless of whether a log path is
    // configured — this ensures the heartbeat metric is always accurate.
    ctx.counters.record_parse_failure();

    let Some(ref log_path) = ctx.config.pipeline.failed_jobs_log else {
        return;
    };

    let entry = serde_json::json!({
        "path": path.to_string_lossy(),
        "error": error.to_string(),
        "ts": chrono::Utc::now().to_rfc3339(),
    });
    let line = format!("{entry}\n");

    // Serialise the check-rotate-append sequence across concurrent workers.
    let _guard = ctx.failed_jobs_log_lock.lock().await;

    let max_log_bytes = ctx.config.pipeline.failed_jobs_log_max_bytes;
    // Rotate if the log has grown past the cap.
    if let Ok(meta) = tokio::fs::metadata(log_path).await
        && meta.len() >= max_log_bytes
    {
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
                max_bytes = max_log_bytes,
                "rotated failed-jobs log"
            );
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

pub(crate) async fn process_job(job: IndexJob, ctx: &WorkerContext) -> Result<JobStats, RagError> {
    let job_start = std::time::Instant::now();

    let io_t0 = std::time::Instant::now();
    // Canonicalize can fail for benign reasons — most commonly ENOENT when the
    // file was deleted between the watcher event and job execution. That's a
    // race, not a backend failure, so we must NOT return `Err` (which would
    // wrongly bump `files_failed`). Return Ok with empty stats instead.
    let canonical = match tokio::fs::canonicalize(&job.path).await {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(
                path = %job.path.display(),
                "file vanished before processing"
            );
            return Ok(JobStats::default());
        }
        Err(e) => {
            tracing::warn!(
                path = %job.path.display(),
                error = %e,
                "canonicalize failed — skipping job"
            );
            return Ok(JobStats::default());
        }
    };
    if !scan::path_is_within_any_watch_root(&canonical, &ctx.watch_roots) {
        tracing::warn!(
            path = %job.path.display(),
            "path outside watch_dir — skipping (potential TOCTOU attack)"
        );
        ctx.counters.record_failure();
        return Ok(JobStats::default());
    }
    let file = tokio::fs::File::open(&canonical).await?;
    let file_meta = file.metadata().await?;
    let size = file_meta.len();

    let max_file_bytes = ctx.config.pipeline.max_file_size_bytes;
    if size > max_file_bytes {
        tracing::warn!(path = ?job.path, size, max_file_bytes, "file too large, skipping");
        return Ok(JobStats::default());
    }

    let mut file_bytes: Vec<u8> = Vec::with_capacity(size as usize);
    // Enforce the cap at I/O level — guards against grow-after-stat race where a
    // writer appends bytes between the metadata() check and read_to_end().
    file.take(max_file_bytes + 1)
        .read_to_end(&mut file_bytes)
        .await?;
    if file_bytes.len() as u64 > max_file_bytes {
        tracing::warn!(path = ?job.path, max_file_bytes, "file exceeded size limit after read (grow-after-stat), skipping");
        return Ok(JobStats::default());
    }
    let io_ms = io_t0.elapsed().as_millis() as u64;

    // Compute xxHash3 of raw bytes before file_bytes is moved into parse_file.
    let file_hash = format!("{:016x}", xxhash_rust::xxh3::xxh3_64(&file_bytes));

    let parse_t0 = std::time::Instant::now();
    let parsed = match parse::parse_file(&job.path, file_bytes).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(path = ?job.path, error = %e, "parse failed, skipping");
            append_failed_job(&job.path, &e, ctx).await;
            return Ok(JobStats::default());
        }
    };
    let parse_ms = parse_t0.elapsed().as_millis() as u64;
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
    let git_branch = if let Some(cached) =
        find_cached_branch(&canonical, &ctx.git_branch_cache)
    {
        // Cache hit — no spawn_blocking needed. `canonical` ancestors were walked
        // synchronously in async context (pure in-memory DashMap lookups, no I/O).
        cached
    } else {
        // Cache miss — dispatch to a blocking thread to read .git/HEAD from disk.
        // Store the result keyed by the canonical git root so future files in the
        // same repo hit the pre-check without spawning.
        let canonical_clone = canonical.clone();
        let cache = ctx.git_branch_cache.clone();
        tokio::task::spawn_blocking(move || {
            if let Some((git_root, branch)) = scan::detect_git_root_and_branch(&canonical_clone) {
                // Canonicalize the git root so subsequent lookups via `canonical`
                // ancestors always match, regardless of symlinks or path normalization.
                let key = std::fs::canonicalize(&git_root).unwrap_or(git_root);
                cache
                    .entry(key)
                    .or_insert_with(|| Some(branch.clone()))
                    .clone()
            } else {
                // Cache the miss so we don't re-stat .git for every file outside a repo.
                let key = std::fs::canonicalize(
                    canonical_clone.parent().unwrap_or(&canonical_clone),
                )
                .unwrap_or_else(|_| {
                    canonical_clone
                        .parent()
                        .unwrap_or(&canonical_clone)
                        .to_path_buf()
                });
                cache.entry(key).or_insert(None).clone()
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
        return Ok(JobStats::default());
    }
    let url = crate::url_util::normalize_url(&raw_url);

    // Freeze result in Arc: no further mutations, and the chunker closure needs
    // 'static ownership while we still need &result for build_point_payload below.
    let result = Arc::new(result);

    let t1 = std::time::Instant::now();
    let chunks = {
        let result_for_chunk = Arc::clone(&result);
        let config_chunker = ctx.config.chunker.clone();
        let tokenizer = Arc::clone(&ctx.tokenizer);
        tokio::task::spawn_blocking(move || {
            crate::chunker::chunk(&result_for_chunk, &config_chunker, &tokenizer)
        })
        .await
        .map_err(|e| RagError::WorkerPanic(format!("chunker panicked: {e}")))?
    };
    if chunks.is_empty() {
        tracing::info!(url = %url, "no indexable content after chunking");
        return Ok(JobStats::default());
    }
    let chunk_ms = t1.elapsed().as_millis() as u64;

    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let total_tokens: u64 = chunks.iter().map(|c| c.token_estimate as u64).sum();
    let t2 = std::time::Instant::now();
    // Cooperative cancellation: if shutdown fires mid-HTTP we return cleanly
    // rather than being force-aborted by the 10s drain timeout.
    let vectors = tokio::select! {
        r = ctx.embed.embed(&texts) => r?,
        _ = ctx.shutdown.cancelled() => {
            tracing::debug!(url = %url, "embed cancelled by shutdown");
            return Ok(JobStats::default());
        }
    };
    let embed_ms = t2.elapsed().as_millis() as u64;
    let embed_tokens_per_sec = (total_tokens * 1_000).checked_div(embed_ms).unwrap_or(0);

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
    // Build per-file metadata once; chunk loop clones from it instead of from
    // result + provenance on every iteration (bead noxa-346).
    let file_meta = parse::FileMetadata::from_result_and_provenance(
        &result,
        git_branch,
        &parsed.provenance,
    );
    let points: Vec<Point> = chunks
        .into_iter()
        .zip(vectors.into_iter())
        .enumerate()
        .map(|(i, (chunk, vector))| {
            let id = uuid::Uuid::new_v5(
                &ctx.config.uuid_namespace,
                format!("{}#chunk{}", url, i).as_bytes(),
            );
            Point {
                id,
                vector,
                payload: parse::build_point_payload(
                    chunk,
                    &file_meta,
                    &url,
                    Some(&file_hash),
                ),
            }
        })
        .collect();

    let url_lock = ctx
        .url_locks
        .entry(url.clone())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _guard = url_lock.lock().await;

    let new_ids: Vec<uuid::Uuid> = points.iter().map(|p| p.id).collect();
    let t4 = std::time::Instant::now();
    // Cooperative cancellation around upsert+delete so shutdown does not strand
    // in-flight HTTP calls past the drain timeout. On cancellation, implicit drop
    // of _guard and url_lock releases the per-URL mutex; heartbeat sweeps the
    // stale DashMap entry within 60s.
    let store_result: Result<u64, RagError> = tokio::select! {
        r = async {
            let upserted = ctx.store.upsert(points).await.map_err(|e| {
                tracing::error!(url = %url, error = %e, "upsert failed");
                e
            })?;
            let upsert_ms = t4.elapsed().as_millis() as u64;

            let t5 = std::time::Instant::now();
            ctx.store
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
            let delete_ms = t5.elapsed().as_millis() as u64;

            tracing::info!(
                url = %url,
                chunks = upserted,
                embed_tokens = total_tokens,
                embed_tokens_per_sec,
                io_ms,
                parse_ms,
                chunk_ms,
                embed_ms,
                delete_ms,
                upsert_ms,
                total_ms = job_start.elapsed().as_millis() as u64,
                "indexed"
            );

            Ok::<u64, RagError>(upsert_ms)
        } => r,
        _ = ctx.shutdown.cancelled() => {
            tracing::debug!(url = %url, "upsert cancelled by shutdown");
            return Ok(JobStats::default());
        }
    };

    drop(_guard);
    drop(url_lock);
    ctx.url_locks
        .remove_if(&url, |_, v| Arc::strong_count(v) == 1);

    let upsert_ms = store_result?;

    Ok(JobStats {
        chunks: n_chunks,
        io_ms,
        parse_ms,
        chunk_ms,
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
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "failed to delete chunks for removed file")
        }
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
        assert!(
            validate_url_scheme("file://localhost/tmp/foo.md")
                .await
                .is_ok()
        );
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
