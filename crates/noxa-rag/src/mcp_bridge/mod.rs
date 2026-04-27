use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use url::Url;

use crate::RagError;

mod bytestash;
pub mod executor;
mod io;
mod linkding;
mod memos;
mod paperless;

pub use bytestash::normalize_bytestash_record;
pub use executor::ProcessMcporterExecutor;
pub use io::{relative_output_path, write_bridge_document};
pub use linkding::normalize_linkding_record;
pub use memos::normalize_memo_record;
pub use paperless::normalize_paperless_record;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSource {
    Linkding,
    Memos,
    Bytestash,
    Paperless,
}

impl McpSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linkding => "linkding",
            Self::Memos => "memos",
            Self::Bytestash => "bytestash",
            Self::Paperless => "paperless",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeConfig {
    pub server: String,
    /// Directory where bridge JSON files are written. Must be one of the dirs
    /// listed in the daemon's `source.watch_dirs` so the daemon picks them up.
    pub watch_dir: PathBuf,
    pub page_size: u32,
    pub platform_base_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub fetched: usize,
    pub written: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone)]
pub struct BridgeDocument {
    pub source: McpSource,
    pub external_id: String,
    pub platform_url: Option<String>,
    pub extraction: noxa_core::ExtractionResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteStatus {
    Written,
    Unchanged,
}

#[async_trait]
pub trait McporterExecutor: Send + Sync {
    async fn call(
        &self,
        server: &str,
        service: McpSource,
        action: &str,
        params: Value,
    ) -> Result<Value, RagError>;
}

pub struct McpBridge<E> {
    executor: E,
    config: BridgeConfig,
}

impl<E> McpBridge<E>
where
    E: McporterExecutor,
{
    pub fn new(executor: E, config: BridgeConfig) -> Self {
        Self { executor, config }
    }

    pub async fn sync(&self, source: McpSource) -> Result<SyncReport, RagError> {
        match source {
            McpSource::Linkding => self.sync_linkding().await,
            McpSource::Memos => self.sync_memos().await,
            McpSource::Bytestash => self.sync_bytestash().await,
            McpSource::Paperless => self.sync_paperless().await,
        }
    }

    pub(self) async fn call_data(
        &self,
        source: McpSource,
        action: &str,
        params: Value,
    ) -> Result<Value, RagError> {
        let raw = self
            .executor
            .call(&self.config.server, source, action, params)
            .await?;
        extract_mcporter_data(raw)
    }
}

// ---------------------------------------------------------------------------
// Shared helper functions (used by platform submodules via `use super::...`)
// ---------------------------------------------------------------------------

pub(super) fn extract_mcporter_data(raw: Value) -> Result<Value, RagError> {
    let ok = raw.get("ok").and_then(Value::as_bool).unwrap_or(true);
    if ok {
        raw.get("data")
            .cloned()
            .ok_or_else(|| RagError::Parse("mcporter response missing data".to_string()))
    } else {
        let message = raw
            .get("error")
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("unknown mcporter error");
        Err(RagError::Generic(message.to_string()))
    }
}

pub(super) fn array_field<'a>(value: &'a Value, key: &str) -> Result<Vec<&'a Value>, RagError> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().collect::<Vec<_>>())
        .ok_or_else(|| RagError::Parse(format!("expected array field {key}")))
}

pub(super) fn required_base_url(
    config: &BridgeConfig,
    source: McpSource,
) -> Result<&str, RagError> {
    config.platform_base_url.as_deref().ok_or_else(|| {
        RagError::Config(format!("{} requires --platform-base-url", source.as_str()))
    })
}

pub(super) fn join_base_url(base: &str, path: &str) -> Result<String, RagError> {
    let base = base.trim_end_matches('/');
    let url = format!("{base}{path}");
    Url::parse(&url)
        .map(|parsed| parsed.to_string())
        .map_err(|e| RagError::Parse(format!("invalid base URL {base:?}: {e}")))
}

pub(super) fn required_value<'a>(value: &'a Value, key: &str) -> Result<&'a Value, RagError> {
    value
        .get(key)
        .ok_or_else(|| RagError::Parse(format!("missing required field {key}")))
}

pub(super) fn required_string(value: &Value, key: &str) -> Result<String, RagError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| RagError::Parse(format!("missing required string field {key}")))
}

pub(super) fn optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

pub(super) fn as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<u64>().ok()))
}

pub(super) fn as_u64_value(value: &Value) -> Result<u64, RagError> {
    as_u64(value).ok_or_else(|| RagError::Parse("expected integer id".to_string()))
}

pub(super) fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(ToOwned::to_owned).or_else(|| {
                        item.get("name")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(super) fn join_non_empty<I>(parts: I) -> String
where
    I: IntoIterator<Item = Option<String>>,
{
    parts
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(super) fn first_line_title(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.chars().take(80).collect::<String>())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};

    use super::*;

    type ExecutorCall = (String, McpSource, String, Value);

    #[derive(Clone, Default)]
    struct MockExecutor {
        calls: Arc<Mutex<Vec<ExecutorCall>>>,
        responses: Arc<Mutex<VecDeque<Result<Value, RagError>>>>,
    }

    impl MockExecutor {
        fn with_responses(responses: Vec<Result<Value, RagError>>) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            }
        }

        fn calls(&self) -> Vec<ExecutorCall> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    #[async_trait]
    impl McporterExecutor for MockExecutor {
        async fn call(
            &self,
            server: &str,
            service: McpSource,
            action: &str,
            params: Value,
        ) -> Result<Value, RagError> {
            self.calls.lock().expect("calls lock").push((
                server.to_string(),
                service,
                action.to_string(),
                params,
            ));
            self.responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .expect("response queued")
        }
    }

    #[test]
    fn relative_output_path_sanitizes_external_ids() {
        let path = relative_output_path(McpSource::Memos, "memos/G3BLCD3swV4Fuxk4uMT97d");

        let rendered = path.display().to_string();
        assert!(rendered.starts_with("mcp/memos/memos_G3BLCD3swV4Fuxk4uMT97d-"));
        assert!(rendered.ends_with(".json"));
    }

    #[test]
    fn relative_output_path_distinguishes_colliding_sanitized_ids() {
        let first = relative_output_path(McpSource::Memos, "memos/G3");
        let second = relative_output_path(McpSource::Memos, "memos_G3");
        assert_ne!(first, second);
    }

    #[test]
    fn normalize_linkding_record_maps_bookmark_fields() {
        let record = serde_json::json!({
            "id": 441,
            "url": "https://pipenet.dev/",
            "title": "pipenet",
            "description": "",
            "notes": "MCP client",
            "tag_names": ["mcp", "rust"],
            "date_added": "2026-02-02T15:23:27.821564-05:00"
        });

        let document =
            normalize_linkding_record(&record, Some("https://ding.tootie.tv")).expect("maps");

        assert_eq!(document.external_id, "linkding:441");
        assert_eq!(
            document.extraction.metadata.url.as_deref(),
            Some("https://pipenet.dev/")
        );
        assert_eq!(
            document.platform_url.as_deref(),
            Some("https://ding.tootie.tv/bookmarks?q=https%3A%2F%2Fpipenet.dev%2F")
        );
        assert_eq!(
            document.extraction.metadata.technologies,
            vec!["mcp".to_string(), "rust".to_string()]
        );
        assert_eq!(
            document.extraction.metadata.published_date.as_deref(),
            Some("2026-02-02T15:23:27.821564-05:00")
        );
        assert!(document.extraction.content.markdown.contains("MCP client"));
    }

    #[test]
    fn normalize_linkding_record_preserves_base_path_prefix() {
        let record = serde_json::json!({
            "id": 12,
            "url": "https://example.com/doc",
            "title": "Doc",
            "description": "",
            "notes": ""
        });

        let document =
            normalize_linkding_record(&record, Some("https://ding.example/app")).expect("maps");

        assert_eq!(
            document.platform_url.as_deref(),
            Some("https://ding.example/app/bookmarks?q=https%3A%2F%2Fexample.com%2Fdoc")
        );
    }

    #[test]
    fn normalize_memo_record_maps_tags_and_dates() {
        let record = serde_json::json!({
            "name": "memos/G3BLCD3swV4Fuxk4uMT97d",
            "content": "Use `docker inspect`.\n#docker #cmd",
            "tags": ["docker", "cmd"],
            "createTime": "2024-05-07T00:26:59Z",
            "displayTime": "2024-05-17T22:39:50Z"
        });

        let document = normalize_memo_record(&record, "https://memos.example.com").expect("maps");

        assert_eq!(document.external_id, "memos:G3BLCD3swV4Fuxk4uMT97d");
        assert_eq!(
            document.extraction.metadata.url.as_deref(),
            Some("https://memos.example.com/api/v1/memos/G3BLCD3swV4Fuxk4uMT97d")
        );
        assert_eq!(
            document.platform_url.as_deref(),
            Some("https://memos.example.com/api/v1/memos/G3BLCD3swV4Fuxk4uMT97d")
        );
        assert_eq!(
            document.extraction.metadata.published_date.as_deref(),
            Some("2024-05-17T22:39:50Z")
        );
        assert_eq!(
            document.extraction.metadata.technologies,
            vec!["docker".to_string(), "cmd".to_string()]
        );
    }

    #[test]
    fn normalize_bytestash_record_maps_fragments_and_categories() {
        let record = serde_json::json!({
            "id": "abc123",
            "title": "docker env",
            "description": "Inspect container env vars",
            "language": "bash",
            "categories": ["docker", "ops"],
            "fragments": [
                {
                    "fileName": "inspect.sh",
                    "code": "docker inspect my-container | jq '.[0].Config.Env'"
                }
            ]
        });

        let document =
            normalize_bytestash_record(&record, "https://stash.example.com").expect("maps");

        assert_eq!(document.external_id, "bytestash:abc123");
        assert_eq!(
            document.extraction.metadata.url.as_deref(),
            Some("https://stash.example.com/api/snippets/abc123")
        );
        assert_eq!(
            document.extraction.metadata.language.as_deref(),
            Some("bash")
        );
        assert_eq!(
            document.extraction.metadata.technologies,
            vec!["docker".to_string(), "ops".to_string()]
        );
        assert!(document.extraction.content.markdown.contains("```bash\n"));
    }

    #[test]
    fn normalize_paperless_record_maps_tag_ids_and_correspondent() {
        let record = serde_json::json!({
            "id": 6,
            "title": "Alcohol Server Training Certificate",
            "content": "SERVSAFE certificate text",
            "tags": [1, 4],
            "correspondent": 9,
            "created": "2026-04-01"
        });
        let tag_names = HashMap::from([
            (1_u64, "paperless".to_string()),
            (4_u64, "certification".to_string()),
        ]);
        let correspondent_names = HashMap::from([(9_u64, "ServSafe".to_string())]);

        let document = normalize_paperless_record(
            &record,
            &tag_names,
            &correspondent_names,
            "https://paperless.example.com",
        )
        .expect("maps");

        assert_eq!(document.external_id, "paperless:6");
        assert_eq!(
            document.extraction.metadata.url.as_deref(),
            Some("https://paperless.example.com/api/documents/6/")
        );
        assert_eq!(
            document.extraction.metadata.author.as_deref(),
            Some("ServSafe")
        );
        assert_eq!(
            document.extraction.metadata.published_date.as_deref(),
            Some("2026-04-01")
        );
        assert_eq!(
            document.extraction.metadata.technologies,
            vec!["paperless".to_string(), "certification".to_string()]
        );
    }

    #[tokio::test]
    async fn write_bridge_document_skips_unchanged_payloads() {
        let temp = tempfile::tempdir().expect("tempdir");
        let document = BridgeDocument {
            source: McpSource::Linkding,
            external_id: "linkding:441".to_string(),
            platform_url: Some(
                "https://ding.tootie.tv/bookmarks?q=https%3A%2F%2Fpipenet.dev%2F".to_string(),
            ),
            extraction: io::build_extraction(
                "https://pipenet.dev/".to_string(),
                Some("pipenet".to_string()),
                Some("2026-02-02T15:23:27.821564-05:00".to_string()),
                None,
                None,
                vec!["mcp".to_string()],
                "# pipenet".to_string(),
                "pipenet".to_string(),
            ),
        };

        let first = write_bridge_document(temp.path(), &document)
            .await
            .expect("first write");
        let second = write_bridge_document(temp.path(), &document)
            .await
            .expect("second write");

        assert_eq!(first, WriteStatus::Written);
        assert_eq!(second, WriteStatus::Unchanged);
    }

    #[tokio::test]
    async fn sync_linkding_pages_and_writes_documents() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executor = MockExecutor::with_responses(vec![
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "results": [
                        {
                            "id": 1,
                            "url": "https://example.com/one",
                            "title": "One",
                            "description": "",
                            "notes": "",
                            "tag_names": ["rust"],
                            "date_added": "2026-04-01T00:00:00Z"
                        }
                    ],
                    "next": "https://ding.example/api/bookmarks/?limit=1&offset=1"
                }
            })),
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "results": [
                        {
                            "id": 2,
                            "url": "https://example.com/two",
                            "title": "Two",
                            "description": "Second",
                            "notes": "",
                            "tag_names": [],
                            "date_added": "2026-04-02T00:00:00Z"
                        }
                    ],
                    "next": null
                }
            })),
        ]);
        let bridge = McpBridge::new(
            executor.clone(),
            BridgeConfig {
                server: "lab".to_string(),
                watch_dir: temp.path().to_path_buf(),
                page_size: 1,
                platform_base_url: Some("https://ding.example".to_string()),
            },
        );

        let report = bridge.sync(McpSource::Linkding).await.expect("sync");

        assert_eq!(
            report,
            SyncReport {
                fetched: 2,
                written: 2,
                skipped: 0
            }
        );
        let calls = executor.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "lab");
        assert_eq!(calls[0].1, McpSource::Linkding);
        assert_eq!(calls[0].2, "bookmark.list");
        assert_eq!(calls[0].3, serde_json::json!({ "limit": 1, "offset": 0 }));
        assert_eq!(calls[1].3, serde_json::json!({ "limit": 1, "offset": 1 }));
        assert!(
            temp.path()
                .join(relative_output_path(McpSource::Linkding, "linkding:1"))
                .exists()
        );
        assert!(
            temp.path()
                .join(relative_output_path(McpSource::Linkding, "linkding:2"))
                .exists()
        );
    }

    #[tokio::test]
    async fn sync_linkding_stops_when_pagination_repeats_same_records() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executor = MockExecutor::with_responses(vec![
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "results": [
                        {
                            "id": 1,
                            "url": "https://example.com/one",
                            "title": "One",
                            "description": "",
                            "notes": "",
                            "tag_names": [],
                            "date_added": "2026-04-01T00:00:00Z"
                        }
                    ],
                    "next": "https://ding.example/api/bookmarks/?limit=1&offset=1"
                }
            })),
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "results": [
                        {
                            "id": 1,
                            "url": "https://example.com/one",
                            "title": "One",
                            "description": "",
                            "notes": "",
                            "tag_names": [],
                            "date_added": "2026-04-01T00:00:00Z"
                        }
                    ],
                    "next": "https://ding.example/api/bookmarks/?limit=1&offset=2"
                }
            })),
        ]);
        let bridge = McpBridge::new(
            executor.clone(),
            BridgeConfig {
                server: "lab".to_string(),
                watch_dir: temp.path().to_path_buf(),
                page_size: 1,
                platform_base_url: Some("https://ding.example".to_string()),
            },
        );

        let report = bridge.sync(McpSource::Linkding).await.expect("sync");

        assert_eq!(
            report,
            SyncReport {
                fetched: 1,
                written: 1,
                skipped: 0
            }
        );
        let calls = executor.calls();
        assert_eq!(calls.len(), 2);
        assert!(
            temp.path()
                .join(relative_output_path(McpSource::Linkding, "linkding:1"))
                .exists()
        );
    }

    #[tokio::test]
    async fn sync_memos_uses_page_tokens_and_writes_documents() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executor = MockExecutor::with_responses(vec![
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "memos": [
                        {
                            "name": "memos/first",
                            "content": "first memo",
                            "tags": ["ops"],
                            "displayTime": "2026-04-01T00:00:00Z"
                        }
                    ],
                    "nextPageToken": "page-2"
                }
            })),
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "memos": [
                        {
                            "name": "memos/second",
                            "content": "second memo",
                            "tags": [],
                            "createTime": "2026-04-02T00:00:00Z"
                        }
                    ],
                    "nextPageToken": ""
                }
            })),
        ]);
        let bridge = McpBridge::new(
            executor.clone(),
            BridgeConfig {
                server: "lab".to_string(),
                watch_dir: temp.path().to_path_buf(),
                page_size: 1,
                platform_base_url: Some("https://memos.example.com".to_string()),
            },
        );

        let report = bridge.sync(McpSource::Memos).await.expect("sync memos");

        assert_eq!(
            report,
            SyncReport {
                fetched: 2,
                written: 2,
                skipped: 0
            }
        );
        let calls = executor.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].2, "memos.list");
        assert_eq!(calls[0].3, serde_json::json!({ "page_size": 1 }));
        assert_eq!(
            calls[1].3,
            serde_json::json!({ "page_size": 1, "page_token": "page-2" })
        );
        assert!(
            temp.path()
                .join(relative_output_path(McpSource::Memos, "memos:first"))
                .exists()
        );
        assert!(
            temp.path()
                .join(relative_output_path(McpSource::Memos, "memos:second"))
                .exists()
        );
    }

    #[tokio::test]
    async fn sync_memos_stops_when_next_page_token_repeats() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executor = MockExecutor::with_responses(vec![
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "memos": [
                        {
                            "name": "memos/first",
                            "content": "first memo",
                            "tags": []
                        }
                    ],
                    "nextPageToken": "page-2"
                }
            })),
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "memos": [
                        {
                            "name": "memos/second",
                            "content": "second memo",
                            "tags": []
                        }
                    ],
                    "nextPageToken": "page-2"
                }
            })),
        ]);
        let bridge = McpBridge::new(
            executor.clone(),
            BridgeConfig {
                server: "lab".to_string(),
                watch_dir: temp.path().to_path_buf(),
                page_size: 1,
                platform_base_url: Some("https://memos.example.com".to_string()),
            },
        );

        let report = bridge.sync(McpSource::Memos).await.expect("sync memos");

        assert_eq!(
            report,
            SyncReport {
                fetched: 2,
                written: 2,
                skipped: 0
            }
        );
        assert_eq!(executor.calls().len(), 2);
    }

    #[tokio::test]
    async fn sync_bytestash_accepts_array_payloads() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executor = MockExecutor::with_responses(vec![Ok(serde_json::json!({
            "ok": true,
            "data": [
                {
                    "id": "snippet-1",
                    "title": "docker env",
                    "description": "Inspect container env vars",
                    "language": "bash",
                    "categories": ["docker"],
                    "fragments": [
                        {
                            "fileName": "inspect.sh",
                            "code": "docker inspect app"
                        }
                    ]
                }
            ]
        }))]);
        let bridge = McpBridge::new(
            executor.clone(),
            BridgeConfig {
                server: "lab".to_string(),
                watch_dir: temp.path().to_path_buf(),
                page_size: 50,
                platform_base_url: Some("https://stash.example.com".to_string()),
            },
        );

        let report = bridge
            .sync(McpSource::Bytestash)
            .await
            .expect("sync bytestash");

        assert_eq!(
            report,
            SyncReport {
                fetched: 1,
                written: 1,
                skipped: 0
            }
        );
        let calls = executor.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].2, "snippets.list");
        assert_eq!(calls[0].3, serde_json::json!({}));
        assert!(
            temp.path()
                .join(relative_output_path(
                    McpSource::Bytestash,
                    "bytestash:snippet-1"
                ))
                .exists()
        );
    }

    #[tokio::test]
    async fn sync_paperless_resolves_lookup_tables_before_documents() {
        let temp = tempfile::tempdir().expect("tempdir");
        let executor = MockExecutor::with_responses(vec![
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "results": [
                        { "id": 1, "name": "finance" }
                    ]
                }
            })),
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "results": [
                        { "id": 9, "name": "Bank" }
                    ]
                }
            })),
            Ok(serde_json::json!({
                "ok": true,
                "data": {
                    "results": [
                        {
                            "id": 17,
                            "title": "Statement",
                            "content": "balance: 100",
                            "tags": [1],
                            "correspondent": 9,
                            "created": "2026-04-01"
                        }
                    ],
                    "next": null
                }
            })),
        ]);
        let bridge = McpBridge::new(
            executor.clone(),
            BridgeConfig {
                server: "lab".to_string(),
                watch_dir: temp.path().to_path_buf(),
                page_size: 100,
                platform_base_url: Some("https://paperless.example.com".to_string()),
            },
        );

        let report = bridge
            .sync(McpSource::Paperless)
            .await
            .expect("sync paperless");

        assert_eq!(
            report,
            SyncReport {
                fetched: 1,
                written: 1,
                skipped: 0
            }
        );
        let calls = executor.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].2, "tags.list");
        assert_eq!(calls[1].2, "correspondents.list");
        assert_eq!(calls[2].2, "documents.list");
        assert_eq!(
            calls[2].3,
            serde_json::json!({ "page_size": 100, "page": 1 })
        );
        assert!(
            temp.path()
                .join(relative_output_path(McpSource::Paperless, "paperless:17"))
                .exists()
        );
    }
}
