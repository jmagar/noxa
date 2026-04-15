# Bead Status Review

Date: 2026-04-14

This review is based on current repository code, targeted test runs, and bead metadata. It is not a tracker-only audit.

## Already Closed

### `noxa-tpc`

- What it is: Gemini CLI as the primary LLM provider for `noxa-llm`, with fallback providers and schema-validated JSON extraction.
- Suggestion: Closed.
- Why:
  - `GeminiCliProvider` exists in `crates/noxa-llm/src/providers/gemini_cli.rs`.
  - Provider ordering is Gemini-first in `crates/noxa-llm/src/chain.rs`.
  - Schema validation and retry are implemented in `crates/noxa-llm/src/extract.rs`.
  - CLI/docs wiring is present in `crates/noxa-cli/src/main.rs`, `README.md`, and `CLAUDE.md`.
  - Verified with `cargo test -p noxa-llm gemini -- --nocapture`.

### `noxa-21v`

- What it is: Wire SearXNG into the MCP search tool.
- Suggestion: Closed.
- Why:
  - MCP search reads `SEARXNG_URL` and calls `noxa_fetch::searxng_search()` in `crates/noxa-mcp/src/server.rs`.
  - Result URLs are validated and scraped through `fetch_and_extract_batch`.
  - The CLI search path is also wired in `crates/noxa-cli/src/main.rs`.

## Update Description / Status

### `noxa-k47`

- What it is: Full doc lifecycle for the content store: list, retrieve, status, refresh, background crawl, and tests.
- Suggestion: Update description/status, keep open.
- Why:
  - Implemented now:
    - `run_list`
    - `run_retrieve`
    - `run_status`
    - `spawn_crawl_background`
    - content-store integration via `noxa-store`
  - Remaining:
    - `--refresh` still does not appear implemented.
    - The epic description still reads like a near-greenfield plan even though much of the CLI surface already exists.
  - Best tracker action:
    - Update the bead to say the list/retrieve/status/background-crawl work is landed.
    - Leave refresh/testing/hardening as remaining scope.

### `noxa-241`

- What it is: Master epic for noxa-rag v1 foundation, including schema, ingestion, observability, and related tracks.
- Suggestion: Update description/status, keep open.
- Why:
  - Implemented now:
    - expanded `Metadata` fields in `crates/noxa-core/src/types.rs`
    - expanded `PointPayload` and `SearchResult` in `crates/noxa-rag/src/types.rs`
    - `IngestionContext` exists
    - `file://` handling and multi-format ingestion are in `crates/noxa-rag/src/pipeline.rs`
  - Remaining:
    - the full epic scope is broader than what is landed
    - several Wave 3 / MCP / observability items are not clearly implemented
  - Best tracker action:
    - Mark major foundation pieces as implemented.
    - Reframe the epic around what still blocks v1 instead of the original full plan.

### `noxa-vig`

- What it is: Comprehensive metadata schema for all RAG source types.
- Suggestion: Update description/status, keep open.
- Why:
  - Implemented now:
    - core metadata additions such as `content_hash`, `source_type`, `file_path`, `last_modified`, `is_truncated`, `technologies`, `seed_url`, `crawl_depth`, `search_query`, `fetched_at`
    - rag payload/search result additions such as `title`, `author`, `language`, `source_type`, `content_hash`, `technologies`, provenance placeholders
  - Remaining:
  - much of the long-tail source-specific schema in the epic is not present
  - Qdrant indexes in code are narrower than the epic proposes
  - Best tracker action:
    - Split “landed minimal metadata foundation” from a concrete next step.
    - Keep the follow-up bead focused on email and RSS provenance/dedup fields.
    - Leave MCP/platform, presentation, and subtitle enrichment parked until a source lands.

### `noxa-dki`

- What it is: Multi-format local file ingestion for noxa-rag.
- Suggestion: Update description/status, keep open.
- Why:
  - Implemented now:
    - `file://` support via `Url::from_file_path`
    - broader `is_indexable()`
    - parsing for many formats including markdown/text/log/rst/org/yaml/toml/html/ipynb/pdf/docx/odt/pptx/jsonl/xml/opml/rss/atom/vtt/srt
    - tests in `crates/noxa-rag/src/pipeline.rs`
    - verified with `cargo test -p noxa-rag pipeline -- --nocapture`
  - Remaining:
    - the original epic scope includes formats and behaviors not clearly landed, such as EPUB and email formats
    - some proposed metadata extraction is still partial
  - Best tracker action:
    - Update the epic to say “core multi-format ingestion landed”.
    - Track missing formats and missing metadata as follow-up work.

## Split Remaining Work

### `noxa-2uu`

- What it is: Add `SEARXNG_URL` to env example and docs.
- Suggestion: Split remaining work.
- Why:
  - Docs are already updated in `README.md` and `CLAUDE.md`.
  - There is no `env.example` or `.env.example` file in the repo to update.
  - The current bead bundles two different states:
    - done: docs
    - unresolved: env-example strategy
- Best tracker action:
  - Close or narrow the existing docs portion.
  - Create a follow-up bead only if the repo should gain an env example file.

### `noxa-k47`

- What it is: Doc lifecycle/content-store epic.
- Suggestion: Split remaining work.
- Why:
  - The bead mixes landed commands with unfinished features.
  - The clean split is:
    - landed lifecycle commands
    - `--refresh`
    - additional tests and hardening
- Best tracker action:
  - Keep the epic, but create follow-up child work for the unfinished pieces only.

### `noxa-dki`

- What it is: Multi-format ingestion epic.
- Suggestion: Split remaining work.
- Why:
  - Core ingestion is clearly in the repo.
  - The remaining work is mostly “missing formats / polish” rather than “build ingestion”.
- Best tracker action:
  - Create targeted follow-ups for:
    - EPUB
    - email formats
    - any metadata extraction still missing for landed formats

### `noxa-241`

- What it is: Master RAG v1 foundation epic.
- Suggestion: Split remaining work.
- Why:
  - It currently bundles foundation, observability, MCP sources, and other tracks.
  - Parts of the foundation are already present, while other areas are untouched.
- Best tracker action:
  - Preserve the epic.
  - Split outstanding work into narrower “still not landed” follow-ups rather than carrying the original broad plan unchanged.

### `noxa-vig`

- What it is: Metadata-schema epic.
- Suggestion: Split remaining work.
- Why:
  - Minimal viable metadata foundation is already implemented.
  - The epic still describes a much larger universal schema than the code currently supports.
- Best tracker action:
  - Separate “implemented v1 schema” from “future source-specific enrichment”.

## Leave As-Is

### `noxa-l4r`

- What it is: MCP proxy gateway with OAuth.
- Suggestion: Leave as-is.
- Why:
  - I did not find a `noxa-gateway` crate or equivalent gateway implementation in `crates/`.
  - The bead still appears to describe future work accurately.

### `noxa-lsg`

- What it is: MCP server source ingestion via `mcporter`.
- Suggestion: Leave as-is.
- Why:
  - I found provenance placeholders like `external_id`, but no `mcporter` integration, no MCP source config, and no source ingestion implementation.
  - The bead still matches the code reality.

### `noxa-1u6`

- What it is: Extract shared `noxa-http` HTTP utility crate.
- Suggestion: Leave as-is.
- Why:
  - There is no `noxa-http` crate under `crates/`.
  - Existing references are historical comments or old naming, not an implemented shared crate.

### `noxa-2dm`

- What it is: Paperless-ngx MCP ingestion.
- Suggestion: Leave as-is.
- Why:
  - No paperless ingestor implementation was found.
  - The bead still describes future work.

### `noxa-8qb`

- What it is: Linkding bookmark ingestion.
- Suggestion: Leave as-is.
- Why:
  - No linkding source ingestion implementation was found.
  - The bead still describes future work.

### `noxa-k55`

- What it is: Memos + bytestash ingestion.
- Suggestion: Leave as-is.
- Why:
  - No memos/bytestash ingestion implementation was found.
  - The bead still describes future work.

### `noxa-z9z`

- What it is: Full build + smoke tests for the SearXNG search feature.
- Suggestion: Leave as-is.
- Why:
  - I did not run the full workspace build/smoke matrix described by the bead.
  - There is not enough evidence to close or rewrite it aggressively.

## Short Summary

- Closed already because code is present and verified:
  - `noxa-tpc`
  - `noxa-21v`
- Open but should be rewritten to reflect landed code:
  - `noxa-k47`
  - `noxa-241`
  - `noxa-vig`
  - `noxa-dki`
- Open and should be decomposed because the current bead mixes done and not-done work:
  - `noxa-2uu`
  - `noxa-k47`
  - `noxa-dki`
  - `noxa-241`
  - `noxa-vig`
- Open and still accurately describe future work:
  - `noxa-l4r`
  - `noxa-lsg`
  - `noxa-1u6`
  - `noxa-2dm`
  - `noxa-8qb`
  - `noxa-k55`
  - `noxa-z9z`
