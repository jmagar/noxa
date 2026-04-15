# Noxa

Rust workspace: CLI + MCP server for web content extraction into LLM-optimized formats.

## Architecture

```
noxa/
  crates/
    noxa-core/     # Pure extraction engine. WASM-safe. Zero network deps.
                      # + ExtractionOptions (include/exclude CSS selectors)
                      # + diff engine (change tracking)
                      # + brand extraction (DOM/CSS analysis)
    noxa-fetch/    # HTTP client via wreq (TLS fingerprinting). Crawler. Sitemap discovery.
                      # + proxy pool rotation (per-request)
                      # + PDF content-type detection
                      # + document parsing (DOCX, XLSX, CSV)
    noxa-llm/      # LLM provider chain (Gemini CLI -> OpenAI -> Ollama -> Anthropic)
                      # + JSON schema extraction (validated + retry), prompt extraction, summarization
    noxa-pdf/      # PDF text extraction via pdf-extract
    noxa-mcp/      # MCP server (Model Context Protocol) for AI agents
    noxa-store/    # Filesystem persistence: per-URL .md + .json sidecar storage
                      # + operations log (domain-level .ndjson append log)
    noxa-cli/      # CLI binary (produces `noxa` binary)
    noxa-rag/      # RAG pipeline daemon (TEI embeddings + Qdrant vector store)
                      # + multi-format ingestion (DOCX, XLSX, CSV, PDF, HTML, OPML)
                      # + fs-watcher for automatic re-indexing
```

Three binaries: `noxa` (CLI), `noxa-mcp` (MCP server), `noxa-rag-daemon` (RAG pipeline).

### Core Modules (`noxa-core`)
- `extractor.rs` — Readability-style scoring: text density, semantic tags, link density penalty
- `noise.rs` — Shared noise filter: tags, ARIA roles, class/ID patterns. Tailwind-safe.
- `data_island.rs` — JSON data island extraction for React SPAs, Next.js, Contentful CMS
- `markdown.rs` — HTML to markdown with URL resolution, asset collection
- `lib.rs` — 9-step LLM optimization pipeline (image strip, emphasis strip, link dedup, stat merge, whitespace collapse)
- `domain.rs` — Domain detection from URL patterns + DOM heuristics
- `metadata.rs` — OG, Twitter Card, standard meta tag extraction
- `types.rs` — Core data structures (ExtractionResult, Metadata, Content)
- `js_eval.rs` — JavaScript expression evaluation for data island extraction
- `structured_data.rs` — Structured data (JSON-LD, microdata) extraction
- `youtube.rs` — YouTube-specific content extraction
- `diff.rs` — Content change tracking engine (snapshot diffing)
- `brand.rs` — Brand identity extraction from DOM structure and CSS

### Fetch Modules (`noxa-fetch`)
- `client.rs` — FetchClient with wreq TLS impersonation
- `browser.rs` — Browser profile user-agent strings
- `tls.rs` — TLS fingerprint profiles: Chrome (142/136/133/131), Firefox (144/135/133/128)
- `crawler.rs` — BFS same-origin crawler with configurable depth/concurrency/delay
- `sitemap.rs` — Sitemap discovery and parsing (sitemap.xml, robots.txt)
- `proxy.rs` — Proxy pool with per-request rotation
- `document.rs` — Document parsing: DOCX, XLSX, CSV auto-detection and extraction
- `search.rs` — Web search via SearXNG (self-hosted) with parallel result scraping
- `linkedin.rs` — LinkedIn-specific content extraction
- `reddit.rs` — Reddit-specific content extraction

### LLM Modules (`noxa-llm`)
- `chain.rs` — Provider chain: Gemini CLI (primary) -> OpenAI -> Ollama -> Anthropic
- `provider.rs` — `LlmProvider` trait every backend implements
- `extract.rs` — JSON schema extraction with jsonschema validation + correction-prompt retry
- `summarize.rs` — Content summarization via provider chain
- `clean.rs` — Response cleaning (thinking tag stripping, etc.)
- `testing.rs` — Test utilities for LLM integration tests
- Gemini CLI requires `gemini` binary on PATH; `GEMINI_MODEL` env var controls model (default: `gemini-2.5-pro`)

### PDF Modules (`noxa-pdf`)
- PDF text extraction via pdf-extract crate

### MCP Server (`noxa-mcp`)
- Model Context Protocol server over stdio transport
- 10 tools: scrape, crawl, map, batch, extract, summarize, diff, brand, search, research
- Works with Claude Desktop, Claude Code, and any MCP client
- Uses `rmcp` crate (official Rust MCP SDK)

### Store Modules (`noxa-store`)
- `content_store.rs` — Per-URL `.md` + `.json` sidecar filesystem storage
- `operations_log.rs` — Domain-level `.operations.ndjson` append log
- `paths.rs` — URL-to-path mapping, store root discovery
- `types.rs` — `Op`, `OperationEntry`, `StoreResult`

### RAG Modules (`noxa-rag`)
- `pipeline.rs` — End-to-end ingestion: fetch → chunk → embed → upsert to Qdrant
- `chunker.rs` — Text chunking strategies for embedding
- `embed/` — Embedding providers: TEI (local) with factory pattern
- `store/` — Qdrant REST client (no gRPC)
- `config.rs` — TOML config schema for the daemon
- Produces `noxa-rag-daemon` binary for background indexing

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `NOXA_API_KEY` | Noxa Cloud API key (required for `--research`, `--cloud`) |
| `SEARXNG_URL` | SearXNG instance URL (enables `--search` without cloud) |
| `NOXA_CONFIG` | Path to `config.json` override |
| `NOXA_NO_STORE` | Disable automatic content store persistence |
| `GEMINI_MODEL` | Gemini model override (default: `gemini-2.5-pro`) |
| `OPENAI_API_KEY` | OpenAI API key for LLM provider chain |

Use `config/config.example.json` as the template for `config.json`, `config/config.schema.json` for the JSON contract, and `config/.env.example` as the template for `.env`.

## Hard Rules

- **Core has ZERO network dependencies** — takes `&str` HTML, returns structured output. Keep it WASM-compatible.
- **noxa-fetch and noxa-mcp use wreq** (TLS fingerprinting). noxa-rag uses plain reqwest (no fingerprinting needed for TEI/Qdrant).
- **RUSTFLAGS are set in `.cargo/config.toml`** — no need to pass manually.
- **noxa-llm uses plain reqwest** (NOT wreq). LLM APIs don't need TLS fingerprinting.
- **qwen3 thinking tags** (`<think>`) are stripped at both provider and consumer levels.

## Build & Test

```bash
cargo build --release           # Both binaries
cargo test --workspace          # All tests
cargo test -p noxa-core      # Core only
cargo test -p noxa-llm       # LLM only
```

## CLI

```bash
noxa <url>                          # Basic extraction
noxa <url> --format llm             # LLM-optimized output
noxa <url> --include "article" --exclude "nav,footer"  # CSS selector filtering
noxa <url> --crawl --depth 2 --max-pages 50            # BFS crawl
noxa <url> --map                    # Sitemap discovery
noxa <url> --summarize              # LLM summarization
noxa <url> --extract-json '<schema>'  # Structured JSON extraction
noxa <url> --brand                  # Brand identity extraction
noxa <url> --diff-with snap.json    # Change tracking
noxa <url> --browser firefox        # TLS fingerprint: chrome (default), firefox, random
noxa --search "query"               # SearXNG web search
noxa --file page.html               # Local file
noxa setup                          # Interactive first-run config
noxa --list                         # Cached content store
noxa --grep "pattern"               # Search cached docs
```

Full flag reference: `noxa --help` or see clap derives in `crates/noxa-cli/src/`.

## Key Thresholds

- Scoring minimum: 50 chars text length
- Semantic bonus: +50 for `<article>`/`<main>`, +25 for content class/ID
- Link density: >50% = 0.1x score, >30% = 0.5x
- Data island fallback triggers when DOM word count < 30
- Eyebrow text max: 80 chars

## MCP Setup

Add to Claude Desktop config:
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`
```json
{
  "mcpServers": {
    "noxa": {
      "command": "noxa",
      "args": ["mcp"]
    }
  }
}
```

## Skills

- `/scrape <url>` — extract content from a URL
- `/benchmark [url]` — run extraction performance benchmarks
- `/research <url>` — deep web research via crawl + extraction
- `/crawl <url>` — crawl a website
- `/commit` — conventional commit with change analysis

## Git

- Remote: `git@github.com:jmagar/noxa.git`
- Use `/commit` skill for commits
