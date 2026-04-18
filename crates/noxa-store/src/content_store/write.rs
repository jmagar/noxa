use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::content_store::migrate::parse_sidecar_or_migrate;
use crate::content_store::permissions::{set_dir_permissions, set_file_permissions};
use crate::content_store::{ChangelogEntry, FilesystemContentStore, Sidecar};
use crate::types::{StoreError, StoreResult};

impl FilesystemContentStore {
    pub async fn write(
        &self,
        url: &str,
        extraction: &noxa_core::ExtractionResult,
    ) -> Result<StoreResult, StoreError> {
        tokio::fs::create_dir_all(&self.root).await?;
        let _ = self.get_canonical_root()?;
        let base = self.resolve_path(url)?;
        let md_path = base.with_extension("md");
        let json_path = base.with_extension("json");

        if let Some(parent) = md_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
            #[cfg(unix)]
            set_dir_permissions(parent)?;
        }

        let to_store = sanitize_extraction(url, extraction.clone());

        let now = Utc::now();
        let existing_sidecar = self.load_existing_sidecar(&json_path, now).await?;
        let (mut sidecar, is_new, changed, word_count_delta, diff) =
            build_sidecar(existing_sidecar, &to_store, url, now).await?;
        cap_changelog(&mut sidecar, self.max_changelog_entries);

        let json_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, StoreError> {
            Ok(serde_json::to_vec(&sidecar)?)
        })
        .await??;

        if self.is_oversized(&to_store, &json_bytes, &md_path, &json_path) {
            return Ok(StoreResult {
                md_path,
                json_path,
                is_new: false,
                changed: false,
                word_count_delta: 0,
                diff: None,
            });
        }

        write_sidecar_files(
            &md_path,
            &json_path,
            &to_store,
            &json_bytes,
            is_new || changed,
        )
        .await?;

        // Invalidate the manifest cache so the next list_all_docs() call picks
        // up the newly written document.
        //
        // FIXME: TOCTOU — see PR #12 thread 9.  A concurrent list_all_docs()
        // that began its filesystem walk *before* this write completes can
        // finish its walk *after* this invalidation and repopulate the cache
        // with a snapshot that is missing the document we just wrote.  The
        // 30-second TTL in ManifestCache::is_fresh() bounds the staleness
        // window: at most one TTL period elapses before the next cache miss
        // forces a fresh walk.  A proper fix would require a generation counter
        // that list_all_docs() stamps into the cache entry and compares against
        // the value captured at walk-start, rejecting the result if the counter
        // has advanced in the interim.
        self.manifest_cache.invalidate().await;

        Ok(StoreResult {
            md_path,
            json_path,
            is_new,
            changed,
            word_count_delta,
            diff,
        })
    }

    fn is_oversized(
        &self,
        extraction: &noxa_core::ExtractionResult,
        json_bytes: &[u8],
        md_path: &Path,
        json_path: &Path,
    ) -> bool {
        // Measure the actual bytes that will be written to disk:
        // markdown file + serialized sidecar JSON file.
        let estimated = extraction.content.markdown.len() + json_bytes.len();
        if let Some(max) = self.max_content_bytes
            && estimated > max
        {
            tracing::warn!(
                url = %md_path.display(),
                estimated,
                max,
                sidecar = %json_path.display(),
                "content store: skipping oversized document"
            );
            return true;
        }
        false
    }

    async fn load_existing_sidecar(
        &self,
        json_path: &Path,
        now: DateTime<Utc>,
    ) -> Result<Option<Sidecar>, StoreError> {
        match tokio::fs::read_to_string(json_path).await {
            Ok(contents) => {
                let mtime = tokio::fs::metadata(json_path)
                    .await
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(DateTime::<Utc>::from)
                    .unwrap_or(now);
                let json_path_for_error = json_path.to_path_buf();
                let parsed =
                    tokio::task::spawn_blocking(move || parse_sidecar_or_migrate(&contents, mtime))
                        .await?
                        .map_err(|source| StoreError::CorruptSidecar {
                            path: json_path_for_error,
                            source,
                        })?;
                Ok(Some(parsed))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

fn sanitize_extraction(
    url: &str,
    mut extraction: noxa_core::ExtractionResult,
) -> noxa_core::ExtractionResult {
    if let Some(ref url_str) = extraction.metadata.url
        && let Ok(mut parsed) = url::Url::parse(url_str)
    {
        parsed.set_query(None);
        extraction.metadata.url = Some(parsed.to_string());
    }

    extraction.content.raw_html = None;
    extraction.metadata.file_path = None;
    extraction.metadata.search_query = None;

    if extraction.metadata.url.is_none()
        && let Ok(mut parsed) = url::Url::parse(url)
    {
        parsed.set_query(None);
        extraction.metadata.url = Some(parsed.to_string());
    }

    extraction
}

async fn build_sidecar(
    existing_sidecar: Option<Sidecar>,
    to_store: &noxa_core::ExtractionResult,
    url: &str,
    now: DateTime<Utc>,
) -> Result<(Sidecar, bool, bool, i64, Option<noxa_core::ContentDiff>), StoreError> {
    if let Some(mut existing) = existing_sidecar {
        let previous = existing.current.clone();
        let current = to_store.clone();
        let content_diff =
            tokio::task::spawn_blocking(move || noxa_core::diff::diff(&previous, &current))
                .await
                .map_err(StoreError::TaskJoin)?;
        let changed = content_diff.status == noxa_core::ChangeStatus::Changed;
        let word_count_delta =
            to_store.metadata.word_count as i64 - existing.current.metadata.word_count as i64;

        existing.last_fetched = now;
        existing.fetch_count += 1;
        existing.current = to_store.clone();
        if changed {
            existing.changelog.push(ChangelogEntry {
                at: now,
                word_count: to_store.metadata.word_count,
                diff: Some(content_diff.clone()),
            });
        }

        let diff = if changed { Some(content_diff) } else { None };
        Ok((existing, false, changed, word_count_delta, diff))
    } else {
        let clean_url = url::Url::parse(url)
            .ok()
            .map(|mut parsed| {
                parsed.set_query(None);
                parsed.to_string()
            })
            .unwrap_or_else(|| url.to_string());
        Ok((
            Sidecar {
                schema_version: 1,
                url: clean_url,
                first_seen: now,
                last_fetched: now,
                fetch_count: 1,
                changelog: vec![ChangelogEntry {
                    at: now,
                    word_count: to_store.metadata.word_count,
                    diff: None,
                }],
                current: to_store.clone(),
            },
            true,
            false,
            0,
            None,
        ))
    }
}

fn cap_changelog(sidecar: &mut Sidecar, max_entries: Option<usize>) {
    if let Some(cap) = max_entries {
        let cap = cap.max(1);
        if sidecar.changelog.len() > cap {
            let excess = sidecar.changelog.len() - cap;
            sidecar.changelog.drain(1..1 + excess);
        }
    }
}

async fn write_sidecar_files(
    md_path: &PathBuf,
    json_path: &PathBuf,
    to_store: &noxa_core::ExtractionResult,
    json_bytes: &[u8],
    write_markdown: bool,
) -> Result<(), StoreError> {
    let rand_suffix = {
        use rand::Rng;
        format!("{:016x}", rand::thread_rng().r#gen::<u64>())
    };

    if write_markdown {
        let markdown_bytes = to_store.content.markdown.as_bytes().to_vec();
        let tmp_md = md_path.with_extension(format!("md.{rand_suffix}.tmp"));
        tokio::fs::write(&tmp_md, &markdown_bytes).await?;
        #[cfg(unix)]
        set_file_permissions(&tmp_md)?;
        tokio::fs::rename(&tmp_md, md_path).await?;
    }

    let tmp_json = json_path.with_extension(format!("json.{rand_suffix}.tmp"));
    tokio::fs::write(&tmp_json, json_bytes).await?;
    #[cfg(unix)]
    set_file_permissions(&tmp_json)?;
    tokio::fs::rename(&tmp_json, json_path).await?;
    Ok(())
}
