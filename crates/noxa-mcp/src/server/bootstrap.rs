use std::path::Path;
use std::sync::Arc;

use tracing::{error, info, warn};

use crate::cloud::{self, CloudClient, SmartFetchResult};
use crate::server::{NO_LLM_PROVIDERS_MESSAGE, NoxaMcp};

impl NoxaMcp {
    pub async fn new() -> Self {
        let mut config = noxa_fetch::FetchConfig::default();

        if let Ok(proxy) = std::env::var("NOXA_PROXY") {
            info!("using single proxy from NOXA_PROXY");
            config.proxy = Some(proxy);
        }

        let proxy_file = std::env::var("NOXA_PROXY_FILE")
            .ok()
            .unwrap_or_else(|| "proxies.txt".to_string());
        if Path::new(&proxy_file).exists()
            && let Ok(pool) = noxa_fetch::parse_proxy_file(&proxy_file)
            && !pool.is_empty()
        {
            info!(count = pool.len(), file = %proxy_file, "loaded proxy pool");
            config.proxy_pool = pool;
        }

        let store = noxa_store::FilesystemContentStore::open().unwrap_or_else(|e| {
            error!("content store init failed: {e}");
            std::process::exit(1);
        });
        if let Err(e) = std::fs::create_dir_all(store.root()) {
            error!("failed to create content store directory: {e}");
            std::process::exit(1);
        }
        info!("content store ready");

        config.store = Some(store.clone());

        let fetch_client = match noxa_fetch::FetchClient::new(config) {
            Ok(client) => client,
            Err(e) => {
                error!("failed to build FetchClient: {e}");
                std::process::exit(1);
            }
        };

        let chain = noxa_llm::ProviderChain::default().await;
        let llm_chain = if chain.is_empty() {
            warn!("{NO_LLM_PROVIDERS_MESSAGE} -- extract/summarize tools will fail");
            None
        } else {
            info!(providers = chain.len(), "LLM provider chain ready");
            Some(chain)
        };

        let cloud = CloudClient::from_env();
        if cloud.is_some() {
            info!("cloud API fallback enabled (NOXA_API_KEY set)");
        } else {
            warn!(
                "NOXA_API_KEY not set -- bot-protected sites will return challenge pages. \
                 Get a key at https://noxa.io"
            );
        }

        Self {
            tool_router: Self::tool_router(),
            fetch_client: Arc::new(fetch_client),
            llm_chain,
            cloud,
            store,
        }
    }

    pub(super) async fn smart_fetch_llm(&self, url: &str) -> Result<SmartFetchResult, String> {
        cloud::smart_fetch(
            &self.fetch_client,
            self.cloud.as_ref(),
            url,
            &[],
            &[],
            false,
            &["llm", "markdown"],
        )
        .await
    }
}
