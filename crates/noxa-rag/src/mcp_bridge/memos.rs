use serde_json::Value;

use crate::RagError;

use super::{
    BridgeDocument, McpBridge, McpSource, McporterExecutor, SyncReport, WriteStatus,
    array_field, first_line_title, join_base_url, optional_string, required_base_url,
    required_string, string_array,
    io::{build_extraction, write_bridge_document},
};

impl<E> McpBridge<E>
where
    E: McporterExecutor,
{
    pub(super) async fn sync_memos(&self) -> Result<SyncReport, RagError> {
        let base_url = required_base_url(&self.config, McpSource::Memos)?;
        let mut report = SyncReport::default();
        let mut page_token: Option<String> = None;

        loop {
            let mut params = serde_json::json!({ "page_size": self.config.page_size });
            if let Some(token) = &page_token {
                params["page_token"] = Value::String(token.clone());
            }
            let data = self
                .call_data(McpSource::Memos, "memos.list", params)
                .await?;
            let records = array_field(&data, "memos")?;
            if records.is_empty() {
                break;
            }

            for record in records {
                let document = normalize_memo_record(record, base_url)?;
                report.fetched += 1;
                match write_bridge_document(&self.config.watch_dir, &document).await? {
                    WriteStatus::Written => report.written += 1,
                    WriteStatus::Unchanged => report.skipped += 1,
                }
            }

            let next_page_token = data
                .get("nextPageToken")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            if next_page_token.is_none() || next_page_token == page_token {
                break;
            }
            page_token = next_page_token;
        }

        Ok(report)
    }
}

pub fn normalize_memo_record(
    record: &Value,
    platform_base_url: &str,
) -> Result<BridgeDocument, RagError> {
    let name = required_string(record, "name")?;
    let memo_id = name.strip_prefix("memos/").unwrap_or(&name).to_string();
    let content = required_string(record, "content")?;
    let title = first_line_title(&content);
    let url = join_base_url(platform_base_url, &format!("/api/v1/{name}"))?;
    let published_date =
        optional_string(record, "displayTime").or_else(|| optional_string(record, "createTime"));

    Ok(BridgeDocument {
        source: McpSource::Memos,
        external_id: format!("memos:{memo_id}"),
        platform_url: Some(url.clone()),
        extraction: build_extraction(
            url,
            title,
            published_date,
            None,
            None,
            string_array(record.get("tags")),
            content.clone(),
            content,
        ),
    })
}
