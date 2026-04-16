use super::*;

pub(crate) async fn run_diff(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    snapshot_path: &str,
) -> Result<(), String> {
    // Load previous snapshot
    let snapshot_json = std::fs::read_to_string(snapshot_path)
        .map_err(|e| format!("failed to read snapshot {snapshot_path}: {e}"))?;
    let old: ExtractionResult = serde_json::from_str(&snapshot_json)
        .map_err(|e| format!("failed to parse snapshot JSON: {e}"))?;

    // Extract current version (handles PDF detection for URLs)
    let new_result = fetch_and_extract(cli, resolved).await?.into_extraction()?;

    let diff = noxa_core::diff::diff(&old, &new_result);

    // Append diff result to ops log.
    let ops_log = build_ops_log(cli, resolved);
    let url = cli
        .urls
        .first()
        .map(|u| normalize_url(u))
        .unwrap_or_default();
    log_operation(
        &ops_log,
        &url,
        Op::Diff,
        || serde_json::json!({ "source": "file", "snapshot": snapshot_path }),
        || serde_json::to_value(&diff).unwrap_or(serde_json::Value::Null),
    )
    .await;

    print_diff_output(&diff, &resolved.format);
    Ok(())
}

pub(crate) async fn run_brand(cli: &Cli, resolved: &config::ResolvedConfig) -> Result<(), String> {
    let result = fetch_html(cli, resolved).await?;
    let enriched = enrich_html_with_stylesheets(&result.html, &result.url).await;
    let brand = noxa_core::brand::extract_brand(
        &enriched,
        Some(result.url.as_str()).filter(|s| !s.is_empty()),
    );

    let ops_log = build_ops_log(cli, resolved);
    log_operation(
        &ops_log,
        &result.url,
        Op::Brand,
        || serde_json::json!({}),
        || serde_json::to_value(&brand).unwrap_or(serde_json::Value::Null),
    )
    .await;

    let output = serde_json::to_string_pretty(&brand).expect("serialization failed");
    println!("{output}");
    Ok(())
}
