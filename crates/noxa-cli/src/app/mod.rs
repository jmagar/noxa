use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{CommandFactory, FromArgMatches, Parser, ValueEnum};
use noxa_core::{
    ChangeStatus, ContentDiff, ExtractionOptions, ExtractionResult, Metadata, extract_with_options,
    to_llm_text,
};
use noxa_fetch::{
    BatchExtractResult, BodyRetention, BrowserProfile, CrawlConfig, CrawlResult, Crawler,
    FetchClient, FetchConfig, FetchResult, PageResult, SitemapEntry,
};
use noxa_llm::LlmProvider;
use noxa_pdf::PdfMode;
use noxa_store::{
    FilesystemContentStore, FilesystemOperationsLog, Op, OperationEntry, domain_from_url,
    is_private_or_reserved_ip, parse_http_url, url_to_store_path, validate_public_http_url,
};
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

use crate::{cloud, config, theme::*};

mod batch;
mod cli;
mod crawl;
mod crawl_status;
mod crawl_watch;
mod diff_brand;
mod entry;
mod rag_daemon;
mod rag_watch;
mod store_watch;
mod watch_singleton;
mod fetching {
    pub(crate) mod config;
    pub(crate) mod extract;
    pub(crate) mod inputs;
    pub(crate) mod storage;
}
mod formatting;
mod llm;
mod logging;
mod printing;
mod refresh;
mod research;
mod retrieve;
mod store_ops;
#[cfg(test)]
mod tests_primary;
#[cfg(test)]
mod tests_secondary;
mod watch;

pub(crate) use batch::run_batch;
pub(crate) use cli::{Browser, Cli, OutputFormat, PdfModeArg};
pub(crate) use crawl::{run_crawl, run_map, spawn_crawl_background};
pub(crate) use crawl_status::*;
pub(crate) use crawl_watch::run_crawl_watch;
pub(crate) use diff_brand::{run_brand, run_diff};
pub(crate) use entry::run;
pub(crate) use fetching::config::{
    build_extraction_options, build_fetch_config, content_store_root, normalize_url, validate_url,
};
pub(crate) use fetching::extract::{enrich_html_with_stylesheets, fetch_and_extract, fetch_html};
pub(crate) use fetching::inputs::{FetchOutput, collect_urls};
pub(crate) use fetching::storage::{
    print_save_hint, validate_operator_url, validate_url_sync, write_to_file,
};
pub(crate) use formatting::{
    clamp_search_scrape_concurrency, format_cloud_output, format_map_output, format_output,
    format_progress, raw_html_or_markdown,
};
pub(crate) use llm::{has_llm_flags, run_batch_llm, run_llm};
pub(crate) use logging::{build_ops_log, init_logging, init_mcp_logging, log_operation};
#[cfg(test)]
pub(crate) use printing::format_extractor_catalog;
pub(crate) use printing::{
    print_batch_output, print_cloud_output, print_crawl_output, print_diff_output,
    print_extractor_catalog, print_map_output, print_output,
};
pub(crate) use rag_daemon::{run_rag_start, run_rag_stop};
pub(crate) use rag_watch::run_rag_watch;
pub(crate) use refresh::{run_refresh, run_status};
pub(crate) use research::run_research;
pub(crate) use retrieve::run_retrieve;
pub(crate) use store_ops::{run_grep, run_list, run_search};
pub(crate) use store_watch::run_store_watch;
pub(crate) use watch::{fire_webhook, run_watch};

#[cfg(test)]
pub(crate) use crawl_status::{crawl_log_path_for_home, crawl_status_path_for_home};
#[cfg(test)]
pub(crate) use fetching::config::url_to_filename;
#[cfg(test)]
pub(crate) use formatting::default_search_dir;
#[cfg(test)]
pub(crate) use watch::run_on_change_command;

/// Known anti-bot challenge page titles (case-insensitive prefix match).
const ANTIBOT_TITLES: &[&str] = &[
    "just a moment",
    "attention required",
    "access denied",
    "checking your browser",
    "please wait",
    "one more step",
    "verify you are human",
    "bot verification",
    "security check",
    "ddos protection",
];

/// Detect why a page returned empty content.
pub(crate) enum EmptyReason {
    Antibot,
    JsRequired,
    None,
}

pub(crate) fn detect_empty(result: &ExtractionResult) -> EmptyReason {
    if result.metadata.word_count > 50 || !result.content.markdown.is_empty() {
        return EmptyReason::None;
    }

    if let Some(ref title) = result.metadata.title {
        let lower = title.to_lowercase();
        if ANTIBOT_TITLES.iter().any(|t| lower.starts_with(t)) {
            return EmptyReason::Antibot;
        }
    }

    if result.metadata.word_count == 0 && result.content.links.is_empty() {
        return EmptyReason::JsRequired;
    }

    EmptyReason::None
}

/// Strip all ANSI/VT escape sequences and control characters from user-supplied
/// strings before printing to prevent terminal injection.
pub(crate) fn sanitize_display(s: &str) -> String {
    strip_ansi_escapes::strip_str(s)
        .chars()
        .filter(|&c| !c.is_control() || c == '\t')
        .collect()
}

pub(crate) fn warn_empty(url: &str, reason: &EmptyReason) {
    match reason {
        EmptyReason::Antibot => eprintln!(
            "{}
This site requires CAPTCHA solving or browser rendering.
Use the noxa Cloud API for automatic bypass: https://noxa.io/pricing",
            warning(&format!("Anti-bot protection detected on {url}"))
        ),
        EmptyReason::JsRequired => eprintln!(
            "{}
This site requires JavaScript rendering (SPA).
Use the noxa Cloud API for JS rendering: https://noxa.io/pricing",
            warning(&format!("No content extracted from {url}"))
        ),
        EmptyReason::None => {}
    }
}
