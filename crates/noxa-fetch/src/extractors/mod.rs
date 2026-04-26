//! Site-specific vertical extractors.

pub mod amazon_product;
pub mod arxiv;
pub mod crates_io;
pub mod dev_to;
pub mod docker_hub;
pub mod ebay_listing;
pub mod ecommerce_product;
pub mod etsy_listing;
pub mod github_issue;
pub mod github_pr;
pub mod github_release;
pub mod github_repo;
pub mod hackernews;
pub mod http;
pub mod huggingface_dataset;
pub mod huggingface_model;
pub mod instagram_post;
pub mod instagram_profile;
pub mod linkedin_post;
pub mod npm;
pub mod pypi;
pub mod reddit;
pub mod shopify_collection;
pub mod shopify_product;
pub mod stackoverflow;
pub mod substack_post;
pub mod summary;
pub mod trustpilot_reviews;
pub mod woocommerce_product;
pub mod youtube_video;

use serde::Serialize;
use serde_json::Value;

use crate::error::FetchError;
use crate::extractors::http::ExtractorHttp;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ExtractorInfo {
    pub name: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub url_patterns: &'static [&'static str],
}

pub fn list() -> Vec<ExtractorInfo> {
    vec![
        reddit::INFO,
        hackernews::INFO,
        github_repo::INFO,
        github_pr::INFO,
        github_issue::INFO,
        github_release::INFO,
        pypi::INFO,
        npm::INFO,
        crates_io::INFO,
        huggingface_model::INFO,
        huggingface_dataset::INFO,
        arxiv::INFO,
        docker_hub::INFO,
        dev_to::INFO,
        stackoverflow::INFO,
        substack_post::INFO,
        youtube_video::INFO,
        linkedin_post::INFO,
        instagram_post::INFO,
        instagram_profile::INFO,
        shopify_product::INFO,
        shopify_collection::INFO,
        ecommerce_product::INFO,
        woocommerce_product::INFO,
        amazon_product::INFO,
        ebay_listing::INFO,
        etsy_listing::INFO,
        trustpilot_reviews::INFO,
    ]
}

pub async fn dispatch_by_url(
    client: &dyn ExtractorHttp,
    url: &str,
) -> Option<Result<(&'static str, Value), FetchError>> {
    if reddit::matches(url) {
        return Some(reddit::extract(client, url).await.map(|v| (reddit::INFO.name, v)));
    }
    if hackernews::matches(url) {
        return Some(
            hackernews::extract(client, url)
                .await
                .map(|v| (hackernews::INFO.name, v)),
        );
    }
    if github_repo::matches(url) {
        return Some(
            github_repo::extract(client, url)
                .await
                .map(|v| (github_repo::INFO.name, v)),
        );
    }
    if pypi::matches(url) {
        return Some(pypi::extract(client, url).await.map(|v| (pypi::INFO.name, v)));
    }
    if npm::matches(url) {
        return Some(npm::extract(client, url).await.map(|v| (npm::INFO.name, v)));
    }
    if github_pr::matches(url) {
        return Some(
            github_pr::extract(client, url)
                .await
                .map(|v| (github_pr::INFO.name, v)),
        );
    }
    if github_issue::matches(url) {
        return Some(
            github_issue::extract(client, url)
                .await
                .map(|v| (github_issue::INFO.name, v)),
        );
    }
    if github_release::matches(url) {
        return Some(
            github_release::extract(client, url)
                .await
                .map(|v| (github_release::INFO.name, v)),
        );
    }
    if crates_io::matches(url) {
        return Some(
            crates_io::extract(client, url)
                .await
                .map(|v| (crates_io::INFO.name, v)),
        );
    }
    if huggingface_model::matches(url) {
        return Some(
            huggingface_model::extract(client, url)
                .await
                .map(|v| (huggingface_model::INFO.name, v)),
        );
    }
    if huggingface_dataset::matches(url) {
        return Some(
            huggingface_dataset::extract(client, url)
                .await
                .map(|v| (huggingface_dataset::INFO.name, v)),
        );
    }
    if arxiv::matches(url) {
        return Some(arxiv::extract(client, url).await.map(|v| (arxiv::INFO.name, v)));
    }
    if docker_hub::matches(url) {
        return Some(
            docker_hub::extract(client, url)
                .await
                .map(|v| (docker_hub::INFO.name, v)),
        );
    }
    if dev_to::matches(url) {
        return Some(dev_to::extract(client, url).await.map(|v| (dev_to::INFO.name, v)));
    }
    if stackoverflow::matches(url) {
        return Some(
            stackoverflow::extract(client, url)
                .await
                .map(|v| (stackoverflow::INFO.name, v)),
        );
    }
    if linkedin_post::matches(url) {
        return Some(
            linkedin_post::extract(client, url)
                .await
                .map(|v| (linkedin_post::INFO.name, v)),
        );
    }
    if instagram_post::matches(url) {
        return Some(
            instagram_post::extract(client, url)
                .await
                .map(|v| (instagram_post::INFO.name, v)),
        );
    }
    if instagram_profile::matches(url) {
        return Some(
            instagram_profile::extract(client, url)
                .await
                .map(|v| (instagram_profile::INFO.name, v)),
        );
    }
    if amazon_product::matches(url) {
        return Some(
            amazon_product::extract(client, url)
                .await
                .map(|v| (amazon_product::INFO.name, v)),
        );
    }
    if ebay_listing::matches(url) {
        return Some(
            ebay_listing::extract(client, url)
                .await
                .map(|v| (ebay_listing::INFO.name, v)),
        );
    }
    if etsy_listing::matches(url) {
        return Some(
            etsy_listing::extract(client, url)
                .await
                .map(|v| (etsy_listing::INFO.name, v)),
        );
    }
    if trustpilot_reviews::matches(url) {
        return Some(
            trustpilot_reviews::extract(client, url)
                .await
                .map(|v| (trustpilot_reviews::INFO.name, v)),
        );
    }
    if youtube_video::matches(url) {
        return Some(
            youtube_video::extract(client, url)
                .await
                .map(|v| (youtube_video::INFO.name, v)),
        );
    }
    None
}

pub async fn dispatch_by_name(
    client: &dyn ExtractorHttp,
    name: &str,
    url: &str,
) -> Result<Value, ExtractorDispatchError> {
    match name {
        n if n == reddit::INFO.name => {
            run_or_mismatch(reddit::matches(url), n, url, || reddit::extract(client, url)).await
        }
        n if n == hackernews::INFO.name => {
            run_or_mismatch(hackernews::matches(url), n, url, || {
                hackernews::extract(client, url)
            })
            .await
        }
        n if n == github_repo::INFO.name => {
            run_or_mismatch(github_repo::matches(url), n, url, || {
                github_repo::extract(client, url)
            })
            .await
        }
        n if n == github_pr::INFO.name => {
            run_or_mismatch(github_pr::matches(url), n, url, || {
                github_pr::extract(client, url)
            })
            .await
        }
        n if n == github_issue::INFO.name => {
            run_or_mismatch(github_issue::matches(url), n, url, || {
                github_issue::extract(client, url)
            })
            .await
        }
        n if n == github_release::INFO.name => {
            run_or_mismatch(github_release::matches(url), n, url, || {
                github_release::extract(client, url)
            })
            .await
        }
        n if n == pypi::INFO.name => {
            run_or_mismatch(pypi::matches(url), n, url, || pypi::extract(client, url)).await
        }
        n if n == npm::INFO.name => {
            run_or_mismatch(npm::matches(url), n, url, || npm::extract(client, url)).await
        }
        n if n == crates_io::INFO.name => {
            run_or_mismatch(crates_io::matches(url), n, url, || {
                crates_io::extract(client, url)
            })
            .await
        }
        n if n == huggingface_model::INFO.name => {
            run_or_mismatch(huggingface_model::matches(url), n, url, || {
                huggingface_model::extract(client, url)
            })
            .await
        }
        n if n == huggingface_dataset::INFO.name => {
            run_or_mismatch(huggingface_dataset::matches(url), n, url, || {
                huggingface_dataset::extract(client, url)
            })
            .await
        }
        n if n == arxiv::INFO.name => {
            run_or_mismatch(arxiv::matches(url), n, url, || arxiv::extract(client, url)).await
        }
        n if n == docker_hub::INFO.name => {
            run_or_mismatch(docker_hub::matches(url), n, url, || {
                docker_hub::extract(client, url)
            })
            .await
        }
        n if n == dev_to::INFO.name => {
            run_or_mismatch(dev_to::matches(url), n, url, || dev_to::extract(client, url)).await
        }
        n if n == stackoverflow::INFO.name => {
            run_or_mismatch(stackoverflow::matches(url), n, url, || {
                stackoverflow::extract(client, url)
            })
            .await
        }
        n if n == substack_post::INFO.name => {
            run_or_mismatch(substack_post::matches(url), n, url, || {
                substack_post::extract(client, url)
            })
            .await
        }
        n if n == youtube_video::INFO.name => {
            run_or_mismatch(youtube_video::matches(url), n, url, || {
                youtube_video::extract(client, url)
            })
            .await
        }
        n if n == linkedin_post::INFO.name => {
            run_or_mismatch(linkedin_post::matches(url), n, url, || {
                linkedin_post::extract(client, url)
            })
            .await
        }
        n if n == instagram_post::INFO.name => {
            run_or_mismatch(instagram_post::matches(url), n, url, || {
                instagram_post::extract(client, url)
            })
            .await
        }
        n if n == instagram_profile::INFO.name => {
            run_or_mismatch(instagram_profile::matches(url), n, url, || {
                instagram_profile::extract(client, url)
            })
            .await
        }
        n if n == shopify_product::INFO.name => {
            run_or_mismatch(shopify_product::matches(url), n, url, || {
                shopify_product::extract(client, url)
            })
            .await
        }
        n if n == shopify_collection::INFO.name => {
            run_or_mismatch(shopify_collection::matches(url), n, url, || {
                shopify_collection::extract(client, url)
            })
            .await
        }
        n if n == ecommerce_product::INFO.name => {
            run_or_mismatch(ecommerce_product::matches(url), n, url, || {
                ecommerce_product::extract(client, url)
            })
            .await
        }
        n if n == woocommerce_product::INFO.name => {
            run_or_mismatch(woocommerce_product::matches(url), n, url, || {
                woocommerce_product::extract(client, url)
            })
            .await
        }
        n if n == amazon_product::INFO.name => {
            run_or_mismatch(amazon_product::matches(url), n, url, || {
                amazon_product::extract(client, url)
            })
            .await
        }
        n if n == ebay_listing::INFO.name => {
            run_or_mismatch(ebay_listing::matches(url), n, url, || {
                ebay_listing::extract(client, url)
            })
            .await
        }
        n if n == etsy_listing::INFO.name => {
            run_or_mismatch(etsy_listing::matches(url), n, url, || {
                etsy_listing::extract(client, url)
            })
            .await
        }
        n if n == trustpilot_reviews::INFO.name => {
            run_or_mismatch(trustpilot_reviews::matches(url), n, url, || {
                trustpilot_reviews::extract(client, url)
            })
            .await
        }
        _ => Err(ExtractorDispatchError::UnknownVertical(name.to_string())),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractorDispatchError {
    #[error("unknown vertical: '{0}'")]
    UnknownVertical(String),
    #[error("URL '{url}' does not match the '{vertical}' extractor")]
    UrlMismatch { vertical: String, url: String },
    #[error(transparent)]
    Fetch(#[from] FetchError),
}

async fn run_or_mismatch<F, Fut>(
    matches: bool,
    vertical: &str,
    url: &str,
    f: F,
) -> Result<Value, ExtractorDispatchError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Value, FetchError>>,
{
    if !matches {
        return Err(ExtractorDispatchError::UrlMismatch {
            vertical: vertical.to_string(),
            url: url.to_string(),
        });
    }
    f().await.map_err(ExtractorDispatchError::Fetch)
}

fn host_matches(url: &str, suffix: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
        .is_some_and(|host| host == suffix || host.ends_with(&format!(".{suffix}")))
}

fn stub_error(name: &str) -> FetchError {
    FetchError::Build(format!("extractor not implemented: {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_contains_all_upstream_extractors() {
        let names: Vec<_> = list().iter().map(|info| info.name).collect();

        assert_eq!(names.len(), 28);
        assert!(names.contains(&"amazon_product"));
        assert!(names.contains(&"youtube_video"));
    }

    #[test]
    fn list_names_are_unique() {
        let mut names: Vec<_> = list().iter().map(|info| info.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();

        assert_eq!(before, names.len(), "extractor names must be unique");
    }
}
