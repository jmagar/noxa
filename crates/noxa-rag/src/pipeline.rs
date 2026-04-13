// Pipeline — filesystem watcher → chunk → embed → upsert
//
// Architecture:
//   notify-debouncer-mini (sync mpsc) → spawn_blocking bridge → tokio mpsc IndexJob queue
//   → embed_concurrency worker tasks → process_job()
//
// Key design decisions:
//   - Carry tracing::Span in IndexJob; tokio::spawn would drop it otherwise.
//   - Per-URL mutex (DashMap<String, Arc<Mutex<()>>>) prevents concurrent delete+upsert races.
//   - Workers bounded to embed_concurrency provide natural backpressure without a separate semaphore.
//   - notify-debouncer-mini 0.4.x uses a callback/sender API, not a receiver() method.
//     We use std::sync::mpsc::Sender<DebounceEventResult> as the handler and bridge via spawn_blocking.

use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use noxa_core::types::ExtractionResult;
use tokenizers::Tokenizer;

use crate::chunker;
use crate::config::{RagConfig, SourceConfig};
use crate::embed::DynEmbedProvider;
use crate::error::RagError;
use crate::store::DynVectorStore;
use crate::types::{Point, PointPayload};

// ─── Session counters ─────────────────────────────────────────────────────────

/// Shared session metrics updated by workers and read by the heartbeat/shutdown tasks.
#[derive(Default)]
struct SessionCounters {
    files_indexed: AtomicUsize,
    files_failed: AtomicUsize,
    total_chunks: AtomicUsize,
    total_embed_ms: AtomicU64,
    total_upsert_ms: AtomicU64,
}

// ─── IndexJob ────────────────────────────────────────────────────────────────

/// A unit of work: index the .json file at `path`.
/// The tracing `span` is carried explicitly because tokio::spawn does NOT
/// automatically propagate the current span into the new task.
struct IndexJob {
    path: PathBuf,
    span: tracing::Span,
}

// ─── Pipeline ────────────────────────────────────────────────────────────────

pub struct Pipeline {
    pub config: RagConfig,
    pub embed: DynEmbedProvider,
    pub store: DynVectorStore,
    pub tokenizer: Arc<Tokenizer>,
    pub shutdown: CancellationToken,
    /// Per-URL mutex: prevents concurrent delete-then-upsert races for the same URL.
    url_locks: Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Session-level metrics shared between workers, heartbeat, and shutdown tasks.
    counters: Arc<SessionCounters>,
}

impl Pipeline {
    pub fn new(
        config: RagConfig,
        embed: DynEmbedProvider,
        store: DynVectorStore,
        tokenizer: Arc<Tokenizer>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            config,
            embed,
            store,
            tokenizer,
            shutdown,
            url_locks: Arc::new(DashMap::new()),
            counters: Arc::new(SessionCounters::default()),
        }
    }

    /// Run the filesystem watcher pipeline.
    ///
    /// Returns when the CancellationToken is cancelled.
    pub async fn run(&self) -> Result<(), RagError> {
        // Extract watch config.
        let (watch_dir, debounce_ms) = match &self.config.source {
            SourceConfig::FsWatcher {
                watch_dir,
                debounce_ms,
            } => (watch_dir.clone(), *debounce_ms),
        };

        if self.config.pipeline.embed_concurrency == 0 {
            return Err(RagError::Config(
                "pipeline.embed_concurrency must be > 0 or no workers will run".to_string(),
            ));
        }

        tracing::info!(
            watch_dir = %watch_dir.display(),
            debounce_ms,
            embed_concurrency = self.config.pipeline.embed_concurrency,
            "pipeline starting"
        );

        // Bounded job queue: backpressure at 256 queued jobs.
        let (tx, rx) = tokio::sync::mpsc::channel::<IndexJob>(256);

        // Spawn worker pool — each worker owns a cloned rx.
        // We share a single receiver via Arc<Mutex<Receiver>> so all workers
        // compete fairly for jobs.
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let mut worker_handles = Vec::with_capacity(self.config.pipeline.embed_concurrency);

        for worker_id in 0..self.config.pipeline.embed_concurrency {
            let rx = rx.clone();
            let embed = self.embed.clone();
            let store = self.store.clone();
            let tokenizer = self.tokenizer.clone();
            let config = self.config.clone();
            let url_locks = self.url_locks.clone();
            let counters = self.counters.clone();

            let handle = tokio::spawn(async move {
                tracing::debug!(worker_id, "index worker started");
                loop {
                    let job = {
                        let mut guard = rx.lock().await;
                        guard.recv().await
                    };
                    match job {
                        Some(job) => {
                            let span = job.span.clone();
                            async {
                                match process_job(
                                    job, &embed, &store, &tokenizer, &config, &url_locks,
                                )
                                .await
                                {
                                    Ok(stats) => {
                                        counters.files_indexed.fetch_add(1, Ordering::Relaxed);
                                        counters
                                            .total_chunks
                                            .fetch_add(stats.chunks, Ordering::Relaxed);
                                        counters
                                            .total_embed_ms
                                            .fetch_add(stats.embed_ms, Ordering::Relaxed);
                                        counters
                                            .total_upsert_ms
                                            .fetch_add(stats.upsert_ms, Ordering::Relaxed);
                                    }
                                    Err(e) => {
                                        tracing::error!(error = %e, "index job failed");
                                        counters.files_failed.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                            .instrument(span)
                            .await;
                        }
                        None => {
                            // Sender dropped — workers drain and exit.
                            tracing::debug!(worker_id, "index worker shutting down");
                            break;
                        }
                    }
                }
            });

            worker_handles.push(handle);
        }

        // Build notify debouncer with a *bounded* sync channel as the event handler.
        // notify-debouncer-mini 0.4.x implements DebounceEventHandler for
        // std::sync::mpsc::Sender (unbounded) but not SyncSender, so we wrap
        // SyncSender in a small newtype.  When the bridge is blocked on
        // blocking_send (Tokio queue full) the sync_channel fills and the
        // debouncer's send() call blocks too — closing the backpressure loop.
        struct BoundedSender(std::sync::mpsc::SyncSender<DebounceEventResult>);
        impl notify_debouncer_mini::DebounceEventHandler for BoundedSender {
            fn handle_event(&mut self, event: DebounceEventResult) {
                // Blocks when the channel is full, propagating backpressure.
                let _ = self.0.send(event);
            }
        }

        let (notify_tx, notify_rx) = std::sync::mpsc::sync_channel::<DebounceEventResult>(256);

        let mut debouncer =
            new_debouncer(Duration::from_millis(debounce_ms), BoundedSender(notify_tx))
                .map_err(|e| RagError::Generic(format!("failed to create fs watcher: {e}")))?;

        debouncer
            .watcher()
            .watch(&watch_dir, RecursiveMode::Recursive)
            .map_err(|e| {
                RagError::Generic(format!(
                    "failed to watch directory {}: {e}",
                    watch_dir.display()
                ))
            })?;

        tracing::info!(path = %watch_dir.display(), "watching directory recursively");

        // Bridge: wrap the blocking notify_rx.recv() in spawn_blocking so it
        // doesn't block the tokio reactor.  Send jobs to the tokio job queue.
        let shutdown_clone = self.shutdown.clone();
        let tx_clone = tx.clone();

        let bridge_handle = tokio::task::spawn_blocking(move || {
            // Keep `debouncer` alive for the duration of this thread.
            let _debouncer = debouncer;

            loop {
                // recv_timeout lets us periodically check whether we should stop.
                // We check every 250 ms regardless of debounce setting.
                match notify_rx.recv_timeout(Duration::from_millis(250)) {
                    Ok(Ok(events)) => {
                        if shutdown_clone.is_cancelled() {
                            break;
                        }
                        for event in events {
                            for path in collect_indexable_paths(&event.path) {
                                let span = tracing::info_span!(
                                    "index_job",
                                    path = %path.display(),
                                );
                                let job = IndexJob { path, span };
                                // Retry with a short sleep so shutdown can interrupt a full queue.
                                let mut pending_job = job;
                                let mut saturated_logged = false;
                                loop {
                                    match tx_clone.try_send(pending_job) {
                                        Ok(()) => break,
                                        Err(tokio::sync::mpsc::error::TrySendError::Full(job)) => {
                                            if shutdown_clone.is_cancelled() {
                                                break;
                                            }
                                            if !saturated_logged {
                                                tracing::warn!(
                                                    "job queue saturated (256/256), \
                                                     backing off — embed/upsert catching up"
                                                );
                                                saturated_logged = true;
                                            }
                                            pending_job = job;
                                            std::thread::sleep(Duration::from_millis(10));
                                        }
                                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                            // Receiver dropped — workers are done; exit.
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(error = ?e, "fs watcher error");
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Check if we should stop.
                        if shutdown_clone.is_cancelled() {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }

            tracing::info!("fs watcher bridge exiting");
        });

        // Startup scan: index files already present in watch_dir when the daemon starts.
        //
        // Runs concurrently with the watcher so new events are not missed during the scan.
        // collect_indexable_paths uses std::fs (sync) — MUST run in spawn_blocking to avoid
        // stalling the tokio executor on NFS/CIFS with thousands of files.
        //
        // Delta detection: before enqueuing a path, compute SHA-256 of its bytes and check
        // Qdrant.  If a point with the same URL + content_hash already exists, the file has
        // not changed and is skipped. This prevents re-indexing the entire watch_dir on
        // every daemon restart.
        let scan_tx = tx.clone();
        let scan_store = self.store.clone();
        let scan_shutdown = self.shutdown.clone();
        let scan_watch_dir = watch_dir.clone();

        let startup_handle = tokio::spawn(async move {
            let paths = match tokio::task::spawn_blocking({
                let dir = scan_watch_dir.clone();
                move || collect_indexable_paths(&dir)
            })
            .await
            {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "startup scan: collect_indexable_paths panicked");
                    return;
                }
            };

            let total = paths.len();
            tracing::info!(count = total, "startup scan: checking files for delta");

            let mut queued = 0usize;
            let mut skipped = 0usize;

            for path in paths {
                if scan_shutdown.is_cancelled() {
                    break;
                }

                // Read file + compute URL+hash in spawn_blocking (sync file I/O).
                let path2 = path.clone();
                let hash_and_url = tokio::task::spawn_blocking(move || {
                    startup_scan_key(&path2)
                })
                .await
                .ok()
                .flatten();

                let (hash, url) = match hash_and_url {
                    Some(t) => t,
                    None => {
                        // Cannot determine URL/hash — enqueue conservatively.
                        tracing::debug!(path = %path.display(), "startup scan: no url/hash, queuing");
                        let span = tracing::info_span!("index_job", path = %path.display());
                        tokio::select! {
                            _ = scan_tx.send(IndexJob { path, span }) => {}
                            _ = scan_shutdown.cancelled() => { break; }
                        }
                        queued += 1;
                        continue;
                    }
                };

                // Delta check — skip files already indexed with the same content.
                // On Qdrant error: conservative (assume not indexed, re-enqueue).
                match scan_store.url_with_hash_exists(&url, &hash).await {
                    Ok(true) => {
                        skipped += 1;
                        tracing::debug!(
                            path = %path.display(),
                            url = %url,
                            "startup scan: already indexed, skipping"
                        );
                    }
                    Ok(false) | Err(_) => {
                        let span = tracing::info_span!("index_job", path = %path.display());
                        tokio::select! {
                            _ = scan_tx.send(IndexJob { path, span }) => {}
                            _ = scan_shutdown.cancelled() => { break; }
                        }
                        queued += 1;
                    }
                }
            }

            tracing::info!(total, queued, skipped, "startup scan complete");
        });

        // Heartbeat: log pipeline health every 60s.
        let heartbeat_counters = self.counters.clone();
        let heartbeat_shutdown = self.shutdown.clone();
        let session_start = Instant::now();
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            interval.tick().await; // consume immediate first tick
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let uptime_m = session_start.elapsed().as_secs() / 60;
                        tracing::info!(
                            indexed = heartbeat_counters.files_indexed.load(Ordering::Relaxed),
                            failed = heartbeat_counters.files_failed.load(Ordering::Relaxed),
                            uptime_m,
                            "pipeline alive"
                        );
                    }
                    _ = heartbeat_shutdown.cancelled() => break,
                }
            }
        });

        // Wait for cancellation signal.
        self.shutdown.cancelled().await;
        tracing::info!("shutdown signal received, draining pipeline");

        // Drop tx so workers drain their queues and exit.
        drop(tx);

        // Wait for bridge, heartbeat, and startup scan to finish.
        let _ = bridge_handle.await;
        let _ = heartbeat_handle.await;
        let _ = startup_handle.await;

        // Wait for all workers to drain — 10s hard limit to prevent a stuck
        // job from blocking indefinite shutdown.
        let drain = async {
            for handle in worker_handles {
                let _ = handle.await;
            }
        };
        match tokio::time::timeout(Duration::from_secs(10), drain).await {
            Ok(_) => tracing::info!("pipeline shut down cleanly"),
            Err(_) => {
                tracing::warn!("workers did not drain within 10s, forcing exit");
                return Err(RagError::Generic(
                    "workers did not drain within 10s".to_string(),
                ));
            }
        }

        // Shutdown session summary.
        let indexed = self.counters.files_indexed.load(Ordering::Relaxed);
        let failed = self.counters.files_failed.load(Ordering::Relaxed);
        let chunks = self.counters.total_chunks.load(Ordering::Relaxed);
        let embed_ms = self.counters.total_embed_ms.load(Ordering::Relaxed);
        let upsert_ms = self.counters.total_upsert_ms.load(Ordering::Relaxed);
        let avg_embed_ms = if indexed > 0 { embed_ms / indexed as u64 } else { 0 };
        let avg_upsert_ms = if indexed > 0 { upsert_ms / indexed as u64 } else { 0 };
        tracing::info!(
            indexed,
            failed,
            chunks,
            avg_embed_ms,
            avg_upsert_ms,
            duration_s = session_start.elapsed().as_secs(),
            "session complete"
        );

        Ok(())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns true iff the path has a supported extension AND exists on disk.
///
/// We check existence because rename events (vim/emacs atomic saves) may fire for
/// temp files that are gone by the time we process them.
///
/// Deferred (no confirmed use case, would add new crate deps): .epub, .eml, .mbox
fn is_indexable(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext,
        // ExtractionResult JSON (primary watch-dir format)
        "json"
        // Plain text
        | "md" | "txt" | "log" | "rst" | "org" | "yaml" | "yml" | "toml"
        // HTML
        | "html" | "htm"
        // Notebook
        | "ipynb"
        // Binary document (via noxa-pdf / zip unpack)
        | "pdf" | "docx" | "odt" | "pptx"
        // Structured data
        | "jsonl" | "xml" | "opml"
        // Subtitle / transcript
        | "vtt" | "srt"
        // RSS / Atom
        | "rss" | "atom"
    ) && path.exists()
}

fn collect_indexable_paths(path: &Path) -> Vec<PathBuf> {
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
        // Never follow symlinks — prevents watch_dir/root -> / traversal attacks.
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

/// Returns true iff `host` resolves to a private/loopback/link-local address.
fn is_private_ip(host: &str) -> bool {
    if let Ok(addr) = host.parse::<IpAddr>() {
        return match addr {
            IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
            IpAddr::V6(ip) => {
                ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
            }
        };
    }
    false
}

/// Validate that `url` uses http or https and does not point to a private IP.
fn validate_url_scheme(url: &str) -> Result<(), RagError> {
    if url.is_empty() {
        return Err(RagError::Generic(
            "extraction result has no URL".to_string(),
        ));
    }
    let parsed =
        url::Url::parse(url).map_err(|e| RagError::Generic(format!("invalid URL {url:?}: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {
            // Block private/loopback IP literals and localhost for remote schemes.
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
        "file" => {
            // Local file:// only — no remote file://server/path references.
            // RFC 8089 allows `file://localhost/path` as equivalent to `file:///path`.
            match parsed.host_str() {
                None | Some("") | Some("localhost") => {}
                Some(host) => {
                    return Err(RagError::Generic(format!(
                        "file:// URL with remote host {host:?} is not allowed (only local paths)"
                    )));
                }
            }
        }
        other => {
            return Err(RagError::Generic(format!(
                "URL scheme {other:?} is not allowed (only http/https/file)"
            )));
        }
    }

    Ok(())
}

// ─── Format dispatch ─────────────────────────────────────────────────────────

/// Parse a local file into a normalised `ExtractionResult` for the RAG pipeline.
///
/// Dispatches to the right extractor based on file extension.  Heavy / CPU-bound
/// formats (PDF, DOCX, ipynb) run inside `spawn_blocking` so the tokio executor
/// is never stalled.  All formats set:
///   - `metadata.url`         = file:// URI (percent-encoded, via url crate)
///   - `metadata.domain`      = NOT set here — "local" sentinel set in process_job
///   - `metadata.source_type` = "file"
///   - `metadata.title`       = filename stem (unless the format provides a better one)
///
/// Returns `Err(RagError::Parse(...))` on unrecoverable format errors.
async fn parse_file(path: &Path, bytes: Vec<u8>) -> Result<ExtractionResult, RagError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("json");
    let file_url = url::Url::from_file_path(path)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned());
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Helper: bytes → UTF-8 String with replacement for invalid sequences.
    let as_text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();

    match ext {
        // ── JSON ExtractionResult ──────────────────────────────────────────────
        "json" => serde_json::from_slice::<ExtractionResult>(&bytes)
            .map_err(|e| RagError::Parse(format!("JSON parse failed: {e}"))),

        // ── Plain text group (.md .txt .log .rst .org .yaml .yml .toml) ───────
        "md" | "rst" | "org" => {
            let content = as_text(&bytes);
            let word_count = content.split_whitespace().count();
            Ok(make_text_result(content, String::new(), file_url, Some(title), "file", word_count))
        }
        "txt" | "yaml" | "yml" | "toml" => {
            let content = as_text(&bytes);
            let word_count = content.split_whitespace().count();
            Ok(make_text_result(
                content.clone(),
                content,
                file_url,
                Some(title),
                "file",
                word_count,
            ))
        }
        "log" => {
            let raw = as_text(&bytes);
            let stripped = strip_ansi_escapes::strip_str(&raw);
            let word_count = stripped.split_whitespace().count();
            Ok(make_text_result(
                stripped.clone(),
                stripped,
                file_url,
                Some(title),
                "file",
                word_count,
            ))
        }

        // ── HTML ───────────────────────────────────────────────────────────────
        "html" | "htm" => {
            let html = as_text(&bytes);
            let url_for_extract = file_url.clone();
            tokio::task::spawn_blocking(move || -> Result<ExtractionResult, RagError> {
                let mut r = noxa_core::extract(&html, Some(&url_for_extract))
                    .map_err(|e| RagError::Parse(format!("HTML extract: {e}")))?;
                r.metadata.url = Some(url_for_extract);
                r.metadata.source_type = Some("file".to_string());
                Ok(r)
            })
            .await
            .map_err(|e| RagError::Parse(format!("HTML spawn_blocking: {e}")))?
        }

        // ── Jupyter Notebook ──────────────────────────────────────────────────
        "ipynb" => {
            tokio::task::spawn_blocking(move || parse_ipynb(&bytes, file_url, title))
                .await
                .map_err(|e| RagError::Parse(format!("ipynb spawn_blocking: {e}")))?
        }

        // ── PDF ────────────────────────────────────────────────────────────────
        "pdf" => {
            tokio::task::spawn_blocking(move || parse_pdf(&bytes, file_url, title))
                .await
                .map_err(|e| RagError::Parse(format!("PDF spawn_blocking: {e}")))?
        }

        // ── Office binary formats (ZIP-based) ─────────────────────────────────
        "docx" => {
            tokio::task::spawn_blocking(move || parse_office_zip(&bytes, file_url, title, "docx"))
                .await
                .map_err(|e| RagError::Parse(format!("DOCX spawn_blocking: {e}")))?
        }
        "odt" => {
            tokio::task::spawn_blocking(move || parse_office_zip(&bytes, file_url, title, "odt"))
                .await
                .map_err(|e| RagError::Parse(format!("ODT spawn_blocking: {e}")))?
        }
        "pptx" => {
            tokio::task::spawn_blocking(move || parse_office_zip(&bytes, file_url, title, "pptx"))
                .await
                .map_err(|e| RagError::Parse(format!("PPTX spawn_blocking: {e}")))?
        }

        // ── Structured text (.jsonl .xml .opml .rss .atom) ────────────────────
        "jsonl" => {
            let content = as_text(&bytes);
            let text = content
                .lines()
                .filter_map(|line| {
                    let v: serde_json::Value = serde_json::from_str(line).ok()?;
                    ["text", "content", "body", "message", "value"]
                        .iter()
                        .find_map(|k| v[k].as_str().map(str::to_string))
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            let word_count = text.split_whitespace().count();
            Ok(make_text_result(text.clone(), text, file_url, Some(title), "file", word_count))
        }
        "xml" | "opml" | "rss" | "atom" => {
            let content = as_text(&bytes);
            let text = extract_xml_text(&content);
            let word_count = text.split_whitespace().count();
            Ok(make_text_result(text.clone(), text, file_url, Some(title), "file", word_count))
        }

        // ── Subtitle / transcript (.vtt .srt) ─────────────────────────────────
        "vtt" | "srt" => {
            let content = as_text(&bytes);
            let text = strip_subtitle_timestamps(&content);
            let word_count = text.split_whitespace().count();
            Ok(make_text_result(text.clone(), text, file_url, Some(title), "file", word_count))
        }

        // ── Unknown / unsupported ──────────────────────────────────────────────
        other => Err(RagError::Parse(format!("unsupported file extension: .{other}"))),
    }
}

/// Build a minimal ExtractionResult from pre-extracted text.
fn make_text_result(
    markdown: String,
    plain_text: String,
    url: String,
    title: Option<String>,
    source_type: &str,
    word_count: usize,
) -> ExtractionResult {
    ExtractionResult {
        metadata: noxa_core::Metadata {
            title,
            description: None,
            author: None,
            published_date: None,
            language: None,
            url: Some(url),
            site_name: None,
            image: None,
            favicon: None,
            word_count,
            content_hash: None, // filled by process_job if needed
            source_type: Some(source_type.to_string()),
            file_path: None, // filled by process_job
            last_modified: None, // filled by process_job
            is_truncated: None,
            technologies: Vec::new(),
            seed_url: None,
            crawl_depth: None,
            search_query: None,
            fetched_at: None,
        },
        content: noxa_core::Content {
            markdown,
            plain_text,
            links: Vec::new(),
            images: Vec::new(),
            code_blocks: Vec::new(),
            raw_html: None,
        },
        domain_data: None,
        structured_data: Vec::new(),
    }
}

/// Parse a Jupyter Notebook (.ipynb) — must run in spawn_blocking.
///
/// Extracts source from code + markdown cells only.
/// **Strips cell outputs** to prevent indexing of stack traces, env dumps, or PII.
fn parse_ipynb(bytes: &[u8], url: String, title: String) -> Result<ExtractionResult, RagError> {
    let v: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| RagError::Parse(format!("ipynb JSON parse: {e}")))?;

    let cells = v["cells"]
        .as_array()
        .ok_or_else(|| RagError::Parse("ipynb: missing 'cells' array".to_string()))?;

    let mut parts: Vec<String> = Vec::new();
    for cell in cells {
        let cell_type = cell["cell_type"].as_str().unwrap_or("");
        if !matches!(cell_type, "markdown" | "code") {
            continue;
        }
        // source is either a string or an array of strings.
        let source = match &cell["source"] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(lines) => lines
                .iter()
                .filter_map(|l| l.as_str())
                .collect::<String>(),
            _ => continue,
        };
        // Skip empty cells.
        let trimmed = source.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
        // Outputs are intentionally NOT indexed (may contain PII/env dumps).
    }

    let text = parts.join("\n\n");
    let word_count = text.split_whitespace().count();
    Ok(make_text_result(text.clone(), text, url, Some(title), "notebook", word_count))
}

/// Extract text from a PDF — must run in spawn_blocking.
fn parse_pdf(bytes: &[u8], url: String, title: String) -> Result<ExtractionResult, RagError> {
    let result = noxa_pdf::extract_pdf(
        bytes,
        noxa_pdf::PdfMode::Auto,
    )
    .map_err(|e| RagError::Parse(format!("PDF extract: {e}")))?;
    let text = noxa_pdf::to_markdown(&result);
    let word_count = text.split_whitespace().count();
    Ok(make_text_result(text.clone(), text, url, Some(title), "file", word_count))
}

/// Shared ZIP-based office parser for DOCX, ODT, PPTX — must run in spawn_blocking.
///
/// Uses noxa-fetch's tested DOCX extractor for .docx.
/// ODT and PPTX are extracted via ZIP text-node scan (sufficient for indexing).
///
/// **Decompressed-size guard**: entries > 100 MiB or archives > 1 000 entries
/// are rejected to prevent zip-bomb DoS.
fn parse_office_zip(
    bytes: &[u8],
    url: String,
    title: String,
    ext: &str,
) -> Result<ExtractionResult, RagError> {
    use std::io::Read;

    const MAX_ENTRY_SIZE: u64 = 100 * 1024 * 1024; // 100 MiB decompressed
    const MAX_ENTRIES: usize = 1_000;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| RagError::Parse(format!("{ext} ZIP open: {e}")))?;

    if archive.len() > MAX_ENTRIES {
        return Err(RagError::Parse(format!(
            "{ext}: archive has {} entries (max {MAX_ENTRIES}) — possible zip bomb",
            archive.len()
        )));
    }

    // For DOCX, delegate to the tested noxa-fetch extractor.
    if ext == "docx" {
        let result = noxa_fetch::document::extract_document(bytes, noxa_fetch::document::DocType::Docx)
            .map_err(|e| RagError::Parse(format!("DOCX extract: {e}")))?;
        let mut r = result;
        r.metadata.url = Some(url);
        r.metadata.source_type = Some("file".to_string());
        if r.metadata.title.is_none() {
            r.metadata.title = Some(title);
        }
        return Ok(r);
    }

    // ODT and PPTX: scan all XML entries for text nodes.
    // ODT: content.xml; PPTX: ppt/slides/slide*.xml
    let target_prefix = match ext {
        "odt" => "content",
        "pptx" => "ppt/slides/slide",
        _ => "",
    };

    let mut text_parts: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| RagError::Parse(format!("{ext} entry {i}: {e}")))?;

        if entry.size() > MAX_ENTRY_SIZE {
            return Err(RagError::Parse(format!(
                "{ext}: entry '{}' decompresses to {} bytes (max 100 MiB) — possible zip bomb",
                entry.name(),
                entry.size()
            )));
        }

        let name = entry.name().to_string();
        if !name.ends_with(".xml") {
            continue;
        }
        if !target_prefix.is_empty() && !name.contains(target_prefix) {
            continue;
        }

        let mut xml_buf = String::new();
        entry
            .read_to_string(&mut xml_buf)
            .map_err(|e| RagError::Parse(format!("{ext} read '{name}': {e}")))?;

        // Simple text-node extraction via quick-xml.
        let fragment = extract_xml_text(&xml_buf);
        if !fragment.trim().is_empty() {
            text_parts.push(fragment);
        }
    }

    let text = text_parts.join("\n\n");
    let word_count = text.split_whitespace().count();
    Ok(make_text_result(
        text.clone(),
        text,
        url,
        Some(title),
        "file",
        word_count,
    ))
}

/// Extract plain text from XML/OPML/RSS/Atom by collecting all text nodes.
/// Strips all tags; trims and deduplicates blank lines.
fn extract_xml_text(xml: &str) -> String {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    let mut parts: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => {
                if let Ok(text) = e.unescape() {
                    let t = text.trim().to_string();
                    if !t.is_empty() {
                        parts.push(t);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    parts.join("\n")
}

/// Strip timestamp / cue header lines from WebVTT and SRT subtitles.
/// Keeps only the spoken text lines.
fn strip_subtitle_timestamps(content: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        // Skip WEBVTT header, blank lines as separators, cue timecodes,
        // numeric cue identifiers (SRT), and NOTE/STYLE/REGION blocks.
        if trimmed.is_empty()
            || trimmed.starts_with("WEBVTT")
            || trimmed.starts_with("NOTE")
            || trimmed.starts_with("STYLE")
            || trimmed.starts_with("REGION")
            || trimmed.contains("-->")
            || trimmed.chars().all(|c| c.is_ascii_digit())
        {
            continue;
        }
        lines.push(trimmed);
    }
    lines.join(" ")
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
/// **Must be called inside `spawn_blocking`** — this function reads from disk synchronously.
fn startup_scan_key(path: &std::path::Path) -> Option<(String, String)> {
    use sha2::Digest;

    let bytes = std::fs::read(path).ok()?;

    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        // Partial deserialisation: only decode the metadata header, not the full content.
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
            if let Some(url) = q.metadata.url {
                if !url.is_empty() {
                    return Some((hash, url));
                }
            }
        }
    }

    // Non-JSON or JSON without a stored URL: use file:// + SHA-256 of file bytes.
    let hash = format!("{:x}", sha2::Sha256::digest(&bytes));
    let url = url::Url::from_file_path(path).ok()?.to_string();
    Some((hash, url))
}

/// Walk up the directory tree from `file_path` to find a `.git/HEAD` file.
///
/// Reads the HEAD ref to extract the branch name: `ref: refs/heads/<branch>`.
/// Returns `None` when not in a git repo, on detached HEAD, or on any I/O error.
/// Uses only file reads — no subprocess, no git binary required.
fn detect_git_branch(file_path: &Path) -> Option<String> {
    let mut dir = file_path.parent()?;
    loop {
        let head = dir.join(".git").join("HEAD");
        if head.exists() {
            let content = std::fs::read_to_string(&head).ok()?;
            // `ref: refs/heads/main\n` → `main`
            return content.trim().strip_prefix("ref: refs/heads/").map(str::to_string);
        }
        dir = dir.parent()?;
    }
}

/// Append a failed-job record to the configured log file (NDJSON format).
/// Silently ignores if no log path is configured.
async fn append_failed_job(path: &Path, error: &impl std::fmt::Display, config: &RagConfig) {
    let Some(ref log_path) = config.pipeline.failed_jobs_log else {
        return;
    };
    let entry = serde_json::json!({
        "path": path.to_string_lossy(),
        "error": error.to_string(),
        "ts": chrono::Utc::now().to_rfc3339(),
    });
    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await
    {
        use tokio::io::AsyncWriteExt;
        let _ = file.write_all(format!("{}\n", entry).as_bytes()).await;
    }
}

// ─── Core processing ─────────────────────────────────────────────────────────

/// Per-job timing and volume stats reported back to the worker loop.
struct JobStats {
    chunks: usize,
    embed_ms: u64,
    upsert_ms: u64,
}

async fn process_job(
    job: IndexJob,
    embed: &DynEmbedProvider,
    store: &DynVectorStore,
    tokenizer: &Arc<Tokenizer>,
    config: &RagConfig,
    url_locks: &Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
) -> Result<JobStats, RagError> {
    let job_start = Instant::now();

    // ── 1. Open file and check size from the same FD (TOCTOU fix) ────────────
    let t0 = Instant::now();
    let mut file = tokio::fs::File::open(&job.path).await?;
    let file_meta = file.metadata().await?;
    let size = file_meta.len();

    // Path confinement check — guard against TOCTOU rename/hardlink attacks.
    // Canonicalize resolves any symlink components in the path itself.
    let canonical = tokio::fs::canonicalize(&job.path).await.map_err(|e| {
        RagError::Generic(format!(
            "canonicalize failed for {}: {e}",
            job.path.display()
        ))
    })?;
    let watch_dir = match &config.source {
        SourceConfig::FsWatcher { watch_dir, .. } => watch_dir.clone(),
    };
    let watch_canonical = tokio::fs::canonicalize(&watch_dir).await.map_err(|e| {
        RagError::Generic(format!("canonicalize watch_dir failed: {e}"))
    })?;
    if !canonical.starts_with(&watch_canonical) {
        tracing::warn!(
            path = %job.path.display(),
            "path outside watch_dir — skipping (potential TOCTOU attack)"
        );
        return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
    }

    const MAX_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024; // 50 MiB
    if size > MAX_FILE_SIZE_BYTES {
        tracing::warn!(
            path = ?job.path,
            size,
            "file too large (>50MB), skipping"
        );
        return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
    }

    // Read as bytes so binary formats (PDF, DOCX, PPTX, ODT) are handled correctly.
    // Text formats convert bytes → String inside parse_file with UTF-8 replacement.
    let mut file_bytes: Vec<u8> = Vec::with_capacity(size as usize);
    file.read_to_end(&mut file_bytes).await?;
    let parse_ms = t0.elapsed().as_millis() as u64;

    // ── 2. Parse / ingest by file format ─────────────────────────────────────
    // parse_file() dispatches to the right extractor for each format and returns
    // a normalized ExtractionResult.  Non-JSON formats run in spawn_blocking.
    let mut result: ExtractionResult = match parse_file(&job.path, file_bytes).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(path = ?job.path, error = %e, "parse failed, skipping");
            append_failed_job(&job.path, &e, config).await;
            return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
        }
    };

    // ── 3a. Populate filesystem provenance (noxa-9ww) ─────────────────────────
    // Set file_path and last_modified from job.path if not already populated
    // by the source tool or ingester. git_branch is read from .git/HEAD walk-up.
    if result.metadata.file_path.is_none() {
        result.metadata.file_path = Some(job.path.to_string_lossy().into_owned());
    }
    if result.metadata.last_modified.is_none() {
        if let Ok(mtime) = file_meta.modified() {
            result.metadata.last_modified =
                Some(chrono::DateTime::<chrono::Utc>::from(mtime).to_rfc3339());
        }
    }
    let git_branch = detect_git_branch(&job.path);

    // ── 3b. URL validation ────────────────────────────────────────────────────
    let raw_url = result.metadata.url.as_deref().unwrap_or("").to_string();
    if let Err(e) = validate_url_scheme(&raw_url) {
        tracing::warn!(path = ?job.path, error = %e, "url validation failed, skipping");
        return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
    }
    // Normalize so the mutex key and stored payload match what delete_by_url queries.
    let url = crate::store::qdrant::normalize_url(&raw_url);

    // ── 4. Chunk ─────────────────────────────────────────────────────────────
    let t1 = Instant::now();
    let chunks = chunker::chunk(&result, &config.chunker, tokenizer);
    if chunks.is_empty() {
        tracing::info!(url = %url, "no indexable content after chunking");
        return Ok(JobStats { chunks: 0, embed_ms: 0, upsert_ms: 0 });
    }
    let chunk_ms = t1.elapsed().as_millis() as u64;

    // ── 5. Embed ──────────────────────────────────────────────────────────────
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let total_tokens: u64 = chunks.iter().map(|c| c.token_estimate as u64).sum();
    let t2 = Instant::now();
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

    // ── 6. Build points with deterministic UUID v5 ────────────────────────────
    // Use the normalized URL for both the UUID seed and payload.url so that
    // delete_by_url (which also normalizes) matches the stored value for any
    // equivalent URL form (trailing slash, fragment, etc.).
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
                payload: PointPayload {
                    text: chunk.text.clone(),
                    url: url.clone(),
                    domain: chunk.domain.clone(),
                    chunk_index: chunk.chunk_index,
                    total_chunks: chunk.total_chunks,
                    token_estimate: chunk.token_estimate,
                    title: result.metadata.title.clone(),
                    author: result.metadata.author.clone(),
                    published_date: result.metadata.published_date.clone(),
                    language: result.metadata.language.clone(),
                    source_type: result.metadata.source_type.clone(),
                    content_hash: result.metadata.content_hash.clone(),
                    technologies: result.metadata.technologies.clone(),
                    is_truncated: result.metadata.is_truncated,
                    file_path: result.metadata.file_path.clone(),
                    last_modified: result.metadata.last_modified.clone(),
                    git_branch: git_branch.clone(),
                    // IngestionContext provenance fields — populated in Wave 3 by MCP sources.
                    external_id: None,
                    platform_url: None,
                    seed_url: None,
                    search_query: None,
                    crawl_depth: None,
                },
            }
        })
        .collect();

    // ── 7. Per-URL mutex: delete-then-upsert under lock ───────────────────────
    let url_lock = url_locks
        .entry(url.clone())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _guard = url_lock.lock().await;

    // SAFETY NOTE: delete-before-upsert is destructive: if upsert fails after
    // delete, the document is temporarily unindexed until the next file event
    // triggers a re-index. This is acceptable given the current store API lacks
    // a transactional delete-by-url-excluding-ids. UUIDs are deterministic (v5),
    // so re-indexing is always idempotent.
    //
    // Capture the result instead of returning immediately so we can always run
    // the eviction logic below, even on error paths.
    let t3 = Instant::now();
    let store_result: Result<u64, RagError> = async {
        let stale = store.delete_by_url(&url).await?;
        let delete_ms = t3.elapsed().as_millis() as u64;

        let t4 = Instant::now();
        let upserted = store.upsert(points).await.map_err(|e| {
            tracing::error!(
                url = %url,
                error = %e,
                "upsert failed after delete — document temporarily unindexed until next file event"
            );
            e
        })?;
        let upsert_ms = t4.elapsed().as_millis() as u64;

        if stale > 0 {
            tracing::info!(
                url = %url,
                format = "json",
                chunks = upserted,
                stale_deleted = stale,
                embed_tokens = total_tokens,
                embed_tokens_per_sec,
                parse_ms,
                chunk_ms,
                embed_ms,
                delete_ms,
                upsert_ms,
                total_ms = job_start.elapsed().as_millis() as u64,
                "reindexed"
            );
        } else {
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
        }

        Ok(upsert_ms)
    }
    .await;

    // Always evict the lock entry — including on error paths — to prevent
    // unbounded DashMap growth during store outages.
    drop(_guard);
    // Drop the local Arc clone before eviction check so strong_count reaches 1.
    drop(url_lock);
    url_locks.remove_if(&url, |_, v| Arc::strong_count(v) == 1);

    let upsert_ms = store_result?;

    Ok(JobStats { chunks: n_chunks, embed_ms, upsert_ms })
}

#[cfg(test)]
mod tests {
    use super::{collect_indexable_paths, detect_git_branch, is_indexable};
    use std::fs;

    #[test]
    fn collect_indexable_paths_finds_nested_supported_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let nested = root.join("docs/get-started");
        fs::create_dir_all(&nested).expect("create nested dirs");
        fs::write(root.join("top.json"), "{}").expect("write top-level json");
        fs::write(nested.join("guide.json"), "{}").expect("write nested json");
        // .epub is explicitly deferred — should NOT be returned.
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
            "atom",
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
        for ext in &["epub", "eml", "mbox"] {
            let path = root.join(format!("file.{ext}"));
            fs::write(&path, "x").expect("write file");
            assert!(!is_indexable(&path), ".{ext} should NOT be indexable (deferred)");
        }
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
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature/noxa-rag\n")
            .expect("write HEAD");
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
        // Detached HEAD: just a commit SHA, no "ref: refs/heads/" prefix.
        fs::write(git_dir.join("HEAD"), "abc123def456\n").expect("write HEAD");
        let file = tmp.path().join("foo.txt");
        fs::write(&file, "x").expect("write file");
        assert_eq!(detect_git_branch(&file), None);
    }
}
