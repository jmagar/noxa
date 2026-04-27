use std::collections::HashMap;

use serde_json::Value;

use crate::RagError;

use super::{
    BridgeDocument, McpBridge, McpSource, McporterExecutor, SyncReport, WriteStatus, array_field,
    as_u64, as_u64_value,
    io::{build_extraction, write_bridge_document},
    join_base_url, optional_string, required_base_url, required_value,
};

impl<E> McpBridge<E>
where
    E: McporterExecutor,
{
    pub(super) async fn sync_paperless(&self) -> Result<SyncReport, RagError> {
        let base_url = required_base_url(&self.config, McpSource::Paperless)?;
        let tag_names = self.fetch_paperless_lookup("tags.list").await?;
        let correspondent_names = self.fetch_paperless_lookup("correspondents.list").await?;
        let mut report = SyncReport::default();
        let mut page = 1_u32;

        loop {
            let data = self
                .call_data(
                    McpSource::Paperless,
                    "documents.list",
                    serde_json::json!({
                        "page_size": self.config.page_size,
                        "page": page,
                    }),
                )
                .await?;
            let records = array_field(&data, "results")?;
            if records.is_empty() {
                break;
            }

            for record in records {
                let document =
                    normalize_paperless_record(record, &tag_names, &correspondent_names, base_url)?;
                report.fetched += 1;
                match write_bridge_document(&self.config.watch_dir, &document).await? {
                    WriteStatus::Written => report.written += 1,
                    WriteStatus::Unchanged => report.skipped += 1,
                }
            }

            if data.get("next").is_none() || data.get("next").is_some_and(Value::is_null) {
                break;
            }
            page = page.saturating_add(1);
        }

        Ok(report)
    }

    pub(super) async fn fetch_paperless_lookup(
        &self,
        action: &str,
    ) -> Result<HashMap<u64, String>, RagError> {
        let data = self
            .call_data(McpSource::Paperless, action, serde_json::json!({}))
            .await?;
        let items = if let Some(array) = data.as_array() {
            array.iter().collect::<Vec<_>>()
        } else {
            array_field(&data, "results")?
        };
        let mut lookup = HashMap::new();
        for item in items {
            let Some(id) = item.get("id").and_then(as_u64) else {
                continue;
            };
            let Some(name) = item.get("name").and_then(Value::as_str) else {
                continue;
            };
            lookup.insert(id, name.to_string());
        }
        Ok(lookup)
    }
}

pub fn normalize_paperless_record(
    record: &Value,
    tag_names: &HashMap<u64, String>,
    correspondent_names: &HashMap<u64, String>,
    platform_base_url: &str,
) -> Result<BridgeDocument, RagError> {
    let id = required_value(record, "id").and_then(as_u64_value)?;
    let tags = record
        .get("tags")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(as_u64)
                .filter_map(|value| tag_names.get(&value).cloned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let author = record
        .get("correspondent")
        .and_then(as_u64)
        .and_then(|value| correspondent_names.get(&value).cloned());
    let title = optional_string(record, "title");
    let content = optional_string(record, "content").unwrap_or_default();
    let url = join_base_url(platform_base_url, &format!("/api/documents/{id}/"))?;

    Ok(BridgeDocument {
        source: McpSource::Paperless,
        external_id: format!("paperless:{id}"),
        platform_url: Some(url.clone()),
        extraction: build_extraction(
            url,
            title,
            optional_string(record, "created").or_else(|| optional_string(record, "created_date")),
            author,
            None,
            tags,
            content.clone(),
            content,
        ),
    })
}
