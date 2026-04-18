# Content Store

`noxa-store` is the filesystem persistence crate for noxa. It stores the latest
sanitized extraction for each URL and a per-domain operations log.

## Current Layout

```text
<store_root>/                      # default: ~/.noxa/content/
  <domain>/
    .operations.ndjson             # append-only domain log
    <path>.md                      # latest markdown snapshot
    <path>.json                    # sidecar with current extraction + changelog
```

There is no `.index.ndjson` file and no `index.rs` module in the current crate.

## URL Identity

- Store paths are derived from validated URLs.
- Hostnames are sanitized and `www.` is stripped.
- Query strings are never persisted directly in filenames.
- When the sanitized relative path would exceed the path budget, the crate
  truncates the prefix and appends a stable hash of the full URL so long URLs do
  not alias on disk.
- Invalid URLs are rejected by the content-store API with
  `StoreError::InvalidUrl`; they do not fall back to a shared `"unknown"` path.

## Content Store Semantics

`FilesystemContentStore` owns per-URL persistence.

- `write(url, extraction)` stores two files:
  - `<path>.md` with the latest markdown
  - `<path>.json` with a versioned `Sidecar`
- The first write creates a new sidecar with one changelog entry whose `diff` is
  `null`.
- Subsequent writes:
  - update `last_fetched` and `fetch_count` on every fetch
  - recompute a `ContentDiff`
  - append a changelog entry only when content changed
  - rewrite `<path>.md` only on the first write or on content change
- Sidecars are written atomically with same-directory temp files and rename.
- Legacy sidecars containing a raw `ExtractionResult` are migrated in memory on
  read and rewritten in the new envelope format on the next successful write.
- Corrupt sidecars are not silently replaced. Reads and writes return
  `StoreError::CorruptSidecar` with the path that failed to parse.

## Sanitization and Budgeting

Before persistence, the store strips:

- `content.raw_html`
- `metadata.file_path`
- `metadata.search_query`
- query parameters from `metadata.url`

`max_content_bytes` is enforced against the sanitized persisted payload:

- counted bytes = `markdown.len() + plain_text.len()`
- default limit = 2 MiB
- `None` disables the guard

If the sanitized payload exceeds the limit, the store skips the write and
returns a `StoreResult` with `is_new = false` and `changed = false`.

## Root Initialization and Path Safety

- `FilesystemContentStore::open()` uses `~/.noxa/content/`.
- If `$HOME` is unavailable, `open()` returns `StoreError::HomeDirUnavailable`.
- Fresh roots are initialized by the crate itself on first write; callers do not
  need to pre-create the root directory.
- Path resolution canonicalizes the nearest existing ancestor and rejects
  computed paths that would escape the configured root.
- `read()` and `read_sidecar()` translate only `PathEscape` into `Ok(None)`.
  Other failures, such as invalid URLs or filesystem errors, are returned as
  typed errors.

## Sidecar Schema

```jsonc
{
  "schema_version": 1,
  "url": "https://example.com/page",
  "first_seen": "2026-04-17T00:00:00Z",
  "last_fetched": "2026-04-17T01:00:00Z",
  "fetch_count": 2,
  "current": { /* latest ExtractionResult */ },
  "changelog": [
    {
      "at": "2026-04-17T00:00:00Z",
      "word_count": 120,
      "diff": null
    },
    {
      "at": "2026-04-17T01:00:00Z",
      "word_count": 132,
      "diff": { /* noxa_core::ContentDiff */ }
    }
  ]
}
```

`max_changelog_entries` defaults to `100`. When the cap is exceeded, the store
preserves entry `0` as the initial-fetch sentinel and drains older change
entries after it.

## Operations Log Semantics

`FilesystemOperationsLog` appends one JSON object per line to
`<store_root>/<domain>/.operations.ndjson`.

- each append serializes one `OperationEntry`
- oversized `output` payloads are replaced with:

```json
{
  "output_truncated": true,
  "original_size_bytes": 1234567
}
```

- output truncation threshold = 1 MiB of serialized `output`
- appends use an in-process lock for the full line write

The lock keeps concurrent appends from the same process valid NDJSON. Separate
processes writing the same log path are still best-effort rather than an
audit-grade durability guarantee.

## SSRF Validation Helpers

`url_validation.rs` exports helpers for HTTP/HTTPS URL validation:

- empty or malformed URLs are rejected
- `localhost` and `.localhost` are rejected
- direct IPs and resolved hostnames are rejected when they resolve to private or
  special-use address space, including documentation and benchmarking ranges

The validation helpers fail closed on DNS resolution failure.

## Enumeration and Scaling

`FilesystemContentStore` exposes four enumeration methods added in Wave 1A:

| Method | Scope | Cost |
|---|---|---|
| `list_domains()` | store root (one level) | O(domain count), counts `.md` files per domain via sync recursive walk |
| `list_docs(domain)` | one domain directory | O(docs in domain), parses each `.json` sidecar |
| `list_all_docs()` | entire store | O(total doc count), recursive walk + sidecar parse for every document |
| `list_domain_urls(domain)` | one domain directory | O(docs in domain), recursive walk + sidecar parse |

All four methods perform a filesystem traversal on every call. There is no persistent index. Latency scales linearly with document count and directory depth:

- Small stores (< 1 000 docs): traversal typically completes in tens of milliseconds.
- Large stores (10 000+ docs): traversal and sidecar-parsing overhead reaches several seconds.

`--retrieve` (fuzzy query) calls `list_all_docs()` across the entire store.
`--refresh <domain>` calls `list_domain_urls()` scoped to one domain directory.
Exact-URL `--retrieve` bypasses enumeration entirely and is an O(1) path lookup.

## Deferred / Not Implemented Here

These items are not part of the current crate:

- `.index.ndjson`
- index maintenance APIs
- backend traits such as `ContentBackend` or `OperationsBackend`
- query/read APIs over `.operations.ndjson`
- store-level refresh orchestration

> **See also:** Refresh and retrieval orchestration currently lives in the CLI:
> `crates/noxa-cli/src/app/refresh.rs` (domain re-fetch loop) and
> `crates/noxa-cli/src/app/retrieve.rs` (fuzzy/exact retrieval entrypoint).

If those features are added later, this document should be updated alongside the
implementation.
