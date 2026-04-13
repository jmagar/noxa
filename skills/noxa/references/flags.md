# Noxa CLI — Complete Flag Reference

All flags for the `noxa` binary. Sourced directly from `crates/noxa-cli/src/main.rs`.

Priority order when the same setting appears in multiple places:
**CLI flag > config.json > environment variable > hard default**

---

## Table of Contents

- [Input](#input)
- [Output](#output)
- [Content Filtering](#content-filtering)
- [Request / Network](#request--network)
- [Auth & Identity](#auth--identity)
- [Crawl](#crawl)
- [LLM](#llm)
- [Change Detection](#change-detection)
- [Watch Mode](#watch-mode)
- [Search](#search)
- [Content Store](#content-store)
- [Brand Extraction](#brand-extraction)
- [PDF](#pdf)
- [Cloud API](#cloud-api)
- [Config File](#config-file)
- [Environment Variables](#environment-variables)
- [config.json Reference](#configjson-reference)

---

## Input

| Flag | Type | Description |
|------|------|-------------|
| `[URLS]...` | positional | One or more URLs to fetch. Bare domains are auto-prefixed with `https://`. |
| `--urls-file <FILE>` | string | File with URLs, one per line. `#` comments supported. CSV format `url,filename` sets a custom output filename. |
| `--file <FILE>` | string | Extract from a local HTML file instead of fetching. |
| `--stdin` | bool | Read HTML from stdin. |

---

## Output

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--format <FMT>` | `-f` | `markdown` | Output format: `markdown`, `json`, `text`, `llm`, `html`. Use `llm` when feeding to Claude — 67% fewer tokens than raw HTML. |
| `--metadata` | | false | Include YAML frontmatter with title, source URL, word count. Always included in JSON format. |
| `--raw-html` | | false | Output the raw fetched HTML with no extraction. Useful for debugging. CLI-only — not settable in config.json. |
| `--output-dir <DIR>` | | — | Save each page to a separate file instead of stdout. Works with `--crawl`, batch, and single-URL mode. Filenames derived from URL paths (e.g. `/docs/api` → `docs/api.md`). |
| `--verbose` / `-v` | `-v` | false | Enable verbose extraction pipeline logging to stderr. |

---

## Content Filtering

| Flag | Description |
|------|-------------|
| `--only-main-content` | Auto-detect and extract only the main content element (`<article>`, `<main>`). Strips nav, sidebar, footer. |
| `--include <SELECTORS>` | Comma-separated CSS selectors to include (e.g. `"article,.content"`). In config.json: `include_selectors` array. |
| `--exclude <SELECTORS>` | Comma-separated CSS selectors to exclude (e.g. `"nav,footer,.ads"`). In config.json: `exclude_selectors` array. |

---

## Request / Network

| Flag | Short | Env | Default | Description |
|------|-------|-----|---------|-------------|
| `--browser <BROWSER>` | `-b` | — | `chrome` | TLS fingerprint to impersonate: `chrome`, `firefox`, `random`. `random` picks a different profile per request. |
| `--timeout <SECS>` | `-t` | — | `30` | Request timeout in seconds. |
| `--proxy <URL>` | `-p` | `NOXA_PROXY` | — | Single proxy URL. Formats: `http://user:pass@host:port`, `socks5://host:port`. Takes priority over `--proxy-file` if both are set. |
| `--proxy-file <FILE>` | | `NOXA_PROXY_FILE` | — | Proxy pool file — one proxy per line as `host:port:user:pass`. Rotates per request. |
| `--concurrency <N>` | | — | `5` | Max concurrent requests (also used for crawl). |
| `--delay <MS>` | | — | `100` | Delay between requests in milliseconds. |

---

## Auth & Identity

| Flag | Description |
|------|-------------|
| `-H / --header <VALUE>` | Custom request header, repeatable. Format: `"Name: value"`. |
| `--cookie <STRING>` | Cookie string, shorthand for `-H "Cookie: ..."`. |
| `--cookie-file <FILE>` | JSON cookie file in Chrome extension export format: `[{name, value, domain, path, secure, ...}]`. |

---

## Crawl

All crawl flags require `--crawl` to be active, except `--map` and `--sitemap` which are standalone.

| Flag | Default | Description |
|------|---------|-------------|
| `--crawl` | false | Enable recursive BFS crawl of same-origin links. |
| `--depth <N>` | `1` | Max crawl depth from the start URL. |
| `--max-pages <N>` | `20` | Maximum number of pages to crawl. |
| `--concurrency <N>` | `5` | Max concurrent fetch workers during crawl. |
| `--delay <MS>` | `100` | Delay between requests in milliseconds. |
| `--path-prefix <PREFIX>` | — | Only crawl URLs whose path starts with this prefix (strict string match). |
| `--include-paths <GLOBS>` | — | Comma-separated glob patterns for paths to include (e.g. `"/api/*,/guides/**"`). More flexible than `--path-prefix`. In config.json: `include_paths` array. |
| `--exclude-paths <GLOBS>` | — | Comma-separated glob patterns for paths to exclude (e.g. `"/changelog/*,/blog/*"`). In config.json: `exclude_paths` array. |
| `--sitemap` | false | Seed the crawl frontier from sitemap discovery (checks `robots.txt` and `/sitemap.xml`). Also usable standalone to enable sitemaps without crawling. In config.json: `use_sitemap`. |
| `--map` | false | Discover and print all URLs from the site's sitemaps without fetching content. One URL per line; JSON array with `-f json`. |
| `--crawl-state <FILE>` | — | Path to a JSON file for saving/resuming crawl state. On Ctrl+C: saves progress. On next run: resumes from where it left off. |

---

## LLM

Requires a configured LLM provider. noxa tries Gemini CLI → Ollama → OpenAI → Anthropic in order.

| Flag | Env | Description |
|------|-----|-------------|
| `--summarize [N]` | — | Summarize extracted content. Optional sentence count (default: 3). Pass as positional arg: `--summarize 5`. |
| `--extract-prompt <PROMPT>` | — | Extract content using a natural language prompt. |
| `--extract-json <SCHEMA>` | — | Extract structured JSON conforming to a JSON Schema string. Pass `@file.json` to load schema from a file. |
| `--llm-provider <NAME>` | `NOXA_LLM_PROVIDER` | Force a specific provider: `gemini`, `ollama`, `openai`, `anthropic`. |
| `--llm-model <NAME>` | `NOXA_LLM_MODEL` | Override the model name (e.g. `gpt-4o`, `gemini-2.5-pro`). |
| `--llm-base-url <URL>` | `NOXA_LLM_BASE_URL` | Override the LLM base URL. Use for self-hosted Ollama or OpenAI-compatible endpoints. |

Provider setup:
- **Gemini CLI**: requires `gemini` binary on PATH. Model via `GEMINI_MODEL` (default: `gemini-2.5-pro`).
- **Ollama**: set `OLLAMA_HOST` if not on `http://localhost:11434`.
- **OpenAI**: set `OPENAI_API_KEY`.
- **Anthropic**: set `ANTHROPIC_API_KEY`.

---

## Change Detection

| Flag | Description |
|------|-------------|
| `--diff-with <FILE>` | Compare current extraction against a previously saved JSON snapshot. Reports status (Same/Changed), word delta, and a unified diff. Take a snapshot with `noxa -f json > snapshot.json`. |

---

## Watch Mode

| Flag | Default | Description |
|------|---------|-------------|
| `--watch` | false | Continuously poll a URL for changes and report diffs. |
| `--watch-interval <SECS>` | `300` | Poll interval in seconds. |
| `--on-change <CMD>` | — | Shell command to run when a change is detected. Receives the diff JSON on stdin. CLI-only — intentionally excluded from config.json to prevent shell injection via config file writes. |
| `--webhook <URL>` | `NOXA_WEBHOOK_URL` | POST a JSON payload when changes are detected (watch), a crawl completes, or a batch finishes. Auto-detects Discord and Slack URLs and wraps the payload accordingly. |

---

## Search

Search the web and scrape result pages in one command. Requires `SEARXNG_URL` for local search or `NOXA_API_KEY` for cloud search.

| Flag | Default | Env | Description |
|------|---------|-----|-------------|
| `--search <QUERY>` | — | — | Run a web search and scrape the top result pages. Uses `SEARXNG_URL` (self-hosted SearXNG) if set; otherwise requires `NOXA_API_KEY`. |
| `--num-results <N>` | `10` | — | Number of search results to return (clamped to 1–50). |
| `--no-scrape` | false | — | Print snippets only; skip scraping result page URLs. |
| `--num-scrape-concurrency <N>` | `3` | — | Concurrent fetch workers for scraping result URLs. |

Scraped result pages are auto-persisted to `~/.noxa/content/` via ContentStore. Use `--no-store` to suppress.

---

## Content Store

Every successful `fetch_and_extract` call automatically persists the result to `~/.noxa/content/{domain}/{path}.{md,json}`. Covers all extraction paths: HTML, PDF, DOCX/XLSX/CSV, Reddit JSON, LinkedIn, batch, crawl, and MCP tools.

| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--no-store` | `NOXA_NO_STORE` | false | Disable automatic content persistence for this run. Set `NOXA_NO_STORE` to any non-empty value to disable globally. |

`--file` and `--stdin` paths call `noxa_core::extract()` directly and do not write to the store.

---

## Brand Extraction

| Flag | Description |
|------|-------------|
| `--brand` | Extract brand identity: colors (hex + usage), fonts, logos, favicon. Output is JSON. |

---

## PDF

| Flag | Default | Description |
|------|---------|-------------|
| `--pdf-mode <MODE>` | `auto` | How to handle PDFs: `auto` errors on empty text (catches scanned/image PDFs), `fast` returns whatever text is found. PDFs are auto-detected via `Content-Type` header. |

---

## Cloud API

noxa.io is the optional hosted rendering service. Handles Cloudflare, DataDome, WAF, and JS-rendered SPAs. Get a key at [noxa.io](https://noxa.io).

| Flag | Env | Description |
|------|-----|-------------|
| `--api-key <KEY>` | `NOXA_API_KEY` | Cloud API key. When set, enables automatic fallback to cloud on bot detection. |
| `--cloud` | — | Force all requests through the cloud API, skipping local extraction entirely. |
| `--research <TOPIC>` | — | Run deep multi-source research on a topic via the cloud API. Saves full result (report + sources + findings) to a JSON file. Requires `--api-key`. |
| `--deep` | — | Enable deep research mode (longer, more thorough report). Used with `--research`. |

---

## Config File

noxa loads `./config.json` by default. Override with `--config <PATH>` or `NOXA_CONFIG`.

```bash
noxa --config ~/.noxa/config.json https://example.com
export NOXA_CONFIG=/etc/noxa/config.json
```

**Important caveats:**
- CLI flags always win over config.json values.
- `on_change` is intentionally excluded from config.json (security: prevents shell injection via config writes).
- Secrets and URLs (`api_key`, `proxy`, `webhook`, `llm_base_url`) belong in `.env`, not config.json.
- Bool flags set to `true` in config.json (`only_main_content`, `metadata`, `verbose`, `use_sitemap`) **cannot** be overridden to `false` from the CLI for a single run (clap has no `--no-flag` variant). Use `NOXA_CONFIG=/dev/null` to bypass the config entirely.

---

## Environment Variables

| Variable | Flag equivalent | Description |
|----------|----------------|-------------|
| `NOXA_API_KEY` | `--api-key` | Cloud API key |
| `SEARXNG_URL` | — | Self-hosted SearXNG base URL; enables local search without `NOXA_API_KEY` |
| `NOXA_NO_STORE` | `--no-store` | Set to any non-empty value to disable ContentStore auto-persistence |
| `NOXA_PROXY` | `--proxy` | Single proxy URL |
| `NOXA_PROXY_FILE` | `--proxy-file` | Proxy pool file path |
| `NOXA_WEBHOOK_URL` | `--webhook` | Webhook URL for notifications |
| `NOXA_LLM_PROVIDER` | `--llm-provider` | LLM provider (`gemini`/`openai`/`ollama`/`anthropic`) |
| `NOXA_LLM_MODEL` | `--llm-model` | LLM model name override |
| `NOXA_LLM_BASE_URL` | `--llm-base-url` | LLM base URL for Ollama or OpenAI-compatible endpoints |
| `NOXA_CONFIG` | `--config` | Path to config.json |
| `OPENAI_API_KEY` | — | OpenAI API key |
| `ANTHROPIC_API_KEY` | — | Anthropic API key |
| `OLLAMA_HOST` | — | Ollama endpoint (default: `http://localhost:11434`) |
| `GEMINI_MODEL` | — | Gemini model override (default: `gemini-2.5-pro`) |

---

## config.json Reference

All fields are optional. Unknown fields are silently ignored.

```json
{
  "format": "llm",
  "metadata": true,
  "verbose": false,

  "browser": "firefox",
  "timeout": 60,
  "pdf_mode": "fast",
  "only_main_content": true,

  "include_selectors": ["article", ".content"],
  "exclude_selectors": ["nav", "footer"],

  "depth": 3,
  "max_pages": 100,
  "concurrency": 10,
  "delay": 200,
  "path_prefix": "/docs/",
  "include_paths": ["/docs/*", "/api/*"],
  "exclude_paths": ["/changelog/*", "/blog/*"],
  "use_sitemap": true,

  "llm_provider": "gemini",
  "llm_model": "gemini-2.5-pro"
}
```

**Not configurable via config.json** (CLI-only or secrets):
- `on_change` — shell injection risk
- `api_key`, `proxy`, `webhook`, `llm_base_url` — secrets/URLs belong in `.env`
- `raw_html` — per-run mode, not a persistent default
