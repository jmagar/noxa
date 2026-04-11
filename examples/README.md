# Examples

Practical examples showing what noxa can do. Each example is a self-contained command you can run immediately.

## Basic Extraction

```bash
# Extract as markdown (default)
noxa https://example.com

# Multiple output formats
noxa https://example.com -f markdown    # Clean markdown
noxa https://example.com -f json        # Full structured JSON
noxa https://example.com -f text        # Plain text (no formatting)
noxa https://example.com -f llm         # Token-optimized for LLMs (67% fewer tokens)

# Bare domains work (auto-prepends https://)
noxa example.com
```

## Content Filtering

```bash
# Only extract main content (skip nav, sidebar, footer)
noxa https://docs.rs/tokio --only-main-content

# Include specific CSS selectors
noxa https://news.ycombinator.com --include ".titleline,.score"

# Exclude specific elements
noxa https://example.com --exclude "nav,footer,.ads,.sidebar"

# Combine both
noxa https://docs.rs/reqwest --only-main-content --exclude ".sidebar"
```

## Brand Identity Extraction

```bash
# Extract colors, fonts, logos from any website
noxa --brand https://stripe.com
# Output: { "name": "Stripe", "colors": [...], "fonts": ["Sohne"], "logos": [...] }

noxa --brand https://github.com
# Output: { "name": "GitHub", "colors": [{"hex": "#1F2328", ...}], "fonts": ["Mona Sans"], ... }

noxa --brand wikipedia.org
# Output: 10 colors, 5 fonts, favicon, logo URL
```

## Sitemap Discovery

```bash
# Discover all URLs from a site's sitemaps
noxa --map https://sitemaps.org
# Output: one URL per line (84 URLs found)

# JSON output with metadata
noxa --map https://sitemaps.org -f json
# Output: [{ "url": "...", "last_modified": "...", "priority": 0.8 }]
```

## Recursive Crawling

```bash
# Crawl a site (default: depth 1, max 20 pages)
noxa --crawl https://example.com

# Control depth and page limit
noxa --crawl --depth 2 --max-pages 50 https://docs.rs/tokio

# Crawl with sitemap seeding (finds more pages)
noxa --crawl --sitemap --depth 2 https://docs.rs/tokio

# Filter crawl paths
noxa --crawl --include-paths "/api/*,/guide/*" https://docs.example.com
noxa --crawl --exclude-paths "/changelog/*,/blog/*" https://docs.example.com

# Control concurrency and delay
noxa --crawl --concurrency 10 --delay 200 https://example.com
```

## Change Detection (Diff)

```bash
# Step 1: Save a snapshot
noxa https://example.com -f json > snapshot.json

# Step 2: Later, compare against the snapshot
noxa --diff-with snapshot.json https://example.com
# Output:
#   Status: Same
#   Word count delta: +0

# If the page changed:
#   Status: Changed
#   Word count delta: +42
#   --- old
#   +++ new
#   @@ -1,3 +1,3 @@
#   -Old content here
#   +New content here
```

## PDF Extraction

```bash
# PDF URLs are auto-detected via Content-Type
noxa https://example.com/report.pdf

# Control PDF mode
noxa --pdf-mode auto https://example.com/report.pdf  # Error on empty (catches scanned PDFs)
noxa --pdf-mode fast https://example.com/report.pdf  # Return whatever text is found
```

## Batch Processing

```bash
# Multiple URLs in one command
noxa https://example.com https://httpbin.org/html https://rust-lang.org

# URLs from a file (one per line, # comments supported)
noxa --urls-file urls.txt

# Batch with JSON output
noxa --urls-file urls.txt -f json

# Proxy rotation for large batches
noxa --urls-file urls.txt --proxy-file proxies.txt --concurrency 10
```

## Local Files & Stdin

```bash
# Extract from a local HTML file
noxa --file page.html

# Pipe HTML from another command
curl -s https://example.com | noxa --stdin

# Chain with other tools
noxa https://example.com -f text | wc -w    # Word count
noxa https://example.com -f json | jq '.metadata.title'  # Extract title with jq
```

## Cloud API Mode

When you have a noxa API key, the CLI can route through the cloud for bot protection bypass, JS rendering, and proxy rotation.

```bash
# Set API key (one time)
export NOXA_API_KEY=wc_your_key_here

# Automatic fallback: tries local first, cloud on bot detection
noxa https://protected-site.com

# Force cloud mode (skip local, always use API)
noxa --cloud https://spa-site.com

# Cloud mode works with all features
noxa --cloud --brand https://stripe.com
noxa --cloud -f json https://producthunt.com
noxa --cloud --crawl --depth 2 https://protected-docs.com
```

## Browser Impersonation

```bash
# Chrome (default) — latest Chrome TLS fingerprint
noxa https://example.com

# Firefox fingerprint
noxa --browser firefox https://example.com

# Random browser per request (good for batch)
noxa --browser random --urls-file urls.txt
```

## Custom Headers & Cookies

```bash
# Custom headers
noxa -H "Authorization: Bearer token123" https://api.example.com
noxa -H "Accept-Language: de-DE" https://example.com

# Cookies
noxa --cookie "session=abc123; theme=dark" https://example.com

# Multiple headers
noxa -H "X-Custom: value" -H "Authorization: Bearer token" https://example.com
```

## LLM-Powered Features

These require an LLM provider (Ollama local, or OpenAI/Anthropic API key).

```bash
# Summarize a page (default: 3 sentences)
noxa --summarize https://example.com

# Control summary length
noxa --summarize 5 https://example.com

# Extract structured JSON with a schema
noxa --extract-json '{"type":"object","properties":{"title":{"type":"string"},"price":{"type":"number"}}}' https://example.com/product

# Extract with a schema from file
noxa --extract-json @schema.json https://example.com/product

# Extract with natural language prompt
noxa --extract-prompt "Get all pricing tiers with name, price, and features" https://stripe.com/pricing

# Use a specific LLM provider
noxa --llm-provider ollama --summarize https://example.com
noxa --llm-provider openai --llm-model gpt-4o --extract-prompt "..." https://example.com
noxa --llm-provider anthropic --summarize https://example.com
```

## Raw HTML Output

```bash
# Get the raw fetched HTML (no extraction)
noxa --raw-html https://example.com

# Useful for debugging extraction issues
noxa --raw-html https://example.com > raw.html
noxa --file raw.html  # Then extract locally
```

## Metadata & Verbose Mode

```bash
# Include YAML frontmatter with metadata
noxa --metadata https://example.com
# Output:
#   ---
#   title: "Example Domain"
#   source: "https://example.com"
#   word_count: 20
#   ---
#   # Example Domain
#   ...

# Verbose logging (debug extraction pipeline)
noxa -v https://example.com
```

## Proxy Usage

```bash
# Single proxy
noxa --proxy http://user:pass@proxy.example.com:8080 https://example.com

# SOCKS5 proxy
noxa --proxy socks5://proxy.example.com:1080 https://example.com

# Proxy rotation from file (one per line: host:port:user:pass)
noxa --proxy-file proxies.txt https://example.com

# Auto-load proxies.txt from current directory
echo "proxy1.com:8080:user:pass" > proxies.txt
noxa https://example.com  # Automatically detects and uses proxies.txt
```

## MCP Server (AI Agent Integration)

```bash
# Start the MCP server (stdio transport)
noxa-mcp

# Configure in Claude Desktop (~/.config/claude/claude_desktop_config.json):
# {
#   "mcpServers": {
#     "noxa": {
#       "command": "/path/to/noxa-mcp",
#       "env": {
#         "NOXA_API_KEY": "wc_your_key"  // optional, enables cloud fallback
#       }
#     }
#   }
# }

# Available tools: scrape, crawl, map, batch, extract, summarize, diff, brand, research, search
```

## Real-World Recipes

### Monitor competitor pricing

```bash
# Save today's pricing
noxa --extract-json '{"type":"array","items":{"type":"object","properties":{"plan":{"type":"string"},"price":{"type":"string"}}}}' \
  https://competitor.com/pricing -f json > pricing-$(date +%Y%m%d).json
```

### Build a documentation search index

```bash
# Crawl docs and extract as LLM-optimized text
noxa --crawl --sitemap --depth 3 --max-pages 500 -f llm https://docs.example.com > docs.txt
```

### Extract all images from a page

```bash
noxa https://example.com -f json | jq -r '.content.images[].src'
```

### Get all external links

```bash
noxa https://example.com -f json | jq -r '.content.links[] | select(.href | startswith("http")) | .href'
```

### Compare two pages

```bash
noxa https://site-a.com -f json > a.json
noxa https://site-b.com --diff-with a.json
```
