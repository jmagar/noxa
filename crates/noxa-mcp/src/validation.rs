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
}
