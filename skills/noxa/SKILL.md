---
name: noxa
compatibility: Requires `noxa` binary on PATH. Install via `brew install noxa`, `cargo install --git https://github.com/jmagar/noxa.git noxa-cli --bin noxa`, or download from GitHub Releases.
description: >-
  This skill should be used when the user wants to scrape, extract, or fetch content from
  a URL using the noxa CLI, crawl a website, get the text of a web page, monitor or watch
  a page for changes, extract brand identity (colors, fonts, logos) from a site,
  batch-process URLs, summarize a web page with an LLM, extract structured data from a
  page, run deep research on a topic, or save crawl output to files.
  Trigger on phrases like: "scrape", "extract from", "get content from", "crawl", "fetch
  this page", "what does this site say", "get the text of", "monitor changes", "watch this
  URL", "brand colors of", "sitemap of", "summarize this URL", "deep research". Use this
  skill before running noxa — it covers the correct flag combinations for every workflow
  and prevents common mistakes.
---

# Noxa — Web Content Extraction for AI

noxa extracts clean, LLM-optimized content from any URL using Chrome-level TLS fingerprinting.
No browser required. Output is 67% fewer tokens than raw HTML.

Binary: `noxa` (CLI) — assumed to be on PATH. Verify with `which noxa`.

> **Complete flag reference:** See `references/flags.md` for every flag, its default, env var binding, and the full config.json schema.

---

## Choosing the right mode

To choose the right mode, identify what the user wants from this URL:

| Goal | Mode |
|------|------|
| Read a page | Basic extraction |
| Read docs / whole site | Crawl |
| Find all URLs on a site | Map |
| Multiple URLs at once | Batch |
| Extract structured fields | LLM extraction |
| Summarize a page | Summarize |
| Deep research on a topic | Research (cloud) |
| Track changes once | Diff |
| Continuously watch for changes | Watch |
| Get brand colors/fonts/logos | Brand |
| Debug a 403 or bad output | Raw HTML |

---

## Basic extraction

```bash
# Default: clean markdown, great for reading
noxa https://example.com

# Format options
noxa https://example.com -f llm       # Token-optimized (best for feeding to Claude)
noxa https://example.com -f json      # Full structured JSON with metadata
noxa https://example.com -f text      # Plain text, no formatting
noxa https://example.com -f markdown  # Markdown (same as default)
noxa https://example.com -f html      # Raw extracted HTML

# Skip nav/sidebar/footer noise
noxa https://example.com --only-main-content

# Include/exclude specific elements via CSS selectors
noxa https://example.com --include "article,.content"
noxa https://example.com --exclude "nav,footer,.sidebar,.ads"

# Include metadata as YAML frontmatter
noxa https://example.com --metadata

# Request timeout (default: 30s)
noxa --timeout 60 https://slow-site.com
```

Use `-f llm` when passing content to Claude — it cuts token usage by ~67%.

---

## Crawling a site

```bash
# Crawl with defaults (depth 1, up to 20 pages)
noxa --crawl https://docs.example.com

# Control scope
noxa --crawl --depth 3 --max-pages 100 https://docs.example.com

# Seed from sitemap first (finds more pages)
noxa --crawl --sitemap --depth 2 https://docs.example.com

# Filter by path prefix (strict prefix match)
noxa --crawl --path-prefix /docs https://docs.example.com

# Filter by glob patterns (more flexible than --path-prefix)
noxa --crawl --include-paths "/api/*,/guide/*" https://docs.example.com
noxa --crawl --exclude-paths "/changelog/*,/blog/*" https://docs.example.com

# Control concurrency and delay (ms between requests)
noxa --crawl --concurrency 5 --delay 500 https://example.com

# Save/resume crawl state (Ctrl+C saves progress; rerunning resumes)
noxa --crawl --crawl-state state.json --max-pages 500 https://docs.example.com

# Save each page to a separate file instead of stdout
noxa --crawl --output-dir ./output https://docs.example.com
```

Good for: building search indexes, ingesting documentation, research.

---

## Sitemap discovery

```bash
# List all URLs from the site's sitemaps
noxa --map https://example.com

# JSON with last_modified and priority
noxa --map https://example.com -f json
```

Use `--map` when you want to know what's on a site before crawling.

---

## Batch processing

```bash
# Multiple URLs in one command
noxa https://site-a.com https://site-b.com https://site-c.com

# From a file (one URL per line, # comments OK)
# Also supports CSV format: url,custom-filename
noxa --urls-file urls.txt

# Save each result to a separate file
noxa --urls-file urls.txt --output-dir ./pages

# With concurrency and proxy rotation
noxa --urls-file urls.txt --concurrency 10 -f llm --proxy-file proxies.txt
```

---

## LLM-powered extraction

These require an LLM provider. noxa tries Gemini CLI first, then Ollama, then OpenAI, then Anthropic.

Configure whichever provider you have available:
```bash
# Gemini CLI (primary — requires `gemini` binary on PATH)
# Model controlled by GEMINI_MODEL env var (default: gemini-2.5-pro)

# Ollama (local, no key needed — default endpoint http://localhost:11434)
export OLLAMA_HOST=http://localhost:11434   # only needed if non-default

# OpenAI
export OPENAI_API_KEY=sk-...

# Anthropic
export ANTHROPIC_API_KEY=sk-ant-...

# Override provider/model/URL via env vars
export NOXA_LLM_PROVIDER=openai        # gemini | ollama | openai | anthropic
export NOXA_LLM_MODEL=gpt-4o
export NOXA_LLM_BASE_URL=http://localhost:11434  # for Ollama or OpenAI-compatible endpoints
```

```bash
# Summarize (default: 3 sentences)
noxa --summarize https://example.com
noxa --summarize 5 https://example.com   # pass sentence count as positional arg after the flag

# Extract with natural language
noxa --extract-prompt "Get all pricing tiers with name, price, and features" https://stripe.com/pricing

# Extract as structured JSON
noxa --extract-json '{"type":"object","properties":{"title":{"type":"string"},"price":{"type":"number"}}}' https://example.com/product

# Schema from file
noxa --extract-json @schema.json https://example.com/product

# Force a specific provider via flag
noxa --llm-provider ollama --summarize https://example.com
noxa --llm-provider openai --llm-model gpt-4o --extract-prompt "..." https://example.com
noxa --llm-provider anthropic --summarize https://example.com

# Override LLM base URL (for self-hosted OpenAI-compatible endpoints)
noxa --llm-base-url http://my-server:8080 --llm-provider openai --summarize https://example.com
```

---

## Change detection (diff)

```bash
# Step 1: snapshot
noxa https://example.com -f json > snapshot.json

# Step 2: compare later
noxa --diff-with snapshot.json https://example.com
# Output: Status: Same | Changed, word delta, unified diff
```

Good for: one-off comparisons, price monitoring, detecting updates.

---

## Watch mode (continuous monitoring)

Watch polls a URL on a schedule and reports diffs whenever the content changes.

```bash
# Watch with default interval (300s / 5 minutes)
noxa --watch https://example.com

# Custom interval
noxa --watch --watch-interval 60 https://example.com   # check every 60s

# Run a command when a change is detected (receives diff JSON on stdin)
noxa --watch --on-change "python notify.py" https://example.com

# Post to a webhook on change (also works with --crawl and batch)
noxa --watch --webhook https://hooks.slack.com/... https://example.com
export NOXA_WEBHOOK_URL=https://hooks.discord.com/...   # or via env var
```

Webhook auto-detects Discord and Slack URLs and wraps the payload accordingly.

---

## Deep research (cloud)

Runs multi-source research on a topic via the noxa.io cloud API. Saves a full report (findings + sources) to a JSON file. Requires an API key.

```bash
export NOXA_API_KEY=wc_your_key

# Standard research
noxa --research "best practices for Rust error handling" --api-key $NOXA_API_KEY

# Deep mode (longer, more thorough report)
noxa --research "Rust async runtimes compared" --deep --api-key $NOXA_API_KEY
```

---

## Brand identity extraction

```bash
noxa --brand https://stripe.com
# Returns: name, colors (hex + usage), fonts, logos, favicon
```

Output is JSON. Useful for design audits, competitive analysis, or building themed UIs.

---

## PDF extraction

```bash
# Auto-detected via Content-Type header
noxa https://example.com/report.pdf

# Control behavior on scanned PDFs (no extractable text)
noxa --pdf-mode auto https://example.com/report.pdf   # error on empty (default)
noxa --pdf-mode fast https://example.com/report.pdf   # return whatever text exists
```

---

## Auth, headers, cookies, proxies

```bash
# Custom headers
noxa -H "Authorization: Bearer token123" https://api.example.com
noxa -H "Accept-Language: fr-FR" -H "X-Custom: value" https://example.com

# Cookie string (shorthand)
noxa --cookie "session=abc123; theme=dark" https://example.com

# Cookie file (Chrome extension JSON export format)
noxa --cookie-file cookies.json https://example.com

# Browser impersonation (default: Chrome)
noxa --browser firefox https://example.com
noxa --browser random https://example.com   # random per request, good for batch

# Single proxy
noxa --proxy http://user:pass@proxy.example.com:8080 https://example.com
noxa --proxy socks5://proxy.example.com:1080 https://example.com

# Proxy pool rotation
noxa --proxy-file proxies.txt https://example.com   # host:port:user:pass per line
```

---

## Bot-protected sites / JS rendering

noxa.io is the optional hosted cloud rendering service — it handles Cloudflare, DataDome, and JS-rendered SPAs that local TLS fingerprinting can't bypass. Get an API key at [noxa.io](https://noxa.io).

```bash
# Pass key via env var or --api-key flag
export NOXA_API_KEY=wc_your_key
# or: noxa --api-key wc_your_key https://example.com

# Auto: tries local TLS fingerprinting first, falls back to cloud on bot detection
noxa https://cloudflare-protected-site.com

# Force cloud (for SPA / JS-heavy pages)
noxa --cloud https://spa-site.com
```

---

## Output to files

```bash
# Save crawl output — one file per page, filenames derived from URL paths
noxa --crawl --output-dir ./docs https://docs.example.com

# Save batch output
noxa --urls-file urls.txt --output-dir ./pages -f llm

# Single URL to file
noxa --output-dir ./out https://example.com
```

---

## Config file

noxa loads `./config.json` by default. Override with `--config` or `NOXA_CONFIG`:

```bash
noxa --config ~/.noxa/config.json https://example.com
export NOXA_CONFIG=/etc/noxa/config.json
```

Config uses snake_case keys that match `config.example.json` and the Rust config struct. Useful for setting defaults like `llm_provider`, `browser`, `concurrency`, `timeout`.

---

## Local files and stdin

```bash
# Local HTML file
noxa --file page.html

# Pipe HTML
curl -s https://example.com | noxa --stdin
```

---

## Debugging

```bash
# Get the raw fetched HTML to see what noxa received
noxa --raw-html https://example.com

# Verbose extraction pipeline logging
noxa -v https://example.com
```

If a site returns 403, try `--browser firefox` or `--browser random`. If still blocked, use `--cloud` with an API key.

---

## Environment variables reference

| Variable | Flag equivalent | Description |
|----------|----------------|-------------|
| `NOXA_API_KEY` | `--api-key` | Cloud API key |
| `NOXA_PROXY` | `--proxy` | Single proxy URL |
| `NOXA_PROXY_FILE` | `--proxy-file` | Proxy pool file path |
| `NOXA_WEBHOOK_URL` | `--webhook` | Webhook URL for notifications |
| `NOXA_LLM_PROVIDER` | `--llm-provider` | LLM provider (gemini/ollama/openai/anthropic) |
| `NOXA_LLM_MODEL` | `--llm-model` | LLM model name override |
| `NOXA_LLM_BASE_URL` | `--llm-base-url` | LLM base URL (Ollama/OpenAI-compatible) |
| `NOXA_CONFIG` | `--config` | Path to config.json |
| `OPENAI_API_KEY` | — | OpenAI API key |
| `ANTHROPIC_API_KEY` | — | Anthropic API key |
| `OLLAMA_HOST` | — | Ollama endpoint (default: http://localhost:11434) |

---

## Common recipes

```bash
# Read docs site as a single LLM-optimized text file
noxa --crawl --sitemap --depth 3 --max-pages 500 -f llm https://docs.example.com > docs.txt

# Save full crawl to individual files
noxa --crawl --sitemap --depth 2 --output-dir ./docs -f llm https://docs.example.com

# Extract all external links from a page
noxa https://example.com -f json | jq -r '.content.links[] | select(.href | startswith("http")) | .href'

# Monitor competitor pricing — snapshot then diff
noxa https://competitor.com/pricing -f json > pricing-$(date +%Y%m%d).json
noxa https://competitor.com/pricing --diff-with pricing-yesterday.json

# Watch a page and notify on Slack when it changes
noxa --watch --watch-interval 3600 --webhook https://hooks.slack.com/... https://example.com

# Resumable large crawl
noxa --crawl --crawl-state state.json --depth 4 --max-pages 2000 https://docs.example.com

# Word count of a page
noxa https://example.com -f text | wc -w

# Extract article title with jq
noxa https://example.com -f json | jq '.metadata.title'
```
