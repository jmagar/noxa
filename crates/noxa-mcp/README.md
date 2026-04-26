# noxa-mcp

`noxa-mcp` exposes noxa's extraction and research tools over MCP stdio transport.

## Transport

`noxa-mcp` speaks MCP on `stdout`.

- Do not print application logs to `stdout`.
- Initialize logging on `stderr` only.
- When embedding via `noxa_mcp::run()`, leave `stdout` untouched after startup.

The binary entrypoint is:

```bash
cargo run -p noxa-mcp
```

That starts a stdio server intended for MCP clients such as Claude Code, Claude Desktop, and other JSON-RPC/MCP hosts.

## Environment

The server builds its runtime configuration from a typed config layer instead of ad hoc reads in tool handlers.

- `NOXA_API_KEY`: enables cloud fallback for bot-protected pages plus `research` and cloud-backed `search`.
- `SEARXNG_URL`: optional self-hosted SearXNG base URL for local `search`.
- `NOXA_PROXY`: optional single upstream proxy URL for fetch traffic.
- `NOXA_PROXY_FILE`: optional proxy pool file. If unset, `proxies.txt` in the current working directory is used when present.

## Filesystem Writes

The crate writes under the current user's home directory:

- `~/.noxa/content/`: persisted extraction snapshots used by fetch-backed tools and `diff`.
- `~/.noxa/research/`: research job artifacts (`.json` plus markdown report when present).

Startup now creates those directories up front and returns a typed error if initialization fails.

## Tool Notes

- `scrape`, `crawl`, and `batch` use validated format enums instead of free-form strings.
- `scrape` accepts an optional `extractor` string for explicit vertical extraction; use the `extractors` tool to list the supported extractors.
- `extract` requires exactly one of `schema` or `prompt`.
- `search` returns snippets plus fetch errors for validated result URLs; it does not write to `stdout` outside MCP.
- `diff` can bootstrap a missing local baseline when a local fetch succeeds.

## Testing

Unit coverage in this crate now covers:

- URL validation helpers
- parameter enum/schema validation
- typed runtime config loading

Integration scaffolding lives in [`tests/support/mod.rs`](./tests/support/mod.rs) and [`tests/startup_harness.rs`](./tests/startup_harness.rs). Wave 2 can extend that harness with full MCP request/response coverage without reworking process setup.
