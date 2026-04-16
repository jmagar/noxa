use chrono::{DateTime, Utc};

use crate::content_store::{ChangelogEntry, Sidecar};

pub(super) fn parse_sidecar_or_legacy(contents: &str) -> Result<Sidecar, serde_json::Error> {
    if let Ok(sidecar) = serde_json::from_str::<Sidecar>(contents) {
        return Ok(sidecar);
    }

    let extraction = serde_json::from_str::<noxa_core::ExtractionResult>(contents)?;
    let now = Utc::now();
    Ok(Sidecar {
        schema_version: 1,
        url: extraction.metadata.url.clone().unwrap_or_default(),
        first_seen: now,
        last_fetched: now,
        fetch_count: 1,
        changelog: vec![ChangelogEntry {
            at: now,
            word_count: extraction.metadata.word_count,
            diff: None,
        }],
        current: extraction,
    })
}

pub(super) fn parse_sidecar_or_migrate(
    contents: &str,
    mtime: DateTime<Utc>,
) -> Result<Sidecar, serde_json::Error> {
    if let Ok(sidecar) = serde_json::from_str::<Sidecar>(contents) {
        return Ok(sidecar);
    }

    let extraction = serde_json::from_str::<noxa_core::ExtractionResult>(contents)?;
    Ok(Sidecar {
        schema_version: 1,
        url: extraction.metadata.url.clone().unwrap_or_default(),
        first_seen: mtime,
        last_fetched: mtime,
        fetch_count: 1,
        changelog: vec![ChangelogEntry {
            at: mtime,
            word_count: extraction.metadata.word_count,
            diff: None,
        }],
        current: extraction,
    })
}
