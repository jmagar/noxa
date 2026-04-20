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
    noxa-fetch/    # HTTP client via wreq (BoringSSL TLS). Crawler. Sitemap discovery. Batch ops.
                      # + proxy pool rotation (per-request)
                      # + PDF content-type detection
                      # + document parsing (DOCX, XLSX, CSV)
    noxa-llm/      # LLM provider chain (Gemini CLI -> OpenAI -> Ollama -> Anthropic)
                      # + JSON schema extraction (validated + retry), prompt extraction, summarization
    noxa-pdf/      # PDF text extraction via pdf-extract
    noxa-mcp/      # MCP server (Model Context Protocol) for AI agents
    noxa-cli/  # CLI binary (produces `noxa` binary)
    noxa-rag/  # RAG pipeline daemon (TEI embeddings + Qdrant vector store)
                  # + multi-format ingestion (DOCX, XLSX, CSV, PDF, HTML, OPML)
                  # + fs-watcher for automatic re-indexing
    noxa-store/ # Filesystem persistence layer. URL→path mapping, .md + .json sidecar storage.
                  # + versioned sidecar envelope (ExtractionResult + changelog of content diffs)
                  # + FilesystemOperationsLog (domain-level .operations.ndjson append log)
                  # + URL validation (validate_public_http_url, private/reserved IP rejection)
```

Three binaries: `noxa` (CLI), `noxa-mcp` (MCP server), `noxa-rag-daemon` (RAG pipeline).

### Core Modules (`noxa-core`)
- `extractor.rs` — Readability-style scoring: text density, semantic tags, link density penalty
- `noise.rs` — Shared noise filter: tags, ARIA roles, class/ID patterns. Tailwind-safe.
- `data_island.rs` — JSON data island extraction for React SPAs, Next.js, Contentful CMS
- `markdown.rs` — HTML to markdown with URL resolution, asset collection
- `llm.rs` — 9-step LLM optimization pipeline (image strip, emphasis strip, link dedup, stat merge, whitespace collapse)
- `domain.rs` — Domain detection from URL patterns + DOM heuristics
- `metadata.rs` — OG, Twitter Card, standard meta tag extraction
- `types.rs` — Core data structures (ExtractionResult, Metadata, Content)
- `filter.rs` — CSS selector include/exclude filtering (ExtractionOptions)
- `diff.rs` — Content change tracking engine (snapshot diffing)
- `brand.rs` — Brand identity extraction from DOM structure and CSS
- `js_eval.rs` — QuickJS-based inline JavaScript data extraction
- `youtube.rs` — YouTube metadata from `ytInitialPlayerResponse` embedded JSON
- `structured_data.rs` — JSON-LD and structured data extraction
- `extractor/` — Extraction sub-pipeline: scoring, recovery, selectors

### Fetch Modules (`noxa-fetch`)
- `client/` — FetchClient: batch, fetch, pool, and response layers
- `browser.rs` — Browser profiles: Chrome, ChromeMacos, Firefox (wreq/BoringSSL TLS fingerprints)
- `tls.rs` — Browser TLS + HTTP/2 fingerprint profiles (wreq BoringSSL)
- `crawler.rs` — BFS same-origin crawler with configurable depth/concurrency/delay
- `sitemap.rs` — Sitemap discovery and parsing (sitemap.xml, robots.txt)
- `batch.rs` — Multi-URL concurrent extraction
- `proxy.rs` — Proxy pool with per-request rotation
- `document.rs` — Document parsing: DOCX, XLSX, CSV auto-detection and extraction
- `search.rs` — Web search via SearXNG with parallel result scraping
- `linkedin.rs` — LinkedIn post extraction from authenticated HTML
- `reddit.rs` — Reddit JSON API fallback (no-JS post + comment extraction)

### LLM Modules (`noxa-llm`)
- Provider chain: Gemini CLI (primary) -> OpenAI -> Ollama -> Anthropic
- Gemini CLI requires the `gemini` binary on PATH; `GEMINI_MODEL` env var controls model (default: `gemini-2.5-pro`)
- JSON schema extraction with jsonschema validation; retries once with a correction prompt on both parse failures and schema mismatches.
- Prompt-based extraction, summarization
- `clean.rs` — Post-processing: strip thinking tags (`<think>`), normalize LLM output

### PDF Modules (`noxa-pdf`)
- PDF text extraction via pdf-extract crate

### MCP Server (`noxa-mcp`)
- Model Context Protocol server over stdio transport
- 10 tools: scrape, crawl, map, batch, extract, summarize, diff, brand, search, research
- Works with Claude Desktop, Claude Code, and any MCP client
- Uses `rmcp` crate (official Rust MCP SDK)

### RAG Modules (`noxa-rag`)
- `pipeline.rs` — End-to-end ingestion: fetch → chunk → embed → upsert to Qdrant
- `chunker.rs` — Text chunking strategies for embedding
- `embed/` — Embedding providers: TEI (local), OpenAI, VoyageAI
- `store/` — Qdrant REST client (no gRPC)
- `config.rs` — TOML config schema for the daemon
- `mcp_bridge.rs` — MCP tool bridge exposing RAG search to MCP clients
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
| `OPENAI_BASE_URL` | Override OpenAI-compatible base URL (e.g. for local proxies) |
| `ANTHROPIC_API_KEY` | Anthropic API key for LLM provider chain |
| `OLLAMA_HOST` | Ollama server URL (default: `http://localhost:11434`) |
| `OLLAMA_MODEL` | Ollama model name override |
| `OLLAMA_HEALTH_TIMEOUT_MS` | Timeout for Ollama availability check |

## Hard Rules

- **Core has ZERO network dependencies** — takes `&str` HTML, returns structured output. Keep it WASM-compatible.
- **noxa-fetch uses `wreq` (BoringSSL)** for TLS fingerprinting — no `[patch.crates-io]` or RUSTFLAGS needed.
- **noxa-llm uses plain `reqwest`** (rustls-tls). LLM APIs don't need browser fingerprinting.
- **qwen3 thinking tags** (`<think>`) are stripped at both provider and consumer levels.

## Build & Test

```bash
cargo build --release           # All three binaries
cargo test --workspace          # All tests
cargo test -p noxa-core      # Core only
cargo test -p noxa-llm       # LLM only
```

## CLI

```bash
# Basic extraction
noxa https://example.com
noxa https://example.com --format llm

# Content filtering
noxa https://example.com --include "article" --exclude "nav,footer"
noxa https://example.com --only-main-content

# Batch + proxy rotation
noxa url1 url2 url3 --proxy-file proxies.txt
noxa --urls-file urls.txt --concurrency 10

# Sitemap discovery
noxa https://docs.example.com --map

# Crawling (with sitemap seeding)
noxa https://docs.example.com --crawl --depth 2 --max-pages 50 --sitemap

# Change tracking
noxa https://example.com -f json > snap.json
noxa https://example.com --diff-with snap.json

# Brand extraction
noxa https://example.com --brand

# LLM features (Gemini CLI primary; requires `gemini` on PATH)
noxa https://example.com --summarize
noxa https://example.com --extract-prompt "Get all pricing tiers"
noxa https://example.com --extract-json '{"type":"object","properties":{"title":{"type":"string"}}}'

# Force a specific LLM provider
noxa https://example.com --llm-provider gemini --summarize
noxa https://example.com --llm-provider openai --summarize

# PDF (auto-detected via Content-Type)
noxa https://example.com/report.pdf

# Browser impersonation: chrome (default), firefox, random
noxa https://example.com --browser firefox

# Local file / stdin
noxa --file page.html
cat page.html | noxa --stdin

# Interactive first-run setup (config, API keys, MCP registration)
noxa setup

# Web search via SearXNG or Noxa Cloud
noxa --search "rust async runtime comparison" --num-results 20
noxa --search "query" --no-scrape         # snippets only, skip URL scraping

# Local doc store management
noxa --list                               # all cached domains
noxa --list docs.example.com             # docs for a specific domain
noxa --retrieve https://example.com/docs # exact URL or fuzzy query
noxa --grep "authentication"             # search cached docs with rg
noxa --status docs.example.com          # background crawl status

# Watch for changes
noxa https://example.com --watch --watch-interval 60
noxa https://example.com --watch --on-change "notify-send 'Changed!'"
```

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
