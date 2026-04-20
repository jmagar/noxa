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
                        config.follow_redirects,
                        config.max_redirects,
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
                    crate::tls::build_client(
                        variant,
                        config.timeout,
                        &config.headers,
                        Some(proxy),
                        config.follow_redirects,
                        config.max_redirects,
                    )
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
            ClientPool::Rotating { clients } => {
                let host = extract_host(url);
                pick_for_host(clients, &host)
            }
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
    // Use a simple stable hash derived from the byte values of the host string.
    // This is deterministic across process restarts (unlike DefaultHasher which
    // uses a random seed), so the same host always maps to the same client.
    let hash: usize = host.bytes().fold(0usize, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as usize)
    });
    &clients[hash % clients.len()]
}
