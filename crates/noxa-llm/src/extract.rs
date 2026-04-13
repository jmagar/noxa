/// Schema-based and prompt-based LLM extraction.
/// Both functions build a system prompt, send content to the LLM, and parse JSON back.
use jsonschema;

use crate::clean::strip_thinking_tags;
use crate::error::LlmError;
use crate::provider::{CompletionRequest, LlmProvider, Message};

/// Validate a JSON value against a schema. Returns Ok(()) on success or
/// Err(LlmError::InvalidJson) with a concise error message on failure.
fn validate_schema(value: &serde_json::Value, schema: &serde_json::Value) -> Result<(), LlmError> {
    let compiled = jsonschema::validator_for(schema)
        .map_err(|e| LlmError::InvalidJson(format!("invalid schema: {e}")))?;

    let first_error = compiled.iter_errors(value).next();

    match first_error {
        None => Ok(()),
        Some(e) => {
            let msg = format!("{} at {}", e, e.instance_path());
            Err(LlmError::InvalidJson(format!(
                "schema validation failed: {msg}"
            )))
        }
    }
}

/// Compile a schema up front so invalid schemas fail before any provider call.
fn validate_schema_definition(schema: &serde_json::Value) -> Result<(), LlmError> {
    jsonschema::validator_for(schema)
        .map(|_| ())
        .map_err(|e| LlmError::InvalidJson(format!("invalid schema: {e}")))
}

/// Build a targeted correction prompt from a schema validation failure.
///
/// Extracts the instance path and the schema keyword that failed (e.g. "type",
/// "required") and formats them into a short instruction under 200 chars.
/// Raw model output and web content are intentionally excluded — the caller
/// must NOT pass them here.
fn build_schema_correction_prompt(value: &serde_json::Value, schema: &serde_json::Value) -> String {
    let Ok(compiled) = jsonschema::validator_for(schema) else {
        return "Return ONLY corrected JSON matching the schema.".to_string();
    };

    let correction = compiled.iter_errors(value).next().map(|e| {
        let path = e.instance_path().to_string();
        let keyword = e.kind().keyword();
        if path.is_empty() || path == "/" {
            format!("Field failed '{}' check. Return ONLY corrected JSON.", keyword)
        } else {
            format!("Field '{}' failed '{}' check. Return ONLY corrected JSON.", path, keyword)
        }
    }).unwrap_or_else(|| "Return ONLY corrected JSON matching the schema.".to_string());

    // Hard cap at 200 chars — schema errors should never need more than this.
    if correction.len() > 200 {
        correction[..200].to_string()
    } else {
        correction
    }
}

/// Extract structured JSON from content using a JSON schema.
/// The schema tells the LLM exactly what fields to extract and their types.
///
/// Retry policy:
/// - If the response cannot be parsed as JSON: retry once with a terse correction prompt.
/// - If the response is valid JSON but fails schema validation: retry once with
///   a correction prompt containing only the field path and keyword that failed.
/// - The correction prompt is capped at 200 chars and never embeds raw model
///   output or web content, preventing token overflow and schema leakage.
pub async fn extract_json(
    content: &str,
    schema: &serde_json::Value,
    provider: &dyn LlmProvider,
    model: Option<&str>,
) -> Result<serde_json::Value, LlmError> {
    validate_schema_definition(schema)?;

    let system = format!(
        "You are a JSON extraction engine. Extract data from the content according to this schema.\n\
         Return ONLY valid JSON matching the schema. No explanations, no markdown, no commentary.\n\n\
         Schema:\n```json\n{}\n```",
        serde_json::to_string_pretty(schema).unwrap_or_else(|_| schema.to_string())
    );

    let mut messages = vec![
        Message {
            role: "system".into(),
            content: system,
        },
        Message {
            role: "user".into(),
            content: content.to_string(),
        },
    ];

    let mut request = CompletionRequest {
        model: model.unwrap_or_default().to_string(),
        messages: messages.clone(),
        temperature: Some(0.0),
        max_tokens: None,
        json_mode: true,
    };

    let response = provider.complete(&request).await?;

    match parse_and_validate(&response, schema) {
        Ok(value) => Ok(value),
        Err(_) => {
            // First attempt failed — retry once with a targeted correction prompt.
            //
            // IMPORTANT: Do NOT embed raw model output or web content here.
            // For schema mismatches, extract path + keyword from the parsed value
            // so the correction is precise. For parse failures, use a terse generic
            // message. Both stay under 200 chars.
            let correction_prompt = match parse_json_response(&response) {
                Ok(parsed_value) => {
                    // Valid JSON but schema mismatch — extract specific field info.
                    build_schema_correction_prompt(&parsed_value, schema)
                }
                Err(_) => {
                    // Unparseable JSON — terse generic correction.
                    "Your response was not valid JSON. Return ONLY valid JSON matching the schema."
                        .to_string()
                }
            };

            // Push only the correction message — raw model output is excluded
            // to prevent token overflow and avoid reinforcing wrong patterns.
            messages.push(Message {
                role: "user".into(),
                content: correction_prompt,
            });

            request.messages = messages;
            let retry_response = provider.complete(&request).await?;
            parse_and_validate(&retry_response, schema)
        }
    }
}

/// Helper: parse response string as JSON and validate it against the schema.
fn parse_and_validate(
    response: &str,
    schema: &serde_json::Value,
) -> Result<serde_json::Value, LlmError> {
    let value = parse_json_response(response)?;
    validate_schema(&value, schema)?;
    Ok(value)
}

/// Extract information using a natural language prompt.
/// More flexible than schema extraction — the user describes what they want.
pub async fn extract_with_prompt(
    content: &str,
    prompt: &str,
    provider: &dyn LlmProvider,
    model: Option<&str>,
) -> Result<serde_json::Value, LlmError> {
    let system = format!(
        "You are a JSON extraction engine. Extract information from the content based on these instructions.\n\
         Return ONLY valid JSON. No explanations, no markdown, no commentary.\n\n\
         Instructions: {prompt}"
    );

    let request = CompletionRequest {
        model: model.unwrap_or_default().to_string(),
        messages: vec![
            Message {
                role: "system".into(),
                content: system,
            },
            Message {
                role: "user".into(),
                content: content.to_string(),
            },
        ],
        temperature: Some(0.0),
        max_tokens: None,
        json_mode: true,
    };

    let response = provider.complete(&request).await?;
    parse_json_response(&response)
}

/// Parse an LLM response string as JSON. Handles common edge cases:
/// - Thinking tags (`<think>...</think>`)
/// - Markdown code fences (```json ... ```)
/// - Leading/trailing whitespace
fn parse_json_response(response: &str) -> Result<serde_json::Value, LlmError> {
    // Strip thinking tags before any JSON parsing — providers already do this,
    // but defense in depth for any caller that bypasses the provider layer
    let cleaned = strip_thinking_tags(response);
    let trimmed = cleaned.trim();

    // Strip markdown code fences if present
    let json_str = if trimmed.starts_with("```") {
        let without_opener = trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```"))
            .unwrap_or(trimmed);
        without_opener
            .strip_suffix("```")
            .unwrap_or(without_opener)
            .trim()
    } else {
        trimmed
    };

    serde_json::from_str(json_str)
        .map_err(|e| LlmError::InvalidJson(format!("{e} — raw response: {response}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock::MockProvider;

    #[test]
    fn parse_clean_json() {
        let result = parse_json_response(r#"{"name": "Rust", "version": 2024}"#).unwrap();
        assert_eq!(result["name"], "Rust");
        assert_eq!(result["version"], 2024);
    }

    #[test]
    fn parse_json_with_code_fence() {
        let response = "```json\n{\"key\": \"value\"}\n```";
        let result = parse_json_response(response).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn parse_json_with_whitespace() {
        let response = "  \n  {\"ok\": true}  \n  ";
        let result = parse_json_response(response).unwrap();
        assert_eq!(result["ok"], true);
    }

    #[test]
    fn parse_invalid_json() {
        let result = parse_json_response("not json at all");
        assert!(matches!(result, Err(LlmError::InvalidJson(_))));
    }

    #[test]
    fn parse_json_with_thinking_tags() {
        let response = "<think>analyzing the content</think>{\"title\": \"Hello\"}";
        let result = parse_json_response(response).unwrap();
        assert_eq!(result["title"], "Hello");
    }

    #[test]
    fn parse_json_with_thinking_and_code_fence() {
        let response = "<think>let me think</think>\n```json\n{\"key\": \"value\"}\n```";
        let result = parse_json_response(response).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[tokio::test]
    async fn extract_json_uses_schema_in_prompt() {
        let mock = MockProvider::ok(r#"{"title": "Test Article", "author": "Jane"}"#);

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "author": { "type": "string" }
            }
        });

        let result = extract_json("Some article content by Jane", &schema, &mock, None)
            .await
            .unwrap();

        assert_eq!(result["title"], "Test Article");
        assert_eq!(result["author"], "Jane");
    }

    #[tokio::test]
    async fn extract_with_prompt_returns_json() {
        let mock = MockProvider::ok(r#"{"emails": ["test@example.com"]}"#);

        let result = extract_with_prompt(
            "Contact us at test@example.com",
            "Find all email addresses",
            &mock,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result["emails"][0], "test@example.com");
    }

    // ── Schema validation ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn schema_validation_passes_for_matching_json() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["price"],
            "properties": {
                "price": { "type": "number" }
            }
        });
        let mock = MockProvider::ok(r#"{"price": 9.99}"#);
        let result = extract_json("content", &schema, &mock, None).await.unwrap();
        assert_eq!(result["price"], 9.99);
    }

    #[tokio::test]
    async fn schema_validation_fails_for_wrong_type() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["price"],
            "properties": {
                "price": { "type": "number" }
            }
        });
        // Model returns valid JSON but wrong type ("string" instead of number).
        // Retry fires with a schema-aware correction prompt, but MockProvider returns
        // the same bad JSON again — both attempts fail, so the result is InvalidJson.
        let mock = MockProvider::ok(r#"{"price": "not-a-number"}"#);
        let result = extract_json("content", &schema, &mock, None).await;
        assert!(
            matches!(result, Err(LlmError::InvalidJson(_))),
            "expected InvalidJson after both attempts return wrong type, got {result:?}"
        );
    }

    #[tokio::test]
    async fn schema_validation_fails_for_missing_required_field() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["title"],
            "properties": {
                "title": { "type": "string" }
            }
        });
        let mock = MockProvider::ok(r#"{"other": "value"}"#);
        let result = extract_json("content", &schema, &mock, None).await;
        assert!(matches!(result, Err(LlmError::InvalidJson(_))));
    }

    #[tokio::test]
    async fn parse_failure_triggers_one_retry() {
        use crate::testing::mock::SequenceMockProvider;

        let schema = serde_json::json!({
            "type": "object",
            "properties": { "title": { "type": "string" } }
        });

        // First call: unparseable JSON. Second call: valid JSON matching schema.
        let mock = SequenceMockProvider::new(
            "mock-seq",
            vec![
                Ok("this is not json at all".to_string()),
                Ok(r#"{"title": "Retry succeeded"}"#.to_string()),
            ],
        );

        let result = extract_json("content", &schema, &mock, None).await.unwrap();
        assert_eq!(result["title"], "Retry succeeded");
    }

    #[tokio::test]
    async fn both_attempts_fail_returns_invalid_json() {
        use crate::testing::mock::SequenceMockProvider;

        let schema = serde_json::json!({
            "type": "object",
            "properties": { "title": { "type": "string" } }
        });

        let mock = SequenceMockProvider::new(
            "mock-seq",
            vec![Ok("not json".to_string()), Ok("also not json".to_string())],
        );

        let result = extract_json("content", &schema, &mock, None).await;
        assert!(
            matches!(result, Err(LlmError::InvalidJson(_))),
            "expected InvalidJson after both attempts fail"
        );
    }

    #[tokio::test]
    async fn schema_mismatch_triggers_retry() {
        use crate::testing::mock::SequenceMockProvider;

        let schema = serde_json::json!({
            "type": "object",
            "required": ["price"],
            "properties": {
                "price": { "type": "number" }
            }
        });

        // First call: valid JSON but schema mismatch (price is string).
        // Second call: valid JSON matching schema.
        let mock = SequenceMockProvider::new(
            "mock-seq",
            vec![
                Ok(r#"{"price": "wrong-type"}"#.to_string()),
                Ok(r#"{"price": 9.99}"#.to_string()),
            ],
        );

        let result = extract_json("content", &schema, &mock, None).await.unwrap();
        assert_eq!(result["price"], 9.99);
    }

    // ── Correction prompt unit tests ───────────────────────────────────────────

    /// Correction prompt for a type mismatch must include the field path and
    /// the failing keyword, and must stay under 200 chars.
    #[test]
    fn correction_prompt_includes_field_path_and_keyword() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "price": { "type": "integer" }
            }
        });
        // Provide a string where integer is expected.
        let value = serde_json::json!({"price": "wrong"});
        let prompt = build_schema_correction_prompt(&value, &schema);

        // Must mention the failing field path.
        assert!(
            prompt.contains("price"),
            "expected field path in correction prompt, got: {prompt:?}"
        );
        // Must mention the schema keyword.
        assert!(
            prompt.contains("type"),
            "expected schema keyword in correction prompt, got: {prompt:?}"
        );
        // Must stay under 200 chars — hard cap enforced by the function.
        assert!(
            prompt.len() <= 200,
            "correction prompt exceeded 200 chars: {} chars",
            prompt.len()
        );
        // Must NOT contain raw model output or web content markers.
        assert!(
            !prompt.contains("wrong"),
            "correction prompt must not embed the invalid value, got: {prompt:?}"
        );
    }

    /// Correction prompt for a missing required field must mention the
    /// 'required' keyword and stay under 200 chars.
    #[test]
    fn correction_prompt_for_missing_required_field() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["title"],
            "properties": {
                "title": { "type": "string" }
            }
        });
        let value = serde_json::json!({"other": "data"});
        let prompt = build_schema_correction_prompt(&value, &schema);

        assert!(
            prompt.len() <= 200,
            "correction prompt exceeded 200 chars: {} chars",
            prompt.len()
        );
        // 'required' keyword surfaced for missing required properties.
        assert!(
            prompt.contains("required"),
            "expected 'required' keyword in prompt, got: {prompt:?}"
        );
    }

    /// The retry message must not embed the raw model response.
    /// We verify this by checking that a very long/distinctive model output
    /// does not appear in any message sent during the retry call.
    #[tokio::test]
    async fn retry_prompt_does_not_embed_raw_model_output() {
        use std::sync::{Arc, Mutex};
        use async_trait::async_trait;
        use crate::provider::{CompletionRequest, LlmProvider};

        /// A mock that records every request it receives.
        struct RecordingProvider {
            responses: Vec<String>,
            call_count: Arc<Mutex<usize>>,
            recorded_messages: Arc<Mutex<Vec<Vec<crate::provider::Message>>>>,
        }

        #[async_trait]
        impl LlmProvider for RecordingProvider {
            async fn complete(&self, request: &CompletionRequest) -> Result<String, LlmError> {
                let mut count = self.call_count.lock().unwrap();
                let idx = (*count).min(self.responses.len() - 1);
                *count += 1;
                self.recorded_messages
                    .lock()
                    .unwrap()
                    .push(request.messages.clone());
                Ok(self.responses[idx].clone())
            }
            async fn is_available(&self) -> bool { true }
            fn name(&self) -> &str { "recording-mock" }
        }

        // A distinctive raw model output that must NOT appear in the retry prompt.
        let raw_model_output = r#"{"price": "DISTINCTIVE_BAD_VALUE_DO_NOT_RELAY"}"#;

        let recorded = Arc::new(Mutex::new(Vec::<Vec<crate::provider::Message>>::new()));
        let mock = RecordingProvider {
            responses: vec![
                raw_model_output.to_string(),
                r#"{"price": 9.99}"#.to_string(),
            ],
            call_count: Arc::new(Mutex::new(0)),
            recorded_messages: recorded.clone(),
        };

        let schema = serde_json::json!({
            "type": "object",
            "required": ["price"],
            "properties": { "price": { "type": "number" } }
        });

        let result = extract_json("some content", &schema, &mock, None).await.unwrap();
        assert_eq!(result["price"], 9.99);

        // Inspect the messages sent on the second (retry) call.
        let all_calls = recorded.lock().unwrap();
        assert_eq!(all_calls.len(), 2, "expected exactly 2 provider calls");

        let retry_messages = &all_calls[1];
        for msg in retry_messages {
            assert!(
                !msg.content.contains("DISTINCTIVE_BAD_VALUE_DO_NOT_RELAY"),
                "raw model output leaked into retry message role={}: {:?}",
                msg.role,
                msg.content
            );
        }
    }
}
