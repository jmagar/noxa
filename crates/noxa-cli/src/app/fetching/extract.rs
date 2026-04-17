use crate::app::*;

pub(crate) async fn fetch_and_extract(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
) -> Result<FetchOutput, String> {
    // Local sources: read and extract as HTML
    if cli.stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        let options = build_extraction_options(resolved);
        return extract_with_options(&buf, None, &options)
            .map(|r| FetchOutput::Local(Box::new(r)))
            .map_err(|e| format!("extraction error: {e}"));
    }

    if let Some(ref path) = cli.file {
        let html =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
        let options = build_extraction_options(resolved);
        return extract_with_options(&html, None, &options)
            .map(|r| FetchOutput::Local(Box::new(r)))
            .map_err(|e| format!("extraction error: {e}"));
    }

    let raw_url = cli
        .urls
        .first()
        .ok_or("no input provided -- pass a URL, --file, or --stdin")?;
    let url = normalize_url(raw_url);
    let url = url.as_str();

    let cloud_client = cloud::CloudClient::new(cli.api_key.as_deref());

    // Helper: map OutputFormat to the cloud API format string.
    fn cloud_format_str(f: &OutputFormat) -> &'static str {
        match f {
            OutputFormat::Markdown => "markdown",
            OutputFormat::Json => "json",
            OutputFormat::Text => "text",
            OutputFormat::Llm => "llm",
            OutputFormat::Html => "html",
        }
    }

    // --cloud: skip local, go straight to cloud API
    if cli.cloud {
        let c = cloud_client.ok_or("--cloud requires NOXA_API_KEY (set via env or --api-key)")?;
        let options = build_extraction_options(resolved);
        let resp = c
            .scrape(
                url,
                &[cloud_format_str(&resolved.format)],
                &options.include_selectors,
                &options.exclude_selectors,
                options.only_main_content,
            )
            .await?;
        return Ok(FetchOutput::Cloud(resp));
    }

    // Normal path: try local first
    let client = FetchClient::new(build_fetch_config(cli, resolved))
        .map_err(|e| format!("client error: {e}"))?;
    let options = build_extraction_options(resolved);
    let result = client
        .fetch_and_extract_with_options(url, &options)
        .await
        .map_err(|e| format!("fetch error: {e}"))?;

    // Check if we should fall back to cloud
    let reason = detect_empty(&result);
    if !matches!(reason, EmptyReason::None) {
        if let Some(ref c) = cloud_client {
            eprintln!("{}", info("falling back to cloud API..."));
            match c
                .scrape(
                    url,
                    &[cloud_format_str(&resolved.format)],
                    &options.include_selectors,
                    &options.exclude_selectors,
                    options.only_main_content,
                )
                .await
            {
                Ok(resp) => return Ok(FetchOutput::Cloud(resp)),
                Err(e) => {
                    eprintln!("{}", warning(&format!("cloud fallback failed: {e}")));
                    // Fall through to return the local result with a warning
                }
            }
        }
        warn_empty(url, &reason);
    }

    Ok(FetchOutput::Local(Box::new(result)))
}

/// Fetch raw HTML from a URL (no extraction). Used for --raw-html and brand extraction.
pub(crate) async fn fetch_html(
    cli: &Cli,
    resolved: &config::ResolvedConfig,
) -> Result<FetchResult, String> {
    if cli.stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        return Ok(FetchResult {
            html: buf,
            url: String::new(),
            status: 200,
            headers: Default::default(),
            elapsed: Default::default(),
        });
    }

    if let Some(ref path) = cli.file {
        let html =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
        return Ok(FetchResult {
            html,
            url: String::new(),
            status: 200,
            headers: Default::default(),
            elapsed: Default::default(),
        });
    }

    let raw_url = cli
        .urls
        .first()
        .ok_or("no input provided -- pass a URL, --file, or --stdin")?;
    let url = normalize_url(raw_url);

    let client = FetchClient::new(build_fetch_config(cli, resolved))
        .map_err(|e| format!("client error: {e}"))?;
    client
        .fetch(&url)
        .await
        .map_err(|e| format!("fetch error: {e}"))
}

/// Fetch external stylesheets referenced in HTML and inject them as `<style>` blocks.
/// This allows brand extraction to see colors/fonts from external CSS files.
pub(crate) async fn enrich_html_with_stylesheets(html: &str, base_url: &str) -> String {
    let base = match url::Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return html.to_string(),
    };

    // Extract stylesheet hrefs from <link rel="stylesheet" href="...">
    let re = regex::Regex::new(
        r#"<link[^>]+rel=["']stylesheet["'][^>]+href=["']([^"']+)["']|<link[^>]+href=["']([^"']+)["'][^>]+rel=["']stylesheet["']"#
    ).unwrap();

    let hrefs: Vec<String> = re
        .captures_iter(html)
        .filter_map(|cap| {
            let href = cap.get(1).or(cap.get(2))?;
            Some(
                base.join(href.as_str())
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| href.as_str().to_string()),
            )
        })
        .take(10)
        .collect();

    if hrefs.is_empty() {
        return html.to_string();
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return html.to_string(),
    };

    // Fetch stylesheets in parallel, but validate each URL first to prevent SSRF.
    let mut join_set = tokio::task::JoinSet::new();
    for href in hrefs {
        let client = client.clone();
        join_set.spawn(async move {
            // SSRF guard: only fetch public HTTP(S) URLs.
            if validate_public_http_url(&href).await.is_err() {
                return None;
            }
            if let Ok(resp) = client.get(&href).send().await
                && resp.status().is_success()
                && let Ok(body) = resp.text().await
                && !body.trim_start().starts_with("<!")
                && body.len() < 2_000_000
            {
                Some(body)
            } else {
                None
            }
        });
    }

    let mut extra_css = String::new();
    while let Some(result) = join_set.join_next().await {
        if let Ok(Some(body)) = result {
            extra_css.push_str("\n<style>\n");
            extra_css.push_str(&body);
            extra_css.push_str("\n</style>\n");
        }
    }

    if extra_css.is_empty() {
        return html.to_string();
    }

    if let Some(pos) = find_ascii_case_insensitive(html, "</head>") {
        let mut enriched = String::with_capacity(html.len() + extra_css.len());
        enriched.push_str(&html[..pos]);
        enriched.push_str(&extra_css);
        enriched.push_str(&html[pos..]);
        enriched
    } else {
        format!("{extra_css}{html}")
    }
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle_len = needle.len();
    haystack
        .as_bytes()
        .windows(needle_len)
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}
