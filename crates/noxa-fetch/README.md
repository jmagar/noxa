# noxa-fetch

`noxa-fetch` is the transport layer for `noxa`. It wraps `wreq` with browser-like
TLS/HTTP2 impersonation and adds crate-local extraction shortcuts for PDFs,
office documents, Reddit JSON, and LinkedIn HTML payloads.

## Operational Semantics

- Redirects are controlled by `FetchConfig.follow_redirects` and
  `FetchConfig.max_redirects`.
- `proxy_pool` is host-sticky, not per-request random. The same host hashes to
  the same prebuilt client so HTTP/2 connections can be reused through one
  proxy.
- `proxy` is a single fixed proxy used when `proxy_pool` is empty.
- Proxy file entries support `host:port` and `host:port:user:pass`. Credentials
  are percent-encoded before building the proxy URL.

## Response Limits

`noxa-fetch` buffers responses before handing them to extractors, so it applies
 hard limits:

- HTML: 2 MiB
- JSON: 2 MiB
- Office documents: 16 MiB
- PDFs: 32 MiB
- DOCX `word/document.xml`: 8 MiB decompressed
- DOCX archive entries: 256 maximum

These limits are enforced before fully buffering the response body when the
content length is known, and while streaming chunks when it is not.

## Site-specific Extraction

- Reddit post URLs first try the `.json` API so comment trees are preserved.
- LinkedIn post URLs read embedded JSON from `<code>` blocks in authenticated
  HTML.
- Both custom extractors normalize `plain_text` from their generated markdown so
  CLI and MCP text output stays non-empty.

## Maintenance Notes

`wreq` is pinned to a release candidate because the impersonation and transport
features used here are not yet available in a stable line with the same API
surface. When upgrading:

1. Re-run `cargo test -p noxa-fetch --lib`.
2. Re-run `cargo clippy -p noxa-fetch --all-targets -- -D warnings`.
3. Re-check redirect behavior, proxy auth, and response-size guards.
4. Refresh hard-coded browser fingerprints in `src/tls.rs` against current
   Chrome/Firefox/Safari/Edge wire behavior.
