use std::collections::HashSet;

use serde_json::Value;
use url::Url;

use crate::RagError;

use super::{
    BridgeDocument, McpBridge, McpSource, McporterExecutor, SyncReport, WriteStatus,
    array_field, as_u64_value, join_non_empty, optional_string, required_string, required_value,
    string_array,
    io::{build_extraction, write_bridge_document},
};

impl<E> McpBridge<E>
where
    E: McporterExecutor,
{
    pub(super) async fn sync_linkding(&self) -> Result<SyncReport, RagError> {
        let mut report = SyncReport::default();
        let mut offset = 0_u32;
        let mut seen_ids = HashSet::new();

        loop {
            let data = self
                .call_data(
                    McpSource::Linkding,
                    "bookmark.list",
                    serde_json::json!({
                        "limit": self.config.page_size,
                        "offset": offset,
                    }),
                )
                .await?;
            let records = array_field(&data, "results")?;
            if records.is_empty() {
                break;
            }

            let mut new_records = 0_usize;
            for record in records {
                let document =
                    normalize_linkding_record(record, self.config.platform_base_url.as_deref())?;
                if !seen_ids.insert(document.external_id.clone()) {
                    continue;
                }
                new_records += 1;
                report.fetched += 1;
                match write_bridge_document(&self.config.watch_dir, &document).await? {
                    WriteStatus::Written => report.written += 1,
                    WriteStatus::Unchanged => report.skipped += 1,
                }
            }

            if data.get("next").is_none() || data.get("next").is_some_and(Value::is_null) {
                break;
            }
            if new_records == 0 {
                break;
            }
            offset = offset.saturating_add(self.config.page_size.max(1));
        }

        Ok(report)
    }
}

pub fn normalize_linkding_record(
    record: &Value,
    platform_base_url: Option<&str>,
) -> Result<BridgeDocument, RagError> {
    let id = required_value(record, "id").and_then(as_u64_value)?;
    let url = required_string(record, "url")?;
    let title = optional_string(record, "title");
    let description = optional_string(record, "description");
    let notes = optional_string(record, "notes");
    let markdown = join_non_empty([
        title.as_deref().map(|value| format!("# {value}")),
        description.clone(),
        notes.clone(),
    ]);
    let plain_text = join_non_empty([title.clone(), description, notes]);
    let platform_url = match platform_base_url {
        Some(base) => Some(linkding_platform_url(base, &url)?),
        None => None,
    };

    Ok(BridgeDocument {
        source: McpSource::Linkding,
        external_id: format!("linkding:{id}"),
        platform_url,
        extraction: build_extraction(
            url,
            title,
            optional_string(record, "date_added"),
            None,
            None,
            string_array(record.get("tag_names")),
            markdown,
            plain_text,
        ),
    })
}

fn linkding_platform_url(base: &str, bookmark_url: &str) -> Result<String, RagError> {
    let mut url = Url::parse(base)
        .map_err(|e| RagError::Parse(format!("invalid linkding base URL {base:?}: {e}")))?;
    let current_path = url.path().trim_end_matches('/');
    let next_path = if current_path.is_empty() {
        "/bookmarks".to_string()
    } else {
        format!("{current_path}/bookmarks")
    };
    url.set_path(&next_path);
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("q", bookmark_url)
        .finish();
    url.set_query(Some(&query));
    Ok(url.to_string())
}
