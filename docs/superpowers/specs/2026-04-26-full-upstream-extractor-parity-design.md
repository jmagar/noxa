# Full Upstream Extractor Parity Design

## Goal

Port the full upstream `webclaw-fetch` vertical extractor catalog into Noxa while preserving the existing generic scrape, batch, crawl, CLI, and MCP behavior by default.

Full parity means all 28 upstream vertical extractors:

- `amazon_product`
- `arxiv`
- `crates_io`
- `dev_to`
- `docker_hub`
- `ebay_listing`
- `ecommerce_product`
- `etsy_listing`
- `github_issue`
- `github_pr`
- `github_release`
- `github_repo`
- `hackernews`
- `huggingface_dataset`
- `huggingface_model`
- `instagram_post`
- `instagram_profile`
- `linkedin_post`
- `npm`
- `pypi`
- `reddit`
- `shopify_collection`
- `shopify_product`
- `stackoverflow`
- `substack_post`
- `trustpilot_reviews`
- `woocommerce_product`
- `youtube_video`

## Non-Goals

- Do not replace Noxa's generic content extractor.
- Do not make brittle broad matchers steal URLs from normal scraping.
- Do not add live-network tests. Site behavior and rate limits are too unstable for deterministic CI.
- Do not require API keys for baseline local extractor behavior unless an upstream extractor already depends on an external service.

## Current State

Noxa has generic extraction in `crates/noxa-core/src/extractor.rs` and fetch orchestration in `crates/noxa-fetch/src/client/fetch.rs`. It has site-specific special cases for Reddit and LinkedIn in `crates/noxa-fetch/src/reddit.rs` and `crates/noxa-fetch/src/linkedin.rs`, but it does not have upstream's `extractors/` catalog, catalog listing, explicit extractor dispatch, or typed vertical JSON output.

`noxa_core::ExtractionResult` currently contains:

- `metadata`
- `content`
- `domain_data: Option<DomainData>`
- `structured_data`

`DomainData` only stores a `DomainType`, so it is not sufficient for full vertical extractor payloads.

## Architecture

Add a focused vertical extractor layer in `noxa-fetch`:

- `crates/noxa-fetch/src/extractors/mod.rs` owns catalog listing, auto-dispatch, explicit name dispatch, and dispatch errors.
- Each upstream extractor gets a dedicated file under `crates/noxa-fetch/src/extractors/`.
- Extractors expose `INFO`, `matches(url)`, and `extract(client, url) -> serde_json::Value` following upstream's shape.
- Shared helpers live in small modules under `extractors/` only if duplication becomes concrete during porting.

Keep the dispatcher static rather than dynamic. A static chain matches upstream, keeps ordering explicit, and makes broad-match exclusions easy to audit.

## Output Model

Add an additive field to `noxa_core::ExtractionResult`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub vertical_data: Option<VerticalData>,
```

Where:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalData {
    pub extractor: String,
    pub data: serde_json::Value,
}
```

This preserves backward compatibility: existing JSON consumers keep seeing the same fields, and vertical results appear only when a vertical extractor is selected or auto-detected.

For a vertical hit, Noxa still returns a normal `ExtractionResult`. The generic `metadata` should include the URL, title/description where known, `fetched_at`, and a compact markdown/plain-text summary when the extractor has enough fields to produce one. The complete typed payload lives in `vertical_data.data`.

## Dispatch Behavior

Support two modes:

- Auto mode: the caller uses normal scraping and Noxa tries safe extractors before generic HTML extraction.
- Explicit mode: the caller chooses an extractor by name; Noxa validates `matches(url)` and returns a clear mismatch error if the URL does not belong to that extractor.

Auto-dispatch should include upstream's safe/narrow matchers:

- `reddit`
- `hackernews`
- `github_repo`
- `github_pr`
- `github_issue`
- `github_release`
- `pypi`
- `npm`
- `crates_io`
- `huggingface_model`
- `huggingface_dataset`
- `arxiv`
- `docker_hub`
- `dev_to`
- `stackoverflow`
- `linkedin_post`
- `instagram_post`
- `instagram_profile`
- `amazon_product`
- `ebay_listing`
- `etsy_listing`
- `trustpilot_reviews`
- `youtube_video`

Explicit-only extractors are broad or ambiguous and must not hijack generic scraping:

- `shopify_product`
- `shopify_collection`
- `ecommerce_product`
- `woocommerce_product`
- `substack_post`

## Fetch Integration

Add public methods on `FetchClient`:

- `list_extractors() -> Vec<ExtractorInfo>`
- `fetch_and_extract_vertical(url, extractor_name, options) -> Result<ExtractionResult, FetchError>`
- Internal auto-dispatch hook in `fetch_and_extract_inner` before generic HTML extraction.

Use the existing fetch client abstraction and response caps. Extractors that call JSON APIs should use the same client configuration, proxy/cookie behavior where applicable, timeout behavior, and error types.

Reddit should be reconciled with the existing `crates/noxa-fetch/src/reddit.rs` implementation rather than duplicated blindly. The current hardened verification-wall behavior must remain.

LinkedIn should be reconciled with the existing fallback in `crates/noxa-fetch/src/linkedin.rs`. If upstream `linkedin_post` covers a different output shape, keep old fallback behavior available through generic scraping and expose the upstream vertical shape through `vertical_data`.

## CLI Surface

Add:

- `--extractor <name>` for explicit vertical extraction.
- `--list-extractors` to print extractor catalog.

Default `noxa <url>` remains generic scrape with safe auto-detect. `--extractor` is valid for single URL and batch. Crawl should continue using generic extraction plus safe auto-detect only; explicit vertical extraction across crawl is out of scope unless a future use case requires it.

JSON output includes `vertical_data`. Markdown/text output prints the vertical summary when available, falling back to generic content output.

## MCP Surface

Extend `scrape` params with:

- `extractor: Option<String>`

Add an extractor catalog tool:

- `extractors()` returns the same catalog as CLI `--list-extractors`.

The existing `scrape` tool keeps current behavior when `extractor` is absent. If `extractor` is present, the MCP tool uses explicit dispatch and returns a readable error for unknown extractors or URL mismatches.

## Testing

Use TDD with fixture-backed unit tests.

Required coverage:

- Catalog contains all 28 extractors and unique names.
- Auto-dispatch includes only safe/narrow matchers.
- Explicit dispatch accepts every extractor by name.
- Explicit dispatch returns `UnknownVertical` for invalid names.
- Explicit dispatch returns `UrlMismatch` for wrong URL/extractor combinations.
- Each extractor has matcher tests for positive and negative URL examples.
- Each extractor has fixture parse tests using mocked responses.
- Existing Reddit verification-wall tests continue passing.
- CLI parser tests cover `--extractor` and `--list-extractors`.
- MCP schema/tests cover optional `extractor` and catalog tool.
- Workspace tests pass.

Do not add live tests against GitHub, npm, PyPI, Amazon, Instagram, or any other public site.

## Error Handling

Use typed dispatch errors internally:

- `UnknownVertical(String)`
- `UrlMismatch { vertical, url }`
- `Fetch(FetchError)`

Map these to user-facing CLI/MCP messages without panics.

Extractors should prefer partial structured output over failure when optional fields are absent, but fail when the core resource identity cannot be parsed.

Anti-bot pages, verification walls, and blocked responses should produce actionable errors rather than returning the block page as content.

## Implementation Strategy

Implement in batches while keeping the target scope full parity:

1. Add output model, catalog, dispatcher, and tests with placeholder-free integration.
2. Port low-risk API-backed extractors.
3. Port social/content extractors and reconcile Reddit/LinkedIn.
4. Port ecommerce/review extractors and broad explicit-only matchers.
5. Add CLI/MCP exposure.
6. Run full workspace verification and commit each coherent batch.

The implementation is complete only when all 28 upstream extractors are present, exposed in the catalog, covered by tests, and wired through explicit dispatch.
