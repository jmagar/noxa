# Config and Environment

This document explains how `noxa` loads configuration, how it merges `config.json` with environment variables and CLI flags, and which settings belong in each place.

## Quick Summary

- `config.json` is for non-secret defaults.
- `.env` is for secrets and URLs.
- CLI flags always win over config and environment variables.
- Unknown keys in `config.json` are ignored.
- `config.json` uses `snake_case` keys.

## Load Order

`noxa` resolves settings in this order:

1. CLI flags
2. `config.json`
3. Environment variables
4. Built-in defaults

That means you can set a default in `config.json`, override it for a single run with a CLI flag, and keep secrets in `.env` without checking them into source control.

## Where `config.json` Comes From

By default, `noxa` loads `./config.json` from the current working directory.

You can override that in two ways:

- `--config <PATH>` on the CLI
- `NOXA_CONFIG=<PATH>` in the environment

If the file does not exist:

- an explicit `--config` path or `NOXA_CONFIG` path is an error
- the default `./config.json` is optional and missing files are ignored

To bypass config entirely for one run:

```bash
NOXA_CONFIG=/dev/null noxa https://example.com
```

## What Belongs Where

### `config.json`

Use `config.json` for stable, non-secret defaults such as:

- output format
- output directory
- browser fingerprint
- timeout
- crawl depth and page limits
- selector filters
- LLM provider and model

### `.env`

Use `.env` for secrets, URLs, and a small number of runtime overrides:

- `NOXA_API_KEY`
- `NOXA_PROXY`
- `NOXA_PROXY_FILE`
- `NOXA_WEBHOOK_URL`
- `NOXA_LLM_BASE_URL`

Those values are intentionally excluded from `config.json`.

If you run `setup.sh` or the Docker Compose stack, the generated `.env` may also include local deployment settings such as `NOXA_PORT`, `NOXA_HOST`, `NOXA_AUTH_KEY`, `NOXA_LOG`, `OLLAMA_HOST`, and `OLLAMA_MODEL`.

### CLI-only

These options stay on the command line and do not belong in `config.json`:

- `--on-change`
- `--raw-html`

`--on-change` is CLI-only because it executes shell commands. `--raw-html` is a per-run mode, not a persistent default.

## Config File Rules

- Keys are `snake_case`.
- All fields are optional.
- Unknown fields are ignored.
- Arrays are used for selector and path lists.
- Boolean flags have one important limitation: if you set them to `true` in `config.json`, you cannot disable them for a single CLI run with a `--no-...` flag because `noxa` does not define one.

The boolean fields with this limitation are:

- `metadata`
- `verbose`
- `only_main_content`
- `use_sitemap`

If you need to turn one of those off temporarily, bypass the config file with `NOXA_CONFIG=/dev/null`.

## Supported `config.json` Keys

### Output

| Key | Type | Default | Notes |
|---|---|---:|---|
| `format` | string | `markdown` | One of `markdown`, `json`, `text`, `llm`, `html` |
| `metadata` | boolean | `false` | Include metadata in output |
| `verbose` | boolean | `false` | Enable verbose logging |
| `output_dir` | string or null | `null` | Write outputs to files in this directory instead of stdout |

When `output_dir` is set, noxa writes results to files instead of printing them for the modes that support file output:

- single URL extraction
- multi-URL batch extraction
- crawl
- LLM extraction and summarization
- sitemap discovery
- diff output
- brand extraction
- research reports
- watch changes

File names are derived from the URL or mode name, and the directory is created on demand.

### Output Directory Layout

For URL-based output, noxa mirrors the URL path under `output_dir`:

| URL | Written file |
|---|---|
| `https://example.com/` | `output_dir/example_com/index.md` |
| `https://example.com/docs/api` | `output_dir/docs/api.md` |
| `https://example.com/docs/api/` | `output_dir/docs/api.md` |
| `https://example.com/blog/post?id=123` | `output_dir/blog/post_id_123.md` |

The extension comes from the selected output format:

| Format | Extension |
|---|---|
| `markdown` | `.md` |
| `llm` | `.md` |
| `json` | `.json` |
| `text` | `.txt` |
| `html` | `.html` |

For `--urls-file`, a CSV entry of `url,filename` uses the custom filename instead of the URL-derived name.

Examples:

```txt
https://example.com/docs/api,api.md
https://example.com/blog/post
```

Becomes:

```txt
output_dir/api.md
output_dir/blog/post.md
```

Mode-specific outputs use fixed filenames in the root of `output_dir`:

| Mode | File |
|---|---|
| `--map` | `sitemap.json` or `sitemap.txt` |
| `--diff-with` | `diff.json` or `diff.txt` |
| `--brand` | `brand.json` |
| `--research` | `research-<slug>.json` |
| `--watch` | `watch-<timestamp>.json` |

The directory tree is created automatically, so nested paths do not need to exist ahead of time.

### Fetch

| Key | Type | Default | Notes |
|---|---|---:|---|
| `browser` | string | `chrome` | One of `chrome`, `firefox`, `random` |
| `timeout` | integer | `30` | Request timeout in seconds |
| `pdf_mode` | string | `auto` | One of `auto`, `fast` |
| `only_main_content` | boolean | `false` | Auto-detect the main content area |

### Content Filtering

| Key | Type | Default | Notes |
|---|---|---:|---|
| `include_selectors` | array of strings | `[]` | CSS selectors to include |
| `exclude_selectors` | array of strings | `[]` | CSS selectors to exclude |

### Crawl

| Key | Type | Default | Notes |
|---|---|---:|---|
| `depth` | integer | `1` | Crawl depth |
| `max_pages` | integer | `20` | Maximum pages to crawl |
| `concurrency` | integer | `5` | Concurrent requests |
| `delay` | integer | `100` | Delay between requests in ms |
| `path_prefix` | string or null | `null` | Only crawl URLs whose path starts with this prefix |
| `include_paths` | array of strings | `[]` | Glob patterns to include |
| `exclude_paths` | array of strings | `[]` | Glob patterns to exclude |
| `use_sitemap` | boolean | `false` | Seed the crawl from sitemap discovery |

### LLM

| Key | Type | Default | Notes |
|---|---|---:|---|
| `llm_provider` | string | unset | Optional provider name: `gemini`, `ollama`, `openai`, `anthropic` |
| `llm_model` | string | unset | Optional model override |

## Environment Variables

| Variable | Purpose | Notes |
|---|---|---|
| `NOXA_API_KEY` | Cloud API key | Used for cloud fallback and cloud-only features |
| `SEARXNG_URL` | Self-hosted SearXNG base URL | Enables local search without `NOXA_API_KEY`; may be a localhost/private operator endpoint |
| `NOXA_NO_STORE` | Disable ContentStore | Set to any non-empty value to skip auto-persistence to `~/.noxa/content/`; per-run opt-out via `--no-store` |
| `NOXA_PROXY` | Single proxy URL | Takes priority over proxy file when set |
| `NOXA_PROXY_FILE` | Proxy pool file path | One proxy per line |
| `NOXA_WEBHOOK_URL` | Notification webhook | Used by watch/crawl/batch notifications; may be a localhost/private operator endpoint |
| `NOXA_LLM_BASE_URL` | LLM endpoint URL | For Ollama or OpenAI-compatible endpoints |
| `NOXA_LLM_PROVIDER` | Default LLM provider | Environment override for the provider name |
| `NOXA_LLM_MODEL` | Default LLM model | Environment override for the model name |
| `NOXA_CONFIG` | Config file path | Override `./config.json` or bypass with `/dev/null` |

`SEARXNG_URL` and `NOXA_WEBHOOK_URL` are treated as operator-supplied endpoints. They must still be valid `http://` or `https://` URLs, but they are allowed to point to localhost or private network addresses. Fetched target URLs and scraped result URLs continue to use the stricter public-address SSRF validation.

The following variables are not part of the `config.json` contract, but they still matter for LLM provider behavior:

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `OLLAMA_HOST`
- `OLLAMA_MODEL`
- `GEMINI_MODEL`

## Example

`config.example.json` shows the recommended baseline:

```json
{
  "$schema": "./config.schema.json",
  "_doc": [
    "Copy to config.json and remove fields you don't need.",
    "Secrets (api_key, proxy, webhook, llm_base_url) go in .env — NOT here."
  ],
  "format": "markdown",
  "browser": "chrome",
  "timeout": 30,
  "pdf_mode": "auto",
  "metadata": false,
  "verbose": false,
  "only_main_content": false,
  "include_selectors": [],
  "exclude_selectors": ["nav", "footer", ".sidebar", ".cookie-banner"],
  "depth": 1,
  "max_pages": 20,
  "concurrency": 5,
  "delay": 100,
  "path_prefix": null,
  "include_paths": [],
  "exclude_paths": ["/changelog/*", "/blog/*", "/releases/*"],
  "use_sitemap": false,
  "llm_provider": "gemini",
  "llm_model": "gemini-2.5-pro"
}
```

## Gotchas

- `config.json` is permissive by design: unknown fields are ignored so newer config files still work on older binaries.
- `llm_provider` is validated by the CLI at runtime; invalid values will fail when the provider is selected.
- `browser`, `timeout`, `depth`, `max_pages`, `concurrency`, and `delay` are ordinary defaults, so CLI flags can override them per run.
- Boolean defaults set to `true` in config are sticky for that run unless you bypass the file.

## Related Files

- [`config.schema.json`](../config.schema.json)
- [`config.example.json`](../config.example.json)
- [`env.example`](../env.example)
