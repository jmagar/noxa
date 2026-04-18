use std::net::IpAddr;
use std::path::Path;
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
use super::{IndexJob, JobStats, SessionCounters};

/// Maximum size of the failed-jobs log before it is rotated (10 MiB).
const FAILED_JOBS_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;

fn is_private_ip(host: &str) -> bool {
    if let Ok(addr) = host.parse::<IpAddr>() {
        return is_private_or_reserved_ip(addr);
    }
    false
}

fn is_private_or_reserved_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_loopback()
                || v4.is_unspecified()
                || v4.is_link_local()
                || v4.is_multicast()
                || octets[0] == 10
                || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 100 && octets[1] >= 64 && octets[1] <= 127)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_or_reserved_ip(IpAddr::V4(v4));
            }
            let seg0 = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg0 & 0xffc0) == 0xfe80
                || (seg0 & 0xfe00) == 0xfc00
        }
    }
}

pub(crate) fn validate_url_scheme(url: &str) -> Result<(), RagError> {
    if url.is_empty() {
        return Err(RagError::Generic(
            "extraction result has no URL".to_string(),
        ));
    }
    let parsed =
        url::Url::parse(url).map_err(|e| RagError::Generic(format!("invalid URL {url:?}: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {
            if let Some(host) = parsed.host_str() {
                if is_private_ip(host) {
                    return Err(RagError::Generic(format!(
                        "URL {url:?} uses a private/loopback IP literal as its host — indexing blocked"
                    )));
                }
                if host.eq_ignore_ascii_case("localhost") {
                    return Err(RagError::Generic(
                        "URL points to localhost — indexing blocked".to_string(),
                    ));
                }
            }
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
            let mut rotated = log_path.as_os_str().to_owned();
            rotated.push(".1");
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
    watch_root: &Path,
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
    if !scan::path_is_within_watch_root(&canonical, watch_root) {
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
    let git_branch = scan::detect_git_branch(&job.path);

    let raw_url = result.metadata.url.clone().unwrap_or_else(|| {
        url::Url::from_file_path(&canonical)
            .map(|url| url.to_string())
            .unwrap_or_else(|_| canonical.to_string_lossy().into_owned())
    });
    if let Err(e) = validate_url_scheme(&raw_url) {
        tracing::warn!(path = ?job.path, error = %e, "url validation failed, skipping");
        return Ok(JobStats {
            chunks: 0,
            embed_ms: 0,
            upsert_ms: 0,
        });
    }
    let url = crate::url_util::normalize_url(&raw_url);

    let t1 = std::time::Instant::now();
    let chunks = crate::chunker::chunk(&result, &config.chunker, tokenizer);
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
            format = "json",
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

#[cfg(test)]
mod tests {
    use super::validate_url_scheme;

    #[test]
    fn validate_url_scheme_accepts_file_local_path() {
        assert!(validate_url_scheme("file:///tmp/foo.md").is_ok());
    }

    #[test]
    fn validate_url_scheme_accepts_file_localhost_host() {
        assert!(validate_url_scheme("file://localhost/tmp/foo.md").is_ok());
    }

    #[test]
    fn validate_url_scheme_rejects_file_with_remote_host() {
        let result = validate_url_scheme("file://remoteserver/share/doc.txt");
        assert!(result.is_err());
    }

    #[test]
    fn validate_url_scheme_accepts_https() {
        assert!(validate_url_scheme("https://example.com/page").is_ok());
    }

    #[test]
    fn validate_url_scheme_rejects_ftp() {
        let result = validate_url_scheme("ftp://example.com/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn validate_url_scheme_rejects_empty_url() {
        assert!(validate_url_scheme("").is_err());
    }
}
