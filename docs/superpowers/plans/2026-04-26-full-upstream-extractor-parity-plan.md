# Full Upstream Extractor Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port all 28 upstream `webclaw-fetch` vertical extractors into Noxa with catalog, dispatch, CLI, MCP, and fixture-backed tests.

**Architecture:** Add an additive `vertical_data` payload to `noxa_core::ExtractionResult`, then add `noxa-fetch::extractors` as a static catalog/dispatcher with one file per upstream extractor. Wire safe auto-dispatch into normal scraping and explicit extractor selection into CLI/MCP without changing default generic extraction semantics.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, `thiserror`, `url`, `regex`, `wreq`, existing Noxa fetch/core/CLI/MCP crates, fixture-backed unit tests.

---

## File Structure

- Modify `crates/noxa-core/src/types.rs`: add `VerticalData` and `ExtractionResult::vertical_data`.
- Modify `crates/noxa-core/src/lib.rs` and tests: construct `vertical_data: None` in existing fixtures/results.
- Create `crates/noxa-fetch/src/extractors/mod.rs`: catalog, `ExtractorInfo`, `VerticalDataBuilder`, dispatch, `ExtractorDispatchError`, fixture-test helpers.
- Create `crates/noxa-fetch/src/extractors/http.rs`: small extractor fetch abstraction and `FetchClient` adapter for JSON/HTML calls.
- Create `crates/noxa-fetch/src/extractors/summary.rs`: helpers for turning vertical JSON into markdown/plain text summaries.
- Create one extractor file per upstream vertical:
  `amazon_product.rs`, `arxiv.rs`, `crates_io.rs`, `dev_to.rs`, `docker_hub.rs`, `ebay_listing.rs`, `ecommerce_product.rs`, `etsy_listing.rs`, `github_issue.rs`, `github_pr.rs`, `github_release.rs`, `github_repo.rs`, `hackernews.rs`, `huggingface_dataset.rs`, `huggingface_model.rs`, `instagram_post.rs`, `instagram_profile.rs`, `linkedin_post.rs`, `npm.rs`, `pypi.rs`, `reddit.rs`, `shopify_collection.rs`, `shopify_product.rs`, `stackoverflow.rs`, `substack_post.rs`, `trustpilot_reviews.rs`, `woocommerce_product.rs`, `youtube_video.rs`.
- Create `crates/noxa-fetch/tests/fixtures/extractors/`: JSON/HTML fixtures for all 28 extractors.
- Modify `crates/noxa-fetch/src/lib.rs`: export `extractors` catalog types.
- Modify `crates/noxa-fetch/src/error.rs`: add conversion or variant for extractor dispatch failures.
- Modify `crates/noxa-fetch/src/client/fetch.rs`: auto-dispatch before generic HTML extraction and add explicit vertical method.
- Modify `crates/noxa-fetch/src/client/batch.rs`: add optional explicit extractor path for batch.
- Modify `crates/noxa-cli/src/app/cli.rs`: add `--extractor` and `--list-extractors`.
- Modify `crates/noxa-cli/src/app/entry.rs`: handle list mode before input validation.
- Modify `crates/noxa-cli/src/app/fetching/extract.rs` and batch path: call explicit vertical extraction when requested.
- Modify `crates/noxa-cli/src/app/printing.rs`: print catalog and vertical summaries.
- Modify `crates/noxa-mcp/src/tools.rs`: add `extractor` to `ScrapeParams`.
- Modify `crates/noxa-mcp/src/server.rs` and/or `server/content_tools.rs`: add `extractors` tool and explicit scrape dispatch.
- Modify `crates/noxa-fetch/Cargo.toml`: add dependencies needed by ported extractor code, expected `async-trait = "0.1"` and `regex = "1"`, and possibly `reqwest` only if upstream API code cannot reuse `wreq`.

## Task 1: Add Vertical Output Model

**Files:**
- Modify: `crates/noxa-core/src/types.rs`
- Modify: `crates/noxa-core/src/lib.rs`
- Modify: any test fixture constructors that fail compilation after adding the field

- [ ] **Step 1: Write failing serialization test**

Add a test in `crates/noxa-core/src/lib.rs` or a nearby test module:

```rust
#[test]
fn extraction_result_serializes_vertical_data_when_present() {
    let mut result = extract("<html><body>Hello</body></html>").unwrap();
    result.vertical_data = Some(VerticalData {
        extractor: "github_repo".to_string(),
        data: serde_json::json!({ "repo": "noxa" }),
    });

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["vertical_data"]["extractor"], "github_repo");
    assert_eq!(json["vertical_data"]["data"]["repo"], "noxa");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p noxa-core extraction_result_serializes_vertical_data_when_present -- --nocapture`

Expected: compile failure because `VerticalData`/`vertical_data` does not exist.

- [ ] **Step 3: Implement model**

Add to `crates/noxa-core/src/types.rs`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub vertical_data: Option<VerticalData>,

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalData {
    pub extractor: String,
    pub data: serde_json::Value,
}
```

Export `VerticalData` from `crates/noxa-core/src/lib.rs`.

- [ ] **Step 4: Fix constructors**

Add `vertical_data: None` to every `ExtractionResult` literal that fails compilation.

- [ ] **Step 5: Verify**

Run: `cargo test -p noxa-core extraction_result_serializes_vertical_data_when_present -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noxa-core/src/types.rs crates/noxa-core/src/lib.rs
git commit -m "feat(core): add vertical extractor payload"
```

## Task 2: Add Extractor Catalog and Dispatch Skeleton

**Files:**
- Create: `crates/noxa-fetch/src/extractors/mod.rs`
- Create: `crates/noxa-fetch/src/extractors/http.rs`
- Create: `crates/noxa-fetch/src/extractors/summary.rs`
- Modify: `crates/noxa-fetch/src/lib.rs`
- Modify: `crates/noxa-fetch/src/error.rs`
- Modify: `crates/noxa-fetch/Cargo.toml`

- [ ] **Step 1: Write catalog tests**

Add tests in `extractors/mod.rs` for:

```rust
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
    assert_eq!(before, names.len());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p noxa-fetch extractors::tests::list_contains_all_upstream_extractors -- --nocapture`

Expected: compile failure because `extractors` does not exist.

- [ ] **Step 3: Implement skeleton**

Create `ExtractorInfo`, `list()`, `dispatch_by_url`, `dispatch_by_name`, and `ExtractorDispatchError`. Add every upstream extractor name to the catalog. Initially, modules may expose only `INFO`, `matches`, and parse stubs that return `FetchError::Build("extractor not implemented: <name>")`; do not ship this state beyond the skeleton commit.

- [ ] **Step 4: Add fetch abstraction**

Create a small trait in `extractors/http.rs` for extractor tests:

```rust
#[async_trait::async_trait]
pub trait ExtractorHttp {
    async fn get_text(&self, url: &str) -> Result<String, FetchError>;
    async fn get_json(&self, url: &str) -> Result<serde_json::Value, FetchError>;
}
```

Implement it for `FetchClient` using existing `fetch()` and response limits.

- [ ] **Step 5: Verify**

Run: `cargo test -p noxa-fetch extractors::tests -- --nocapture`

Expected: catalog tests pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noxa-fetch/src/extractors crates/noxa-fetch/src/lib.rs crates/noxa-fetch/src/error.rs crates/noxa-fetch/Cargo.toml
git commit -m "feat(fetch): add vertical extractor catalog"
```

## Task 3: Port API-Backed Developer/Package Extractors

**Files:**
- Modify: `crates/noxa-fetch/src/extractors/github_repo.rs`
- Modify: `crates/noxa-fetch/src/extractors/github_pr.rs`
- Modify: `crates/noxa-fetch/src/extractors/github_issue.rs`
- Modify: `crates/noxa-fetch/src/extractors/github_release.rs`
- Modify: `crates/noxa-fetch/src/extractors/pypi.rs`
- Modify: `crates/noxa-fetch/src/extractors/npm.rs`
- Modify: `crates/noxa-fetch/src/extractors/crates_io.rs`
- Modify: `crates/noxa-fetch/src/extractors/docker_hub.rs`
- Add fixtures under: `crates/noxa-fetch/tests/fixtures/extractors/`

- [ ] **Step 1: Write matcher tests for this batch**

For each extractor, add positive and negative URL examples. Include GitHub ordering tests so repo URLs do not preempt issue/PR/release URLs.

- [ ] **Step 2: Write fixture parse tests**

Use a mock `ExtractorHttp` that maps expected API URLs to fixture JSON. Assert stable fields such as repo name, package name, version, stars/downloads, title, and URL.

- [ ] **Step 3: Run tests to verify failure**

Run: `cargo test -p noxa-fetch extractors::developer -- --nocapture`

Expected: failures from unimplemented extractors.

- [ ] **Step 4: Port upstream implementations**

Use upstream extractor files as the behavioral source, but adapt crate names and fetch calls to Noxa's `ExtractorHttp`. Keep returned JSON field names compatible with upstream unless there is a Noxa-specific conflict.

- [ ] **Step 5: Verify**

Run: `cargo test -p noxa-fetch extractors::developer -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noxa-fetch/src/extractors crates/noxa-fetch/tests/fixtures/extractors
git commit -m "feat(fetch): port developer package extractors"
```

## Task 4: Port Research/Community Content Extractors

**Files:**
- Modify: `crates/noxa-fetch/src/extractors/arxiv.rs`
- Modify: `crates/noxa-fetch/src/extractors/hackernews.rs`
- Modify: `crates/noxa-fetch/src/extractors/dev_to.rs`
- Modify: `crates/noxa-fetch/src/extractors/stackoverflow.rs`
- Modify: `crates/noxa-fetch/src/extractors/youtube_video.rs`
- Add fixtures under: `crates/noxa-fetch/tests/fixtures/extractors/`

- [ ] **Step 1: Write matcher and fixture tests**

Cover canonical URL forms:

- `https://arxiv.org/abs/<id>`
- `https://news.ycombinator.com/item?id=<id>`
- `https://dev.to/<user>/<slug>`
- `https://stackoverflow.com/questions/<id>/<slug>`
- `https://www.youtube.com/watch?v=<id>` and `https://youtu.be/<id>`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noxa-fetch extractors::community -- --nocapture`

Expected: failures from unimplemented extractors.

- [ ] **Step 3: Port implementations**

Prefer upstream API endpoints where present. Keep HTML parsing fixture-driven and avoid live requests.

- [ ] **Step 4: Verify**

Run: `cargo test -p noxa-fetch extractors::community -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noxa-fetch/src/extractors crates/noxa-fetch/tests/fixtures/extractors
git commit -m "feat(fetch): port research and community extractors"
```

## Task 5: Port HuggingFace and Social Extractors

**Files:**
- Modify: `crates/noxa-fetch/src/extractors/huggingface_model.rs`
- Modify: `crates/noxa-fetch/src/extractors/huggingface_dataset.rs`
- Modify: `crates/noxa-fetch/src/extractors/instagram_post.rs`
- Modify: `crates/noxa-fetch/src/extractors/instagram_profile.rs`
- Modify: `crates/noxa-fetch/src/extractors/linkedin_post.rs`
- Modify: `crates/noxa-fetch/src/linkedin.rs` only if reconciliation is required
- Add fixtures under: `crates/noxa-fetch/tests/fixtures/extractors/`

- [ ] **Step 1: Write matcher and fixture tests**

Assert HuggingFace model/dataset disambiguation and Instagram profile/post disambiguation.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noxa-fetch extractors::social -- --nocapture`

Expected: failures from unimplemented extractors.

- [ ] **Step 3: Port implementations**

Keep the existing LinkedIn generic fallback intact. `linkedin_post` should populate `vertical_data`; existing generic LinkedIn extraction remains a fallback for normal content.

- [ ] **Step 4: Verify**

Run: `cargo test -p noxa-fetch extractors::social -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noxa-fetch/src/extractors crates/noxa-fetch/src/linkedin.rs crates/noxa-fetch/tests/fixtures/extractors
git commit -m "feat(fetch): port huggingface and social extractors"
```

## Task 6: Reconcile and Port Reddit Extractor

**Files:**
- Modify: `crates/noxa-fetch/src/extractors/reddit.rs`
- Modify: `crates/noxa-fetch/src/reddit.rs`
- Modify: `crates/noxa-fetch/src/client/fetch.rs`
- Add fixtures under: `crates/noxa-fetch/tests/fixtures/extractors/`

- [ ] **Step 1: Write parity tests**

Test that Reddit vertical extraction uses the hardened JSON endpoint behavior and that verification-wall HTML still fails with a clear error.

- [ ] **Step 2: Run tests to verify failure or current mismatch**

Run: `cargo test -p noxa-fetch reddit -- --nocapture`

Expected: new vertical tests fail until dispatcher integration is complete; existing hardening tests must continue passing.

- [ ] **Step 3: Implement reconciliation**

Avoid duplicate Reddit parsing logic where practical. Either make `extractors/reddit.rs` wrap the hardened parser from `reddit.rs`, or move shared parsing helpers into a private shared module.

- [ ] **Step 4: Verify**

Run: `cargo test -p noxa-fetch reddit -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noxa-fetch/src/extractors/reddit.rs crates/noxa-fetch/src/reddit.rs crates/noxa-fetch/src/client/fetch.rs crates/noxa-fetch/tests/fixtures/extractors
git commit -m "feat(fetch): expose reddit vertical extractor"
```

## Task 7: Port Ecommerce and Review Extractors

**Files:**
- Modify: `crates/noxa-fetch/src/extractors/amazon_product.rs`
- Modify: `crates/noxa-fetch/src/extractors/ebay_listing.rs`
- Modify: `crates/noxa-fetch/src/extractors/ecommerce_product.rs`
- Modify: `crates/noxa-fetch/src/extractors/etsy_listing.rs`
- Modify: `crates/noxa-fetch/src/extractors/shopify_collection.rs`
- Modify: `crates/noxa-fetch/src/extractors/shopify_product.rs`
- Modify: `crates/noxa-fetch/src/extractors/trustpilot_reviews.rs`
- Modify: `crates/noxa-fetch/src/extractors/woocommerce_product.rs`
- Add fixtures under: `crates/noxa-fetch/tests/fixtures/extractors/`

- [ ] **Step 1: Write matcher and broad-dispatch tests**

Assert:

- Amazon/eBay/Etsy/Trustpilot are eligible for auto-dispatch.
- Shopify/ecommerce/WooCommerce broad matchers work in explicit mode.
- Shopify/ecommerce/WooCommerce are not claimed by auto-dispatch.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noxa-fetch extractors::ecommerce -- --nocapture`

Expected: failures from unimplemented extractors.

- [ ] **Step 3: Port implementations**

Preserve upstream anti-bot handling where present. Block/verification pages must produce errors, not vertical payloads.

- [ ] **Step 4: Verify**

Run: `cargo test -p noxa-fetch extractors::ecommerce -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noxa-fetch/src/extractors crates/noxa-fetch/tests/fixtures/extractors
git commit -m "feat(fetch): port ecommerce vertical extractors"
```

## Task 8: Integrate Auto and Explicit Fetch Dispatch

**Files:**
- Modify: `crates/noxa-fetch/src/client/fetch.rs`
- Modify: `crates/noxa-fetch/src/client/batch.rs`
- Modify: `crates/noxa-fetch/src/extractors/mod.rs`
- Modify: `crates/noxa-fetch/src/error.rs`

- [ ] **Step 1: Write integration tests**

Use a fixture/mock HTTP adapter to assert:

- `fetch_and_extract_with_options()` auto-detects a safe vertical and sets `vertical_data`.
- A broad explicit-only URL still goes through generic extraction in auto mode.
- `fetch_and_extract_vertical()` succeeds for matching URL/name.
- `fetch_and_extract_vertical()` fails clearly for mismatch.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noxa-fetch vertical_dispatch -- --nocapture`

Expected: compile failure or failing assertions until integration exists.

- [ ] **Step 3: Implement integration**

Add explicit method:

```rust
pub async fn fetch_and_extract_vertical(
    &self,
    url: &str,
    extractor: &str,
    options: &noxa_core::ExtractionOptions,
) -> Result<noxa_core::ExtractionResult, FetchError>
```

Add safe auto-dispatch before generic HTML extraction but after document/PDF checks when possible. If a vertical extractor needs JSON/API and does not require the fetched HTML, let it run before fetching the original page.

- [ ] **Step 4: Verify**

Run: `cargo test -p noxa-fetch vertical_dispatch -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noxa-fetch/src/client crates/noxa-fetch/src/extractors crates/noxa-fetch/src/error.rs
git commit -m "feat(fetch): wire vertical extractor dispatch"
```

## Task 9: Add CLI Exposure

**Files:**
- Modify: `crates/noxa-cli/src/app/cli.rs`
- Modify: `crates/noxa-cli/src/app/entry.rs`
- Modify: `crates/noxa-cli/src/app/fetching/extract.rs`
- Modify: `crates/noxa-cli/src/app/batch.rs`
- Modify: `crates/noxa-cli/src/app/printing.rs`
- Modify: `crates/noxa-cli/src/app/tests_primary.rs`

- [ ] **Step 1: Write CLI tests**

Add parser/format tests for:

- `noxa --list-extractors`
- `noxa --extractor github_repo https://github.com/jmagar/noxa`
- batch path passes the explicit extractor to fetch
- `--extractor` with `--file` or `--stdin` errors clearly

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noxa-cli extractor -- --nocapture`

Expected: failures because CLI args and list output do not exist.

- [ ] **Step 3: Implement CLI**

Add `extractor: Option<String>` and `list_extractors: bool`. Route explicit extraction through `FetchClient::fetch_and_extract_vertical`. Print catalog as text by default and JSON when `--format json` is selected.

- [ ] **Step 4: Verify**

Run: `cargo test -p noxa-cli extractor -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noxa-cli/src/app
git commit -m "feat(cli): expose vertical extractors"
```

## Task 10: Add MCP Exposure

**Files:**
- Modify: `crates/noxa-mcp/src/tools.rs`
- Modify: `crates/noxa-mcp/src/server.rs`
- Modify: `crates/noxa-mcp/src/server/content_tools.rs` if scrape implementation lives there
- Modify: `crates/noxa-mcp/tests/startup_harness.rs` or add focused tests

- [ ] **Step 1: Write MCP tests**

Add tests or harness assertions for:

- `scrape` schema includes optional `extractor`.
- `extractors` tool is listed.
- explicit extractor mismatch returns a readable tool error.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p noxa-mcp extractor -- --nocapture`

Expected: failures because schema/tool is missing.

- [ ] **Step 3: Implement MCP**

Add `extractor: Option<String>` to `ScrapeParams`; use explicit dispatch when provided. Add `extractors` tool returning pretty JSON catalog.

- [ ] **Step 4: Verify**

Run: `cargo test -p noxa-mcp extractor -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noxa-mcp/src crates/noxa-mcp/tests
git commit -m "feat(mcp): expose vertical extractors"
```

## Task 11: Documentation and Final Verification

**Files:**
- Modify: `README.md` if extractor usage belongs there
- Modify: `crates/noxa-mcp/README.md`
- Modify: any CLI docs/help snapshots if present
- Modify: `docs/superpowers/plans/2026-04-26-full-upstream-extractor-parity-plan.md` if implementation discoveries require updates

- [ ] **Step 1: Add docs**

Document:

- `noxa --list-extractors`
- `noxa --extractor <name> <url>`
- MCP `extractors` tool
- MCP `scrape.extractor`
- Auto-dispatch vs explicit-only behavior

- [ ] **Step 2: Run focused crate tests**

Run:

```bash
cargo test -p noxa-core
cargo test -p noxa-fetch
cargo test -p noxa-cli
cargo test -p noxa-mcp
```

Expected: all PASS.

- [ ] **Step 3: Run workspace tests**

Run: `cargo test --workspace`

Expected: all PASS. Existing ignored tests may remain ignored.

- [ ] **Step 4: Run build**

Run: `cargo build --workspace`

Expected: PASS.

- [ ] **Step 5: Update Beads**

Run:

```bash
bd close noxa-x2x --reason "Implemented full upstream vertical extractor parity"
```

- [ ] **Step 6: Commit docs/final fixes**

Run:

```bash
git add README.md crates/noxa-mcp/README.md docs/superpowers/plans/2026-04-26-full-upstream-extractor-parity-plan.md
git commit -m "docs: document vertical extractor parity"
```

## Review Notes

The spec-review and plan-review subagent loops from the superpowers workflow were not run automatically because this Codex environment only permits spawning subagents when the user explicitly asks for subagent delegation. If the user asks for agent review, dispatch a plan/spec reviewer before implementation.

## Completion Criteria

- All 28 upstream extractor names are present in `noxa_fetch::extractors::list()`.
- Every extractor has URL matcher coverage and fixture-backed parse coverage.
- Safe auto-dispatch does not include broad Shopify/ecommerce/WooCommerce/Substack matchers.
- Explicit dispatch works for all extractors.
- Existing generic scrape behavior remains compatible.
- CLI and MCP expose catalog and explicit extractor selection.
- `cargo test --workspace` and `cargo build --workspace` pass.
