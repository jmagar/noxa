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

    pub mod developer {
        use std::collections::BTreeMap;

        use async_trait::async_trait;

        use super::*;

        struct FixtureHttp {
            json: BTreeMap<&'static str, &'static str>,
        }

        impl FixtureHttp {
            fn new(entries: &[(&'static str, &'static str)]) -> Self {
                Self {
                    json: entries.iter().copied().collect(),
                }
            }
        }

        #[async_trait]
        impl http::ExtractorHttp for FixtureHttp {
            async fn get_text(&self, url: &str) -> Result<String, FetchError> {
                self.json
                    .get(url)
                    .map(|body| (*body).to_string())
                    .ok_or_else(|| FetchError::Build(format!("missing fixture for {url}")))
            }

            async fn get_json(&self, url: &str) -> Result<Value, FetchError> {
                let body = self.get_text(url).await?;
                serde_json::from_str(&body).map_err(|error| FetchError::BodyDecode(error.to_string()))
            }
        }

        #[test]
        fn developer_matchers_accept_expected_urls() {
            assert!(github_repo::matches("https://github.com/jmagar/noxa"));
            assert!(github_pr::matches("https://github.com/jmagar/noxa/pull/12"));
            assert!(github_issue::matches("https://github.com/jmagar/noxa/issues/34"));
            assert!(github_release::matches(
                "https://github.com/jmagar/noxa/releases/tag/v0.7.0"
            ));
            assert!(pypi::matches("https://pypi.org/project/requests/"));
            assert!(npm::matches("https://www.npmjs.com/package/@types/node"));
            assert!(crates_io::matches("https://crates.io/crates/serde"));
            assert!(docker_hub::matches("https://hub.docker.com/_/nginx"));
        }

        #[test]
        fn github_repo_does_not_preempt_more_specific_github_extractors() {
            assert!(!github_repo::matches("https://github.com/jmagar/noxa/pull/12"));
            assert!(!github_repo::matches("https://github.com/jmagar/noxa/issues/34"));
            assert!(!github_repo::matches(
                "https://github.com/jmagar/noxa/releases/tag/v0.7.0"
            ));
            assert!(!github_repo::matches("https://github.com/topics/rust"));
        }

        #[tokio::test]
        async fn developer_extractors_parse_fixture_payloads() {
            let client = FixtureHttp::new(&[
                (
                    "https://api.github.com/repos/jmagar/noxa",
                    include_str!("../../tests/fixtures/extractors/github_repo.json"),
                ),
                (
                    "https://api.github.com/repos/jmagar/noxa/pulls/12",
                    include_str!("../../tests/fixtures/extractors/github_pr.json"),
                ),
                (
                    "https://api.github.com/repos/jmagar/noxa/issues/34",
                    include_str!("../../tests/fixtures/extractors/github_issue.json"),
                ),
                (
                    "https://api.github.com/repos/jmagar/noxa/releases/tags/v0.7.0",
                    include_str!("../../tests/fixtures/extractors/github_release.json"),
                ),
                (
                    "https://pypi.org/pypi/requests/json",
                    include_str!("../../tests/fixtures/extractors/pypi.json"),
                ),
                (
                    "https://registry.npmjs.org/%40types%2Fnode",
                    include_str!("../../tests/fixtures/extractors/npm_registry.json"),
                ),
                (
                    "https://api.npmjs.org/downloads/point/last-week/%40types%2Fnode",
                    include_str!("../../tests/fixtures/extractors/npm_downloads.json"),
                ),
                (
                    "https://crates.io/api/v1/crates/serde",
                    include_str!("../../tests/fixtures/extractors/crates_io.json"),
                ),
                (
                    "https://hub.docker.com/v2/repositories/library/nginx",
                    include_str!("../../tests/fixtures/extractors/docker_hub.json"),
                ),
            ]);

            let repo = github_repo::extract(&client, "https://github.com/jmagar/noxa").await.unwrap();
            assert_eq!(repo["full_name"], "jmagar/noxa");
            assert_eq!(repo["stars"], 42);

            let pr = github_pr::extract(&client, "https://github.com/jmagar/noxa/pull/12").await.unwrap();
            assert_eq!(pr["number"], 12);
            assert_eq!(pr["title"], "Port upstream extractors");

            let issue =
                github_issue::extract(&client, "https://github.com/jmagar/noxa/issues/34").await.unwrap();
            assert_eq!(issue["number"], 34);
            assert_eq!(issue["labels"][0], "bug");

            let release = github_release::extract(
                &client,
                "https://github.com/jmagar/noxa/releases/tag/v0.7.0",
            )
            .await
            .unwrap();
            assert_eq!(release["tag_name"], "v0.7.0");
            assert_eq!(release["total_downloads"], 7);

            let pypi = pypi::extract(&client, "https://pypi.org/project/requests/").await.unwrap();
            assert_eq!(pypi["name"], "requests");
            assert_eq!(pypi["version"], "2.32.3");

            let npm =
                npm::extract(&client, "https://www.npmjs.com/package/@types/node").await.unwrap();
            assert_eq!(npm["name"], "@types/node");
            assert_eq!(npm["weekly_downloads"], 123456);

            let crate_data =
                crates_io::extract(&client, "https://crates.io/crates/serde").await.unwrap();
            assert_eq!(crate_data["name"], "serde");
            assert_eq!(crate_data["downloads"], 1000);

            let docker = docker_hub::extract(&client, "https://hub.docker.com/_/nginx")
                .await
                .unwrap();
            assert_eq!(docker["full_name"], "library/nginx");
            assert_eq!(docker["pull_count"], 5000);
        }
    }

    pub mod community {
        use std::collections::BTreeMap;

        use async_trait::async_trait;

        use super::*;

        struct FixtureHttp {
            bodies: BTreeMap<&'static str, &'static str>,
        }

        impl FixtureHttp {
            fn new(entries: &[(&'static str, &'static str)]) -> Self {
                Self {
                    bodies: entries.iter().copied().collect(),
                }
            }
        }

        #[async_trait]
        impl http::ExtractorHttp for FixtureHttp {
            async fn get_text(&self, url: &str) -> Result<String, FetchError> {
                self.bodies
                    .get(url)
                    .map(|body| (*body).to_string())
                    .ok_or_else(|| FetchError::Build(format!("missing fixture for {url}")))
            }

            async fn get_json(&self, url: &str) -> Result<Value, FetchError> {
                let body = self.get_text(url).await?;
                serde_json::from_str(&body).map_err(|error| FetchError::BodyDecode(error.to_string()))
            }
        }

        #[test]
        fn community_matchers_accept_expected_urls() {
            assert!(arxiv::matches("https://arxiv.org/abs/2401.12345v2"));
            assert!(hackernews::matches("https://news.ycombinator.com/item?id=123"));
            assert!(dev_to::matches("https://dev.to/jmagar/porting-noxa"));
            assert!(stackoverflow::matches(
                "https://stackoverflow.com/questions/12345/how-to-test-rust"
            ));
            assert!(youtube_video::matches("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
            assert!(youtube_video::matches("https://youtu.be/dQw4w9WgXcQ"));
        }

        #[tokio::test]
        async fn community_extractors_parse_fixture_payloads() {
            let client = FixtureHttp::new(&[
                (
                    "https://export.arxiv.org/api/query?id_list=2401.12345",
                    include_str!("../../tests/fixtures/extractors/arxiv.xml"),
                ),
                (
                    "https://hn.algolia.com/api/v1/items/123",
                    include_str!("../../tests/fixtures/extractors/hackernews.json"),
                ),
                (
                    "https://dev.to/api/articles/jmagar/porting-noxa",
                    include_str!("../../tests/fixtures/extractors/dev_to.json"),
                ),
                (
                    "https://api.stackexchange.com/2.3/questions/12345?site=stackoverflow&filter=withbody",
                    include_str!("../../tests/fixtures/extractors/stackoverflow_question.json"),
                ),
                (
                    "https://api.stackexchange.com/2.3/questions/12345/answers?site=stackoverflow&filter=withbody&order=desc&sort=votes",
                    include_str!("../../tests/fixtures/extractors/stackoverflow_answers.json"),
                ),
                (
                    "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                    include_str!("../../tests/fixtures/extractors/youtube_video.html"),
                ),
            ]);

            let paper = arxiv::extract(&client, "https://arxiv.org/abs/2401.12345v2")
                .await
                .unwrap();
            assert_eq!(paper["id"], "2401.12345");
            assert_eq!(paper["title"], "A Test Paper");
            assert_eq!(paper["authors"][0], "Ada Lovelace");

            let hn = hackernews::extract(&client, "https://news.ycombinator.com/item?id=123")
                .await
                .unwrap();
            assert_eq!(hn["post"]["id"], 123);
            assert_eq!(hn["post"]["comment_count"], 1);

            let article = dev_to::extract(&client, "https://dev.to/jmagar/porting-noxa")
                .await
                .unwrap();
            assert_eq!(article["title"], "Porting Noxa");
            assert_eq!(article["author"]["username"], "jmagar");

            let question = stackoverflow::extract(
                &client,
                "https://stackoverflow.com/questions/12345/how-to-test-rust",
            )
            .await
            .unwrap();
            assert_eq!(question["question_id"], 12345);
            assert_eq!(question["accepted_answer"]["answer_id"], 99);

            let video = youtube_video::extract(
                &client,
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            )
            .await
            .unwrap();
            assert_eq!(video["video_id"], "dQw4w9WgXcQ");
            assert_eq!(video["title"], "Test Video");
            assert_eq!(video["view_count"], 1000);
        }
    }

    pub mod social {
        use std::collections::BTreeMap;

        use async_trait::async_trait;

        use super::*;

        struct FixtureHttp {
            bodies: BTreeMap<&'static str, &'static str>,
        }

        impl FixtureHttp {
            fn new(entries: &[(&'static str, &'static str)]) -> Self {
                Self {
                    bodies: entries.iter().copied().collect(),
                }
            }
        }

        #[async_trait]
        impl http::ExtractorHttp for FixtureHttp {
            async fn get_text(&self, url: &str) -> Result<String, FetchError> {
                self.bodies
                    .get(url)
                    .map(|body| (*body).to_string())
                    .ok_or_else(|| FetchError::Build(format!("missing fixture for {url}")))
            }

            async fn get_json(&self, url: &str) -> Result<Value, FetchError> {
                let body = self.get_text(url).await?;
                serde_json::from_str(&body).map_err(|error| FetchError::BodyDecode(error.to_string()))
            }
        }

        #[test]
        fn social_matchers_disambiguate_urls() {
            assert!(huggingface_model::matches("https://huggingface.co/openai/whisper-large-v3"));
            assert!(!huggingface_model::matches("https://huggingface.co/datasets/openai/gsm8k"));
            assert!(huggingface_dataset::matches("https://huggingface.co/datasets/openai/gsm8k"));
            assert!(instagram_post::matches("https://www.instagram.com/p/ABC123/"));
            assert!(instagram_post::matches("https://www.instagram.com/reel/ABC123/"));
            assert!(!instagram_profile::matches("https://www.instagram.com/p/ABC123/"));
            assert!(instagram_profile::matches("https://www.instagram.com/jmagar/"));
            assert!(linkedin_post::matches(
                "https://www.linkedin.com/feed/update/urn:li:activity:7452618583290892288"
            ));
        }

        #[tokio::test]
        async fn social_extractors_parse_fixture_payloads() {
            let client = FixtureHttp::new(&[
                (
                    "https://huggingface.co/api/models/openai/whisper-large-v3",
                    include_str!("../../tests/fixtures/extractors/huggingface_model.json"),
                ),
                (
                    "https://huggingface.co/api/datasets/openai/gsm8k",
                    include_str!("../../tests/fixtures/extractors/huggingface_dataset.json"),
                ),
                (
                    "https://www.instagram.com/p/ABC123/embed/captioned/",
                    include_str!("../../tests/fixtures/extractors/instagram_post.html"),
                ),
                (
                    "https://www.instagram.com/api/v1/users/web_profile_info/?username=jmagar",
                    include_str!("../../tests/fixtures/extractors/instagram_profile.json"),
                ),
                (
                    "https://www.linkedin.com/embed/feed/update/urn:li:activity:7452618583290892288",
                    include_str!("../../tests/fixtures/extractors/linkedin_post.html"),
                ),
            ]);

            let model =
                huggingface_model::extract(&client, "https://huggingface.co/openai/whisper-large-v3")
                    .await
                    .unwrap();
            assert_eq!(model["model_id"], "openai/whisper-large-v3");
            assert_eq!(model["file_count"], 1);

            let dataset = huggingface_dataset::extract(
                &client,
                "https://huggingface.co/datasets/openai/gsm8k",
            )
            .await
            .unwrap();
            assert_eq!(dataset["id"], "openai/gsm8k");
            assert_eq!(dataset["downloads"], 200);

            let post = instagram_post::extract(&client, "https://www.instagram.com/p/ABC123/")
                .await
                .unwrap();
            assert_eq!(post["shortcode"], "ABC123");
            assert_eq!(post["author_username"], "jmagar");

            let profile =
                instagram_profile::extract(&client, "https://www.instagram.com/jmagar/")
                    .await
                    .unwrap();
            assert_eq!(profile["username"], "jmagar");
            assert_eq!(profile["recent_posts"][0]["shortcode"], "ABC123");

            let linked = linkedin_post::extract(
                &client,
                "https://www.linkedin.com/feed/update/urn:li:activity:7452618583290892288",
            )
            .await
            .unwrap();
            assert_eq!(linked["urn"], "urn:li:activity:7452618583290892288");
            assert_eq!(linked["author_name"], "Jacob Magar");
        }
    }

    pub mod reddit_vertical {
        use std::collections::BTreeMap;

        use async_trait::async_trait;

        use super::*;

        struct FixtureHttp {
            bodies: BTreeMap<&'static str, &'static str>,
        }

        #[async_trait]
        impl http::ExtractorHttp for FixtureHttp {
            async fn get_text(&self, url: &str) -> Result<String, FetchError> {
                self.bodies
                    .get(url)
                    .map(|body| (*body).to_string())
                    .ok_or_else(|| FetchError::Build(format!("missing fixture for {url}")))
            }

            async fn get_json(&self, url: &str) -> Result<Value, FetchError> {
                let body = self.get_text(url).await?;
                serde_json::from_str(&body).map_err(|error| FetchError::BodyDecode(error.to_string()))
            }
        }

        #[tokio::test]
        async fn reddit_vertical_uses_hardened_json_parser() {
            let client = FixtureHttp {
                bodies: [(
                    "https://www.reddit.com/r/rust/comments/abc123/release_thread.json",
                    include_str!("../../tests/fixtures/extractors/reddit.json"),
                )]
                .into_iter()
                .collect(),
            };

            let value = reddit::extract(
                &client,
                "https://www.reddit.com/r/rust/comments/abc123/release_thread/",
            )
            .await
            .unwrap();

            assert_eq!(value["metadata"]["title"], "Rust release thread");
            assert!(
                value["content"]["plain_text"]
                    .as_str()
                    .unwrap()
                    .contains("Thanks for the update!")
            );
        }

        #[tokio::test]
        async fn reddit_vertical_rejects_verify_wall_html() {
            let client = FixtureHttp {
                bodies: [(
                    "https://www.reddit.com/r/rust/comments/abc123/release_thread.json",
                    "<html><body>Whoa there, verify you are human</body></html>",
                )]
                .into_iter()
                .collect(),
            };

            let err = reddit::extract(
                &client,
                "https://www.reddit.com/r/rust/comments/abc123/release_thread/",
            )
            .await
            .expect_err("verify wall must not parse as reddit JSON");

            assert!(err.to_string().contains("verification"));
        }
    }
}
