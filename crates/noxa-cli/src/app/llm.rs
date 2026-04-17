use super::*;

pub(crate) async fn build_llm_provider(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
) -> Result<Box<dyn LlmProvider>, String> {
    if let Some(ref name) = resolved.llm_provider {
        match name.as_str() {
            "gemini" => {
                let provider = noxa_llm::providers::gemini_cli::GeminiCliProvider::new(
                    resolved.llm_model.clone(),
                );
                if !provider.is_available().await {
                    return Err(
                        "gemini CLI not found on PATH -- install it or omit --llm-provider".into(),
                    );
                }
                Ok(Box::new(provider))
            }
            "ollama" => {
                let provider = noxa_llm::providers::ollama::OllamaProvider::new(
                    cli.llm_base_url.clone(),
                    resolved.llm_model.clone(),
                );
                if !provider.is_available().await {
                    return Err("ollama is not running or unreachable".into());
                }
                Ok(Box::new(provider))
            }
            "openai" => {
                let provider = noxa_llm::providers::openai::OpenAiProvider::new(
                    None,
                    cli.llm_base_url.clone(),
                    resolved.llm_model.clone(),
                )
                .ok_or("OPENAI_API_KEY not set")?;
                Ok(Box::new(provider))
            }
            "anthropic" => {
                let provider = noxa_llm::providers::anthropic::AnthropicProvider::new(
                    None,
                    resolved.llm_model.clone(),
                )
                .ok_or("ANTHROPIC_API_KEY not set")?;
                Ok(Box::new(provider))
            }
            other => Err(format!(
                "unknown LLM provider: {other} (use gemini, ollama, openai, or anthropic)"
            )),
        }
    } else {
        let chain = noxa_llm::ProviderChain::default().await;
        if chain.is_empty() {
            return Err(
                "no LLM providers available (priority: Gemini CLI -> OpenAI -> Ollama -> Anthropic) -- install gemini on PATH, set OPENAI_API_KEY, OLLAMA_HOST / OLLAMA_MODEL, or ANTHROPIC_API_KEY"
                    .into(),
            );
        }
        Ok(Box::new(chain))
    }
}

pub(crate) async fn run_llm(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<(), String> {
    // Extract content from source first (handles PDF detection for URLs)
    let result = fetch_and_extract(cli, resolved).await?.into_extraction()?;

    let url = cli
        .urls
        .first()
        .map(|u| normalize_url(u))
        .unwrap_or_default();
    let provider = build_llm_provider(cli, resolved).await?;
    let model = resolved.llm_model.as_deref();
    let ops_log = build_ops_log(cli, resolved);

    let output_str: Option<String> = if let Some(ref schema_input) = cli.extract_json {
        // Support @file syntax for loading schema from file
        let schema_str = if let Some(path) = schema_input.strip_prefix('@') {
            std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read schema file {path}: {e}"))?
        } else {
            schema_input.clone()
        };

        let schema: serde_json::Value =
            serde_json::from_str(&schema_str).map_err(|e| format!("invalid JSON schema: {e}"))?;

        let t = std::time::Instant::now();
        let extracted = noxa_llm::extract::extract_json(
            &result.content.plain_text,
            &schema,
            provider.as_ref(),
            model,
        )
        .await
        .map_err(|e| format!("LLM extraction failed: {e}"))?;
        eprintln!("LLM: {:.1}s", t.elapsed().as_secs_f64());

        log_operation(
            &ops_log,
            &url,
            Op::Extract,
            || {
                serde_json::json!({
                    "kind": "json",
                    "schema": schema,
                    "provider": provider.name(),
                    "model": model
                })
            },
            || extracted.clone(),
        )
        .await;

        Some(serde_json::to_string_pretty(&extracted).expect("serialization failed"))
    } else if let Some(ref prompt) = cli.extract_prompt {
        let t = std::time::Instant::now();
        let extracted = noxa_llm::extract::extract_with_prompt(
            &result.content.plain_text,
            prompt,
            provider.as_ref(),
            model,
        )
        .await
        .map_err(|e| format!("LLM extraction failed: {e}"))?;
        eprintln!("LLM: {:.1}s", t.elapsed().as_secs_f64());

        log_operation(
            &ops_log,
            &url,
            Op::Extract,
            || {
                serde_json::json!({
                    "kind": "prompt",
                    "prompt": prompt,
                    "provider": provider.name(),
                    "model": model
                })
            },
            || extracted.clone(),
        )
        .await;

        Some(serde_json::to_string_pretty(&extracted).expect("serialization failed"))
    } else if let Some(sentences) = cli.summarize {
        let t = std::time::Instant::now();
        let summary = noxa_llm::summarize::summarize(
            &result.content.plain_text,
            Some(sentences),
            provider.as_ref(),
            model,
        )
        .await
        .map_err(|e| format!("LLM summarization failed: {e}"))?;
        eprintln!("LLM: {:.1}s", t.elapsed().as_secs_f64());

        log_operation(
            &ops_log,
            &url,
            Op::Summarize,
            || {
                serde_json::json!({
                    "sentences": sentences,
                    "provider": provider.name(),
                    "model": model
                })
            },
            || serde_json::Value::String(summary.clone()),
        )
        .await;

        Some(summary)
    } else {
        None
    };

    if let Some(s) = output_str {
        println!("{s}");
    }

    Ok(())
}

/// Batch LLM extraction: fetch each URL, run LLM on extracted content, save/print results.
/// URLs are processed sequentially to respect LLM provider rate limits.
pub(crate) async fn run_batch_llm(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    entries: &[(String, Option<String>)],
) -> Result<(), String> {
    let client = FetchClient::new(build_fetch_config(cli, resolved))
        .map_err(|e| format!("client error: {e}"))?;
    let options = build_extraction_options(resolved);
    let provider = build_llm_provider(cli, resolved).await?;
    let model = resolved.llm_model.as_deref();
    let ops_log = client.ops_log().cloned();

    // Pre-parse schema once if --extract-json is used
    let schema = if let Some(ref schema_input) = cli.extract_json {
        let schema_str = if let Some(path) = schema_input.strip_prefix('@') {
            std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read schema file {path}: {e}"))?
        } else {
            schema_input.clone()
        };
        Some(
            serde_json::from_str::<serde_json::Value>(&schema_str)
                .map_err(|e| format!("invalid JSON schema: {e}"))?,
        )
    } else {
        None
    };

    let total = entries.len();
    let mut ok = 0usize;
    let mut errors = 0usize;
    for (i, (url, _)) in entries.iter().enumerate() {
        let idx = i + 1;
        eprint!("[{idx}/{total}] {url} ");

        // Fetch and extract page content
        let extraction = match client.fetch_and_extract_with_options(url, &options).await {
            Ok(r) => r,
            Err(e) => {
                errors += 1;
                let msg = format!("fetch failed: {e}");
                eprintln!("-> error: {msg}");
                continue;
            }
        };

        let text = &extraction.content.plain_text;

        // Run the appropriate LLM operation
        let llm_start = std::time::Instant::now();
        let llm_result = if let Some(ref schema) = schema {
            noxa_llm::extract::extract_json(text, schema, provider.as_ref(), model)
                .await
                .map(LlmOutput::Json)
        } else if let Some(ref prompt) = cli.extract_prompt {
            noxa_llm::extract::extract_with_prompt(text, prompt, provider.as_ref(), model)
                .await
                .map(LlmOutput::Json)
        } else if let Some(sentences) = cli.summarize {
            noxa_llm::summarize::summarize(text, Some(sentences), provider.as_ref(), model)
                .await
                .map(LlmOutput::Text)
        } else {
            unreachable!("run_batch_llm called without LLM flags")
        };
        let llm_elapsed = llm_start.elapsed();

        match llm_result {
            Ok(output) => {
                ok += 1;

                let output_str = match &output {
                    LlmOutput::Json(v) => {
                        serde_json::to_string_pretty(v).expect("serialization failed")
                    }
                    LlmOutput::Text(s) => s.clone(),
                };

                // Count top-level fields/items for progress display
                let detail = match &output {
                    LlmOutput::Json(v) => match v {
                        serde_json::Value::Object(m) => format!("{} fields", m.len()),
                        serde_json::Value::Array(a) => format!("{} items", a.len()),
                        _ => "done".to_string(),
                    },
                    LlmOutput::Text(s) => {
                        let words = s.split_whitespace().count();
                        format!("{words} words")
                    }
                };
                eprintln!("-> extracted {detail} ({:.1}s)", llm_elapsed.as_secs_f64());

                // Append to ops log.
                {
                    let (op, log_input, log_output) = if let Some(ref schema) = schema {
                        (
                            Op::Extract,
                            serde_json::json!({ "kind": "json", "schema": schema, "provider": provider.name(), "model": model }),
                            match &output {
                                LlmOutput::Json(v) => v.clone(),
                                LlmOutput::Text(s) => serde_json::Value::String(s.clone()),
                            },
                        )
                    } else if let Some(ref prompt) = cli.extract_prompt {
                        (
                            Op::Extract,
                            serde_json::json!({ "kind": "prompt", "prompt": prompt, "provider": provider.name(), "model": model }),
                            match &output {
                                LlmOutput::Json(v) => v.clone(),
                                LlmOutput::Text(s) => serde_json::Value::String(s.clone()),
                            },
                        )
                    } else {
                        let sentences = cli.summarize.unwrap_or(3);
                        (
                            Op::Summarize,
                            serde_json::json!({ "sentences": sentences, "provider": provider.name(), "model": model }),
                            match &output {
                                LlmOutput::Text(s) => serde_json::Value::String(s.clone()),
                                LlmOutput::Json(v) => v.clone(),
                            },
                        )
                    };
                    log_operation(&ops_log, url, op, || log_input, || log_output).await;
                }

                println!("--- {url}");
                println!("{output_str}");
                println!();
            }
            Err(e) => {
                errors += 1;
                let msg = format!("LLM extraction failed: {e}");
                eprintln!("-> error: {msg}");
            }
        }
    }

    eprintln!("Processed {total} URLs ({ok} ok, {errors} errors)");

    if let Some(ref webhook_url) = cli.webhook {
        fire_webhook(
            webhook_url,
            &serde_json::json!({
                "event": "batch_llm_complete",
                "total": total,
                "ok": ok,
                "errors": errors,
            }),
        );
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    if errors > 0 {
        Err(format!("{errors} of {total} URLs failed"))
    } else {
        Ok(())
    }
}

/// Intermediate type to hold LLM output before formatting.
pub(crate) enum LlmOutput {
    Json(serde_json::Value),
    Text(String),
}

/// Returns true if any LLM flag is set.
pub(crate) fn has_llm_flags(cli: &Cli) -> bool {
    cli.extract_json.is_some() || cli.extract_prompt.is_some() || cli.summarize.is_some()
}
