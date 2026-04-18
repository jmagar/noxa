use std::path::{Path, PathBuf};

use noxa_store::parse_http_url;

use crate::error::NoxaMcpError;

#[derive(Debug, Clone)]
pub struct NoxaMcpConfig {
    pub fetch: noxa_fetch::FetchConfig,
    pub store: noxa_store::FilesystemContentStore,
    pub research_dir: PathBuf,
    pub searxng_url: Option<String>,
    pub cloud_api_key: Option<String>,
}

impl NoxaMcpConfig {
    pub fn from_env() -> Result<Self, NoxaMcpError> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            NoxaMcpError::message("failed to determine home directory for noxa-mcp")
        })?;
        let proxy_file = std::env::var("NOXA_PROXY_FILE").ok().map(PathBuf::from);

        Self::from_inputs(
            home_dir,
            std::env::var("NOXA_PROXY").ok(),
            proxy_file,
            std::env::var("SEARXNG_URL").ok(),
            std::env::var("NOXA_API_KEY").ok(),
        )
    }

    pub fn from_inputs(
        home_dir: PathBuf,
        proxy: Option<String>,
        proxy_file: Option<PathBuf>,
        searxng_url: Option<String>,
        cloud_api_key: Option<String>,
    ) -> Result<Self, NoxaMcpError> {
        let noxa_root = home_dir.join(".noxa");
        let store_root = noxa_root.join("content");
        let research_dir = noxa_root.join("research");

        create_dir(&store_root)?;
        create_dir(&research_dir)?;

        let store = noxa_store::FilesystemContentStore::new(&store_root);
        let mut fetch = noxa_fetch::FetchConfig {
            store: Some(store.clone()),
            ..Default::default()
        };

        if let Some(proxy) = normalize_optional(proxy) {
            fetch.proxy = Some(proxy);
        }

        let proxy_file = proxy_file.unwrap_or_else(|| PathBuf::from("proxies.txt"));
        if proxy_file.exists() {
            fetch.proxy_pool =
                noxa_fetch::parse_proxy_file(path_to_str(&proxy_file)?).map_err(|source| {
                    NoxaMcpError::ProxyPool {
                        path: proxy_file.clone(),
                        source,
                    }
                })?;
        }

        let searxng_url = match normalize_optional(searxng_url) {
            Some(url) => {
                parse_http_url(&url).map_err(|e| NoxaMcpError::InvalidSearxngUrl(e.to_string()))?;
                Some(url)
            }
            None => None,
        };

        Ok(Self {
            fetch,
            store,
            research_dir,
            searxng_url,
            cloud_api_key: normalize_optional(cloud_api_key),
        })
    }
}

fn create_dir(path: &Path) -> Result<(), NoxaMcpError> {
    std::fs::create_dir_all(path).map_err(|source| NoxaMcpError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}

fn path_to_str(path: &Path) -> Result<&str, NoxaMcpError> {
    path.to_str().ok_or_else(|| {
        NoxaMcpError::message(format!("path is not valid UTF-8: {}", path.display()))
    })
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::NoxaMcpConfig;

    #[test]
    fn config_loads_proxy_pool_and_paths_from_inputs() {
        let home = tempdir().unwrap();
        let proxy_file = home.path().join("proxies.txt");
        std::fs::write(&proxy_file, "proxy.example.com:8080:user:pass\n").unwrap();

        let config = NoxaMcpConfig::from_inputs(
            home.path().to_path_buf(),
            Some("http://proxy.internal:8080".into()),
            Some(proxy_file.clone()),
            Some(" https://search.example.com ".into()),
            Some("api-key".into()),
        )
        .unwrap();

        assert_eq!(
            config.fetch.proxy.as_deref(),
            Some("http://proxy.internal:8080")
        );
        assert_eq!(config.fetch.proxy_pool.len(), 1);
        assert_eq!(
            config.searxng_url.as_deref(),
            Some("https://search.example.com")
        );
        assert_eq!(config.cloud_api_key.as_deref(), Some("api-key"));
        assert_eq!(
            config.store.root(),
            PathBuf::from(home.path()).join(".noxa").join("content")
        );
        assert!(config.research_dir.exists());
    }

    #[test]
    fn config_rejects_invalid_searxng_url() {
        let home = tempdir().unwrap();
        let err = NoxaMcpConfig::from_inputs(
            home.path().to_path_buf(),
            None,
            None,
            Some("ftp://invalid.example.com".into()),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("SEARXNG_URL"));
    }
}
