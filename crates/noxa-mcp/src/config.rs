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
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub llm_base_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NoxaMcpConfigInputs {
    pub home_dir: PathBuf,
    pub proxy: Option<String>,
    pub proxy_file: Option<PathBuf>,
    pub searxng_url: Option<String>,
    pub cloud_api_key: Option<String>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub llm_base_url: Option<String>,
}

impl NoxaMcpConfig {
    pub fn from_env() -> Result<Self, NoxaMcpError> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            NoxaMcpError::message("failed to determine home directory for noxa-mcp")
        })?;
        let proxy_file = std::env::var("NOXA_PROXY_FILE").ok().map(PathBuf::from);

        Self::from_inputs(NoxaMcpConfigInputs {
            home_dir,
            proxy: std::env::var("NOXA_PROXY").ok(),
            proxy_file,
            searxng_url: std::env::var("SEARXNG_URL").ok(),
            cloud_api_key: std::env::var("NOXA_API_KEY").ok(),
            llm_provider: std::env::var("NOXA_LLM_PROVIDER").ok(),
            llm_model: std::env::var("NOXA_LLM_MODEL").ok(),
            llm_base_url: std::env::var("NOXA_LLM_BASE_URL").ok(),
        })
    }

    pub fn from_inputs(inputs: NoxaMcpConfigInputs) -> Result<Self, NoxaMcpError> {
        let NoxaMcpConfigInputs {
            home_dir,
            proxy,
            proxy_file,
            searxng_url,
            cloud_api_key,
            llm_provider,
            llm_model,
            llm_base_url,
        } = inputs;

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
            llm_provider: normalize_optional(llm_provider),
            llm_model: normalize_optional(llm_model),
            llm_base_url: normalize_optional(llm_base_url),
        })
    }
}

pub(crate) fn env_file_candidates(
    home_dir: &Path,
    exe_path: Option<&Path>,
    cwd: Option<&Path>,
    repo_root: Option<&Path>,
) -> Vec<PathBuf> {
    let mut candidates = vec![home_dir.join(".noxa").join(".env")];

    if let Some(exe_path) = exe_path.and_then(Path::parent) {
        candidates.push(exe_path.join(".env"));
    }
    if let Some(repo_root) = repo_root {
        candidates.push(repo_root.join(".env"));
    }
    if let Some(cwd) = cwd {
        candidates.push(cwd.join(".env"));
    }

    candidates
}

pub(crate) fn find_env_file(
    home_dir: &Path,
    exe_path: Option<&Path>,
    cwd: Option<&Path>,
    repo_root: Option<&Path>,
) -> Option<PathBuf> {
    env_file_candidates(home_dir, exe_path, cwd, repo_root)
        .into_iter()
        .find(|path| path.is_file())
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

    use super::{NoxaMcpConfig, NoxaMcpConfigInputs, env_file_candidates, find_env_file};

    #[test]
    fn config_loads_proxy_pool_and_paths_from_inputs() {
        let home = tempdir().unwrap();
        let proxy_file = home.path().join("proxies.txt");
        std::fs::write(&proxy_file, "proxy.example.com:8080:user:pass\n").unwrap();

        let config = NoxaMcpConfig::from_inputs(NoxaMcpConfigInputs {
            home_dir: home.path().to_path_buf(),
            proxy: Some("http://proxy.internal:8080".into()),
            proxy_file: Some(proxy_file.clone()),
            searxng_url: Some(" https://search.example.com ".into()),
            cloud_api_key: Some("api-key".into()),
            llm_provider: Some(" ollama ".into()),
            llm_model: Some(" qwen3.5:14b ".into()),
            llm_base_url: Some(" http://ollama.internal:11434 ".into()),
        })
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
        assert_eq!(config.llm_provider.as_deref(), Some("ollama"));
        assert_eq!(config.llm_model.as_deref(), Some("qwen3.5:14b"));
        assert_eq!(
            config.llm_base_url.as_deref(),
            Some("http://ollama.internal:11434")
        );
        assert_eq!(
            config.store.root(),
            PathBuf::from(home.path()).join(".noxa").join("content")
        );
        assert!(config.research_dir.exists());
    }

    #[test]
    fn config_rejects_invalid_searxng_url() {
        let home = tempdir().unwrap();
        let err = NoxaMcpConfig::from_inputs(NoxaMcpConfigInputs {
            home_dir: home.path().to_path_buf(),
            proxy: None,
            proxy_file: None,
            searxng_url: Some("ftp://invalid.example.com".into()),
            cloud_api_key: None,
            llm_provider: None,
            llm_model: None,
            llm_base_url: None,
        })
        .unwrap_err()
        .to_string();

        assert!(err.contains("SEARXNG_URL"));
    }

    #[test]
    fn env_file_candidates_follow_requested_order() {
        let home = PathBuf::from("/home/tester");
        let exe = PathBuf::from("/opt/noxa/bin/noxa-mcp");
        let cwd = PathBuf::from("/workspace/noxa");
        let repo_root = PathBuf::from("/workspace/noxa");

        let candidates = env_file_candidates(&home, Some(&exe), Some(&cwd), Some(&repo_root));

        assert_eq!(
            candidates,
            vec![
                home.join(".noxa").join(".env"),
                PathBuf::from("/opt/noxa/bin/.env"),
                PathBuf::from("/workspace/noxa/.env"),
                PathBuf::from("/workspace/noxa/.env"),
            ]
        );
    }

    #[test]
    fn find_env_file_returns_first_existing_candidate() {
        let root = tempdir().unwrap();
        let home = root.path().join("home");
        let exe_dir = root.path().join("bin");
        let cwd = root.path().join("cwd");
        let repo = root.path().join("repo");

        std::fs::create_dir_all(home.join(".noxa")).unwrap();
        std::fs::create_dir_all(&exe_dir).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(&repo).unwrap();

        std::fs::write(exe_dir.join(".env"), "SEARXNG_URL=https://binary.example\n").unwrap();
        std::fs::write(repo.join(".env"), "SEARXNG_URL=https://repo.example\n").unwrap();
        std::fs::write(cwd.join(".env"), "SEARXNG_URL=https://cwd.example\n").unwrap();

        let found = find_env_file(
            &home,
            Some(&exe_dir.join("noxa-mcp")),
            Some(&cwd),
            Some(&repo),
        );

        assert_eq!(found, Some(exe_dir.join(".env")));
    }
}
