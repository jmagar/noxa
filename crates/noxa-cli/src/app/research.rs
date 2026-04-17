use super::*;

pub(crate) async fn run_research(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
    query: &str,
) -> Result<(), String> {
    let api_key = cli
        .api_key
        .as_deref()
        .ok_or("--research requires NOXA_API_KEY (set via env or --api-key)")?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("http client error: {e}"))?;

    let mut body = serde_json::json!({ "query": query });
    if cli.deep {
        body["deep"] = serde_json::json!(true);
    }

    eprintln!("Starting research: {query}");
    if cli.deep {
        eprintln!("Deep mode enabled (longer, more thorough)");
    }

    // Start job
    let http_resp = client
        .post("https://api.noxa.io/v1/research")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API error: {e}"))?;
    let status = http_resp.status();
    let resp = http_resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("parse error (HTTP {status}): {e}"))?;
    if !status.is_success() {
        return Err(format!("API error {status}: {resp}"));
    }

    let job_id = resp
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("API did not return a job ID")?
        .to_string();

    eprintln!("Job started: {job_id}");

    // Poll
    for poll in 0..200 {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let poll_http = client
            .get(format!("https://api.noxa.io/v1/research/{job_id}"))
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| format!("poll error: {e}"))?;
        let poll_status = poll_http.status();
        let status_resp = poll_http
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("parse error (HTTP {poll_status}): {e}"))?;
        if !poll_status.is_success() {
            return Err(format!("API error {poll_status}: {status_resp}"));
        }

        let status = status_resp
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match status {
            "completed" => {
                let report = status_resp
                    .get("report")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Save full result to JSON file
                let slug: String = query
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == ' ' {
                            c
                        } else {
                            ' '
                        }
                    })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join("-")
                    .to_lowercase();
                let slug: String = slug.chars().take(50).collect();
                // Guard against empty slug (e.g., emoji-only or CJK-punctuation queries)
                // to avoid producing "research-.json".
                let filename = if slug.is_empty() {
                    format!("research-{job_id}.json")
                } else {
                    format!("research-{slug}.json")
                };

                let json = serde_json::to_string_pretty(&status_resp).unwrap_or_default();
                // Route both branches through write_to_file for consistent traversal/symlink
                // protections and error reporting; use "." as the default when no output_dir set.
                let dir = resolved
                    .output_dir
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                write_to_file(&dir, &filename, &json)?;

                let elapsed = status_resp
                    .get("elapsed_ms")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let sources = status_resp
                    .get("sources_count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let findings = status_resp
                    .get("findings_count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                eprintln!(
                    "Research complete: {sources} sources, {findings} findings, {:.1}s",
                    elapsed as f64 / 1000.0
                );
                eprintln!("Saved to: {filename}");

                // Print report to stdout
                if !report.is_empty() {
                    println!("{report}");
                }

                return Ok(());
            }
            "failed" => {
                let error = status_resp
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                return Err(format!("Research failed: {error}"));
            }
            _ => {
                if poll % 10 == 9 {
                    eprintln!("Still researching... ({:.0}s)", (poll + 1) as f64 * 3.0);
                }
            }
        }
    }

    Err(format!(
        "Research timed out after ~10 minutes. Check status: GET /v1/research/{job_id}"
    ))
}
