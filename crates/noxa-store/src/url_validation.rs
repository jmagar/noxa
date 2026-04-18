use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use url::Url;

pub fn parse_http_url(url: &str) -> Result<Url, String> {
    if url.is_empty() {
        return Err("Invalid URL: must not be empty".into());
    }

    let parsed = Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "Invalid URL: scheme '{scheme}' not allowed, must start with http:// or https://"
            ));
        }
    }
    parsed
        .host_str()
        .ok_or_else(|| "Invalid URL: no host".to_string())?;
    Ok(parsed)
}

pub fn is_private_or_reserved_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_special_use_ipv4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_or_reserved_ip(IpAddr::V4(v4));
            }
            is_special_use_ipv6(v6)
        }
    }
}

fn is_special_use_ipv4(v4: Ipv4Addr) -> bool {
    let octets = v4.octets();
    v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_broadcast()
        || v4.is_documentation()
        || v4.is_multicast()
        || v4.is_unspecified()
        || octets[0] == 0
        || (octets[0] == 100 && octets[1] >= 64 && octets[1] <= 127)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 198 && octets[1] >= 18 && octets[1] <= 19)
        || octets[0] >= 240
}

fn is_special_use_ipv6(v6: Ipv6Addr) -> bool {
    let segments = v6.segments();
    v6.is_loopback()
        || v6.is_unspecified()
        || v6.is_multicast()
        || v6.is_unique_local()
        || v6.is_unicast_link_local()
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
}

pub async fn validate_public_http_url(url: &str) -> Result<(), String> {
    validate_public_http_url_with_resolver(url, |host| async move {
        tokio::net::lookup_host(host)
            .await
            .map(|iter| iter.collect::<Vec<_>>())
    })
    .await
}

pub async fn validate_public_http_url_with_resolver<F, Fut>(
    url: &str,
    resolve: F,
) -> Result<(), String>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = std::io::Result<Vec<SocketAddr>>>,
{
    let parsed = parse_http_url(url).map_err(append_scheme_hint)?;
    let Some(host) = parsed.host_str() else {
        return Ok(());
    };
    let lower = host.to_lowercase();

    if lower == "localhost" || lower.ends_with(".localhost") {
        return Err(format!(
            "Invalid URL: host '{host}' is a private or reserved address"
        ));
    }

    if let Ok(ip) = lower.parse::<IpAddr>() {
        if is_private_or_reserved_ip(ip) {
            return Err(format!(
                "Invalid URL: host '{host}' is a private or reserved address"
            ));
        }
        return Ok(());
    }

    match resolve(format!("{host}:80")).await {
        Ok(addrs) => {
            for addr in addrs {
                if is_private_or_reserved_ip(addr.ip()) {
                    return Err(format!(
                        "Invalid URL: host '{host}' resolves to a private or reserved address"
                    ));
                }
            }
            Ok(())
        }
        Err(e) => Err(format!("Invalid URL: could not resolve host '{host}': {e}")),
    }
}

fn append_scheme_hint(message: String) -> String {
    if message.contains("scheme '") || message.contains("relative URL without a base") {
        format!("{message}. Must start with http:// or https://")
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::{
        is_private_or_reserved_ip, parse_http_url, validate_public_http_url,
        validate_public_http_url_with_resolver,
    };

    #[tokio::test]
    async fn validate_public_http_url_accepts_hostname_resolving_to_public() {
        let result =
            validate_public_http_url_with_resolver("http://example.com/", |_| async move {
                Ok(vec![
                    "93.184.216.34:80".parse::<std::net::SocketAddr>().unwrap(),
                ])
            })
            .await;

        assert!(
            result.is_ok(),
            "hostname resolving to a public IP should be accepted"
        );
    }

    #[tokio::test]
    async fn validate_public_http_url_rejects_hostname_resolving_to_private() {
        let result =
            validate_public_http_url_with_resolver("http://attacker.example/", |_| async move {
                Ok(vec![
                    "192.168.1.1:80".parse::<std::net::SocketAddr>().unwrap(),
                ])
            })
            .await;

        assert!(
            result.is_err(),
            "hostname resolving to a private IP should be rejected"
        );
    }

    #[tokio::test]
    async fn validate_public_http_url_rejects_dns_failures() {
        let result =
            validate_public_http_url_with_resolver("http://nxdomain.example/", |_| async move {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "no such host",
                ))
            })
            .await;

        assert!(result.is_err(), "DNS failures should fail closed");
    }

    #[tokio::test]
    async fn validate_public_http_url_rejects_localhost() {
        let result = validate_public_http_url("http://localhost:8080/secret").await;
        assert!(result.is_err(), "localhost should be rejected");
    }

    #[test]
    fn parse_http_url_rejects_missing_host() {
        let result = parse_http_url("http://");
        assert!(result.is_err(), "missing host should be rejected");
    }

    #[tokio::test]
    async fn validate_public_http_url_empty_error_stays_specific() {
        let err = validate_public_http_url_with_resolver("", |_| async move {
            Ok(Vec::<std::net::SocketAddr>::new())
        })
        .await
        .expect_err("empty URL should be rejected");
        assert_eq!(err, "Invalid URL: must not be empty");
    }

    #[test]
    fn is_private_or_reserved_ip_rejects_missing_special_use_ranges() {
        let benchmarking: IpAddr = "198.18.0.1".parse().unwrap();
        let documentation: IpAddr = "2001:db8::1".parse().unwrap();

        assert!(is_private_or_reserved_ip(benchmarking));
        assert!(is_private_or_reserved_ip(documentation));
    }

    #[tokio::test]
    async fn validate_public_http_url_rejects_benchmarking_range() {
        let result =
            validate_public_http_url_with_resolver("http://benchmark.example/", |_| async move {
                Ok(vec![
                    "198.18.0.1:80".parse::<std::net::SocketAddr>().unwrap(),
                ])
            })
            .await;

        assert!(result.is_err(), "benchmarking range should be rejected");
    }

    #[tokio::test]
    async fn validate_public_http_url_rejects_ipv6_documentation_range() {
        let result =
            validate_public_http_url_with_resolver("http://docs.example/", |_| async move {
                Ok(vec![
                    "[2001:db8::1]:80".parse::<std::net::SocketAddr>().unwrap(),
                ])
            })
            .await;

        assert!(
            result.is_err(),
            "IPv6 documentation range should be rejected"
        );
    }
}
