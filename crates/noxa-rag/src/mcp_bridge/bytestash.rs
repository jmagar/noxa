use serde_json::Value;

use crate::RagError;

use super::{
    BridgeDocument, McpBridge, McpSource, McporterExecutor, SyncReport, WriteStatus,
    array_field, join_base_url, join_non_empty, optional_string, required_base_url,
    required_string, string_array,
    io::{build_extraction, write_bridge_document},
};

impl<E> McpBridge<E>
where
    E: McporterExecutor,
{
    pub(super) async fn sync_bytestash(&self) -> Result<SyncReport, RagError> {
        let base_url = required_base_url(&self.config, McpSource::Bytestash)?;
        let data = self
            .call_data(McpSource::Bytestash, "snippets.list", serde_json::json!({}))
            .await?;
        let records = if let Some(array) = data.as_array() {
            array.iter().collect::<Vec<_>>()
        } else {
            array_field(&data, "snippets")?
        };

        let mut report = SyncReport::default();
        for record in records {
            let document = normalize_bytestash_record(record, base_url)?;
            report.fetched += 1;
            match write_bridge_document(&self.config.watch_dir, &document).await? {
                WriteStatus::Written => report.written += 1,
                WriteStatus::Unchanged => report.skipped += 1,
            }
        }

        Ok(report)
    }
}

pub fn normalize_bytestash_record(
    record: &Value,
    platform_base_url: &str,
) -> Result<BridgeDocument, RagError> {
    let id = required_string(record, "id")?;
    let title = optional_string(record, "title");
    let description = optional_string(record, "description");
    let language = optional_string(record, "language");
    let fragments = record
        .get("fragments")
        .and_then(Value::as_array)
        .ok_or_else(|| RagError::Parse("bytestash record missing fragments array".to_string()))?;

    let mut markdown_parts = Vec::new();
    if let Some(value) = title.as_deref() {
        markdown_parts.push(format!("# {value}"));
    }
    if let Some(value) = description.as_deref() {
        markdown_parts.push(value.to_string());
    }
    for fragment in fragments {
        let file_name = fragment
            .get("fileName")
            .or_else(|| fragment.get("file_name"))
            .and_then(Value::as_str)
            .unwrap_or("snippet");
        let code = fragment
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or_default();
        markdown_parts.push(format!(
            "## {file_name}\n```{}\n{}\n```",
            language.clone().unwrap_or_default(),
            code
        ));
    }
    let plain_text = join_non_empty([
        title.clone(),
        description.clone(),
        Some(
            fragments
                .iter()
                .filter_map(|fragment| fragment.get("code").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n\n"),
        ),
    ]);
    let url = join_base_url(platform_base_url, &format!("/api/snippets/{id}"))?;

    Ok(BridgeDocument {
        source: McpSource::Bytestash,
        external_id: format!("bytestash:{id}"),
        platform_url: Some(url.clone()),
        extraction: build_extraction(
            url,
            title,
            None,
            None,
            language,
            string_array(record.get("categories")),
            markdown_parts.join("\n\n"),
            plain_text,
        ),
    })
}
