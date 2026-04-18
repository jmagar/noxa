use tokio::task::JoinSet;

use crate::error::NoxaMcpError;

const URL_VALIDATION_CONCURRENCY: usize = 8;

/// Validate that a URL is non-empty, has an http/https scheme, and does not target
/// private/loopback/reserved hosts (SSRF prevention).
pub async fn validate_url(url: &str) -> Result<(), NoxaMcpError> {
    noxa_store::validate_public_http_url(url)
        .await
        .map_err(|reason| NoxaMcpError::UrlValidation {
            url: url.to_string(),
            reason,
        })
}

/// Validate a list of URLs concurrently with bounded parallelism.
#[allow(dead_code)]
pub async fn validate_urls(urls: &[String]) -> Result<(), NoxaMcpError> {
    if urls.is_empty() {
        return Ok(());
    }

    let mut pending = urls.iter();
    let mut set = JoinSet::new();

    for _ in 0..URL_VALIDATION_CONCURRENCY.min(urls.len()) {
        if let Some(url) = pending.next() {
            let url = url.clone();
            set.spawn(async move { (url.clone(), validate_url(&url).await) });
        }
    }

    while let Some(result) = set.join_next().await {
        let (_validated_url, validation_result) = result
            .map_err(|e| NoxaMcpError::message(format!("URL validation task failed: {e}")))?;
        validation_result?;

        if let Some(url) = pending.next() {
            let url = url.clone();
            set.spawn(async move { (url.clone(), validate_url(&url).await) });
        }
    }

    Ok(())
}

pub async fn validate_fetch_url(
    config: &noxa_fetch::FetchConfig,
    url: &str,
) -> Result<(), NoxaMcpError> {
    validate_fetch_url_with(
        url,
        config.max_redirects,
        |candidate| async move { validate_url(&candidate).await },
        move |current| {
            let mut config = config.clone();
            config.follow_redirects = false;
            config.store = None;
            config.ops_log = None;
            async move {
                let client =
                    noxa_fetch::FetchClient::new(config).map_err(NoxaMcpError::FetchClientInit)?;
                client.fetch(&current).await.map_err(NoxaMcpError::Fetch)
            }
        },
    )
    .await
}

pub async fn validate_fetch_urls(
    config: &noxa_fetch::FetchConfig,
    urls: &[String],
) -> Result<(), NoxaMcpError> {
    if urls.is_empty() {
        return Ok(());
    }

    let mut pending = urls.iter();
    let mut set = JoinSet::new();

    for _ in 0..URL_VALIDATION_CONCURRENCY.min(urls.len()) {
        if let Some(url) = pending.next() {
            let url = url.clone();
            let config = config.clone();
            set.spawn(async move { (url.clone(), validate_fetch_url(&config, &url).await) });
        }
    }

    while let Some(result) = set.join_next().await {
        let (_validated_url, validation_result) = result
            .map_err(|e| NoxaMcpError::message(format!("URL validation task failed: {e}")))?;
        validation_result?;

        if let Some(url) = pending.next() {
            let url = url.clone();
            let config = config.clone();
            set.spawn(async move { (url.clone(), validate_fetch_url(&config, &url).await) });
        }
    }

    Ok(())
}

pub async fn collect_valid_urls(urls: &[String]) -> Vec<(String, Result<(), NoxaMcpError>)> {
    if urls.is_empty() {
        return Vec::new();
    }

    let mut pending = urls.iter();
    let mut set = JoinSet::new();
    let mut results = Vec::with_capacity(urls.len());

    for _ in 0..URL_VALIDATION_CONCURRENCY.min(urls.len()) {
        if let Some(url) = pending.next() {
            let url = url.clone();
            set.spawn(async move { (url.clone(), validate_url(&url).await) });
        }
    }

    while let Some(result) = set.join_next().await {
        match result {
            Ok(entry) => results.push(entry),
            Err(error) => results.push((
                "<unknown>".to_string(),
                Err(NoxaMcpError::message(format!(
                    "URL validation task failed: {error}"
                ))),
            )),
        }

        if let Some(url) = pending.next() {
            let url = url.clone();
            set.spawn(async move { (url.clone(), validate_url(&url).await) });
        }
    }

    results
}

async fn validate_fetch_url_with<V, VFut, F, FFut>(
    url: &str,
    max_redirects: u32,
    validator: V,
    fetcher: F,
) -> Result<(), NoxaMcpError>
where
    V: Fn(String) -> VFut,
    VFut: std::future::Future<Output = Result<(), NoxaMcpError>>,
    F: Fn(String) -> FFut,
    FFut: std::future::Future<Output = Result<noxa_fetch::FetchResult, NoxaMcpError>>,
{
    validator(url.to_string()).await?;

    let mut current = url.to_string();
    for redirect in 0..=max_redirects {
        let result = fetcher(current.clone()).await?;
        if let Some(next) = redirect_location(&current, result.status, &result.headers)? {
            if redirect == max_redirects {
                return Err(NoxaMcpError::message(format!(
                    "redirect limit exceeded for {url}"
                )));
            }
            validator(next.clone()).await?;
            current = next;
            continue;
        }
        return Ok(());
    }

    Ok(())
}

fn redirect_location(
    current_url: &str,
    status: u16,
    headers: &noxa_fetch::HeaderMap,
) -> Result<Option<String>, NoxaMcpError> {
    if !matches!(status, 301 | 302 | 303 | 307 | 308) {
        return Ok(None);
    }

    let location = headers
        .get("location")
        .ok_or_else(|| {
            NoxaMcpError::message(format!(
                "redirect response missing Location for {current_url}"
            ))
        })?
        .to_str()
        .map_err(|e| {
            NoxaMcpError::message(format!("invalid redirect Location for {current_url}: {e}"))
        })?;

    let next = url::Url::parse(current_url)
        .map_err(|e| {
            NoxaMcpError::message(format!("failed to parse redirect URL {current_url}: {e}"))
        })?
        .join(location)
        .map_err(|e| {
            NoxaMcpError::message(format!("failed to resolve redirect for {current_url}: {e}"))
        })?;

    Ok(Some(next.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn validate_rejects_loopback() {
        assert!(validate_url("http://127.0.0.1/secret").await.is_err());
        assert!(validate_url("http://127.0.0.1:8080/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_localhost() {
        assert!(validate_url("http://localhost/secret").await.is_err());
        assert!(validate_url("http://localhost:8080/secret").await.is_err());
        assert!(validate_url("http://foo.localhost/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_rfc1918() {
        assert!(validate_url("http://10.0.0.1/").await.is_err());
        assert!(validate_url("http://172.16.0.1/").await.is_err());
        assert!(validate_url("http://172.31.255.255/").await.is_err());
        assert!(validate_url("http://192.168.1.1/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_link_local() {
        assert!(
            validate_url("http://169.254.169.254/latest/meta-data/")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn validate_rejects_tailscale() {
        assert!(validate_url("http://100.100.1.1/").await.is_err());
        assert!(validate_url("http://100.127.255.255/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_loopback() {
        assert!(validate_url("http://[::1]/secret").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_link_local() {
        assert!(validate_url("http://[fe80::1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv6_ula() {
        assert!(validate_url("http://[fd00::1]/").await.is_err());
        assert!(validate_url("http://[fc00::1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_rejects_ipv4_mapped_ipv6() {
        assert!(validate_url("http://[::ffff:127.0.0.1]/").await.is_err());
        assert!(
            validate_url("http://[::ffff:169.254.169.254]/latest/meta-data/")
                .await
                .is_err()
        );
        assert!(validate_url("http://[::ffff:10.0.0.1]/").await.is_err());
    }

    #[tokio::test]
    async fn validate_accepts_public_ip() {
        assert!(validate_url("http://8.8.8.8/").await.is_ok());
        assert!(validate_url("http://1.1.1.1/").await.is_ok());
    }

    #[tokio::test]
    async fn validate_urls_checks_all_entries() {
        let urls = vec![
            "http://1.1.1.1/".to_string(),
            "http://127.0.0.1/".to_string(),
            "http://8.8.8.8/".to_string(),
        ];

        let err = validate_urls(&urls).await.unwrap_err().to_string();
        assert!(err.contains("127.0.0.1"));
    }

    #[tokio::test]
    async fn validate_fetch_url_rejects_private_redirect_hop() {
        let mut headers = noxa_fetch::HeaderMap::new();
        headers.insert(
            "location",
            "http://169.254.169.254/latest/meta-data/".parse().unwrap(),
        );

        let err = validate_fetch_url_with(
            "http://public.example/start",
            5,
            |candidate| async move {
                if candidate.contains("169.254.169.254") {
                    Err(NoxaMcpError::message("private target"))
                } else {
                    Ok(())
                }
            },
            move |_current| {
                let headers = headers.clone();
                async move {
                    Ok(noxa_fetch::FetchResult {
                        html: "redirect".into(),
                        status: 302,
                        url: "http://public.example/start".into(),
                        headers,
                        elapsed: std::time::Duration::from_millis(1),
                    })
                }
            },
        )
        .await
        .unwrap_err()
        .to_string();

        assert!(err.contains("private target"));
    }

    #[tokio::test]
    async fn validate_fetch_url_allows_public_redirect_chain() {
        let responses = std::sync::Arc::new(std::sync::Mutex::new(vec![
            (
                302_u16,
                Some("https://example.com/final".to_string()),
                "redirect".to_string(),
            ),
            (200_u16, None, "ok".to_string()),
        ]));

        validate_fetch_url_with(
            "http://public.example/start",
            5,
            |_candidate| async { Ok(()) },
            move |current| {
                let responses = std::sync::Arc::clone(&responses);
                async move {
                    let (status, location, body) = responses.lock().unwrap().remove(0);
                    let mut headers = noxa_fetch::HeaderMap::new();
                    if let Some(location) = location {
                        headers.insert("location", location.parse().unwrap());
                    }
                    Ok(noxa_fetch::FetchResult {
                        html: body,
                        status,
                        url: current,
                        headers,
                        elapsed: std::time::Duration::from_millis(1),
                    })
                }
            },
        )
        .await
        .unwrap();
    }
}
