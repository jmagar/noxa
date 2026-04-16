use std::hash::{Hash, Hasher};

use rand::seq::SliceRandom;
use tracing::debug;

use crate::browser::{self, BrowserProfile, BrowserVariant};
use crate::client::{ClientPool, FetchClient, FetchConfig};
use crate::error::FetchError;

impl FetchClient {
    pub fn new(config: FetchConfig) -> Result<Self, FetchError> {
        let variants = collect_variants(&config.browser);
        let pdf_mode = config.pdf_mode.clone();
        let store = config.store.clone();
        let ops_log = config.ops_log.clone();

        let pool = if config.proxy_pool.is_empty() {
            let clients = variants
                .into_iter()
                .map(|variant| {
                    crate::tls::build_client(
                        variant,
                        config.timeout,
                        &config.headers,
                        config.proxy.as_deref(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            let random = matches!(config.browser, BrowserProfile::Random);
            debug!(
                count = clients.len(),
                random, "fetch client ready (static pool)"
            );
            ClientPool::Static { clients, random }
        } else {
            let mut rng = rand::thread_rng();
            let clients = config
                .proxy_pool
                .iter()
                .map(|proxy| {
                    let variant = *variants.choose(&mut rng).unwrap();
                    crate::tls::build_client(variant, config.timeout, &config.headers, Some(proxy))
                })
                .collect::<Result<Vec<_>, _>>()?;

            debug!(
                clients = clients.len(),
                "fetch client ready (pre-built rotating pool)"
            );
            ClientPool::Rotating { clients }
        };

        Ok(Self {
            pool,
            pdf_mode,
            store,
            ops_log,
        })
    }

    pub fn proxy_pool_size(&self) -> usize {
        match &self.pool {
            ClientPool::Static { .. } => 0,
            ClientPool::Rotating { clients } => clients.len(),
        }
    }

    pub(super) fn pick_client(&self, url: &str) -> &wreq::Client {
        match &self.pool {
            ClientPool::Static { clients, random } => {
                if *random {
                    let host = extract_host(url);
                    pick_for_host(clients, &host)
                } else {
                    &clients[0]
                }
            }
            ClientPool::Rotating { clients } => pick_random(clients),
        }
    }
}

pub(super) fn collect_variants(profile: &BrowserProfile) -> Vec<BrowserVariant> {
    match profile {
        BrowserProfile::Random => browser::all_variants(),
        BrowserProfile::Chrome => vec![browser::latest_chrome()],
        BrowserProfile::Firefox => vec![browser::latest_firefox()],
    }
}

pub(super) fn extract_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_default()
}

pub(super) fn pick_for_host<'a>(clients: &'a [wreq::Client], host: &str) -> &'a wreq::Client {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    host.hash(&mut hasher);
    let idx = (hasher.finish() as usize) % clients.len();
    &clients[idx]
}

pub(super) fn pick_random(clients: &[wreq::Client]) -> &wreq::Client {
    use rand::Rng;
    let idx = rand::thread_rng().gen_range(0..clients.len());
    &clients[idx]
}
