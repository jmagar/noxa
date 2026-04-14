# Content Store — Design Spec

## Status

| Section | Status |
|---|---|
| Per-URL sidecar as diff changelog | ✅ Implemented (PR #7) |
| Config options through crawl/search | ✅ Implemented (PR #6) |
| File layout — per-URL vs domain-level | ✅ Approved |
| Architecture — `noxa-store` crate split | ✅ Approved |
| Operations log format — NDJSON (not JSON array) | ✅ Approved |
| `Op` typed enum (replaces `op: String`) | ✅ Approved |
| Security stripping (`raw_html`, `file_path`, `search_query`) | ✅ Approved |
| File permissions (`0o700`/`0o600`) | ✅ Approved |
| `dirs::home_dir()` hard error (no `"."` fallback) | ✅ Approved |
| `NOXA_NO_CONTENT_STORE` rename + `NOXA_NO_OPERATIONS_LOG` | ✅ Approved |
| `FetchMethod` in sidecar entries | ⏸ Deferred — no consumer yet |
| Backend traits (`ContentBackend`, `OperationsBackend`) | ⏸ Deferred — start with concrete types |
| `query()` on `OperationsBackend` | ⏸ Deferred — no read consumer yet |
| Moving diff/watch into `noxa-fetch` | ⏸ Deferred — keep in CLI, add `append()` call inline |
| `NullOperationsLog` (no-op impl) | ⏸ Deferred — needed before test suite ships |
| Sidecar changelog cap (N entries max) | ⏸ Deferred — implement before unbounded growth is observed |
| `.index.ndjson` for `--list`/`--retrieve` | ⏸ Deferred — implement before >1k pages |
| Directory hash sharding | ⏸ Deferred — implement before >5k files per domain |
| Create `crates/noxa-store/` — move store.rs + add new types | 🔲 Pending |
| `FilesystemOperationsLog` (NDJSON) | 🔲 Pending |
| Move `content_store_root()` from `noxa-cli` to `noxa-store` | 🔲 Pending |
| Wire map/brand → `OperationsLog` in `noxa-fetch` | 🔲 Pending |
| Wire diff/watch → `OperationsLog` in CLI (inline `append()`) | 🔲 Pending |
| Wire summarize/extract → `OperationsLog` in `noxa-llm` | 🔲 Pending |
| Watch restart continuity + `is_initial_baseline` guard | 🔲 Pending |
| Security stripping in `ContentStore::write` | 🔲 Pending |
| File permission enforcement (`0o700`/`0o600`) | 🔲 Pending |
| Delete `write_to_file` loose-file calls in `noxa-cli` | 🔲 Pending |

---

## Goals

- **Single canonical location** for all content, regardless of how it was obtained
  (scrape, crawl, batch, search, MCP).
- **One document per URL** — re-fetching the same URL updates in place; never creates duplicates.
- **Full history** — every fetch that changes content appends a `ContentDiff` entry to the
  per-URL sidecar. The sidecar is a master changelog, not a last-diff snapshot.
- **LLM outputs and operations are first-class** — saved automatically, never to loose files.
- **Watch continuity across restarts** — on startup, `--watch` reads the last stored snapshot
  from the store as the baseline. Restarts do not lose the watch baseline or miss changes.
- **Fast local retrieval** — `--list` and `--retrieve` read a flat index, not the full sidecar tree.
- **Secure by default** — sensitive fields stripped before write; files created `0o600`; home dir
  absence is a hard error.
- **Configurable root** — `output_dir` in `config.json` sets the store root.
  Default: `~/.noxa/content/`.

---

## Approved File Layout

```
<store_root>/                               # default: ~/.noxa/content/
  .index.ndjson                             # flat URL index — updated on every store.write()
  <domain>/
    .operations.ndjson                      # domain-level ops log (NDJSON, one entry per line)
    <path>.md                               # current markdown (overwritten on content change)
    <path>.json                             # per-URL sidecar: content diff changelog only
```

### What goes where

| Data | Location | Why |
|---|---|---|
| Scraped content (current) | `<path>.md` | Human-readable, diffable |
| Content diff history | `<path>.json` | Per-URL, grows only when content changes |
| URL index | `.index.ndjson` | Fast `--list`/`--retrieve` without sidecar walk |
| Sitemap (`--map`) | `.operations.ndjson` | Domain-level — no single URL |
| Brand (`--brand`) | `.operations.ndjson` | Domain identity, not one page |
| Summarize (`--summarize`) | `.operations.ndjson` | LLM output varies run-to-run; not a content event |
| Extract (`--extract-json`, `--extract-prompt`) | `.operations.ndjson` | Same reasoning |
| Diff (`--diff-with`, MCP `diff`) | `.operations.ndjson` | Analytical result; content write already in sidecar |
| Watch change event (`--watch`) | `.operations.ndjson` | Each detected change = diff entry |

**Key decisions:**
- Per-URL `.json` = content events only (diffs). Stays small and focused.
- `.operations.ndjson` = all derived/analytical operations, one JSON object per line.
  Append-only. Gives a single audit log: "every operation ever run against this domain."
- LLM outputs are NOT in the content changelog — LLM output varies for identical content
  and would pollute the diff history with noise.
- `.index.ndjson` = one line per stored URL. `--list`/`--retrieve` never touch the sidecar tree.

---

## Per-URL Sidecar Schema (`<path>.json`)

```jsonc
{
  "schema_version": 1,
  "url": "https://modelcontextprotocol.io/specification/2025-11-25/server/tools",
  "first_seen": "2025-04-13T00:00:00Z",
  "last_fetched": "2025-04-14T12:00:00Z",
  "fetch_count": 3,
  "current": { /* full ExtractionResult — store.read() reads this */ },
  "changelog": [
    {
      "at": "2025-04-13T00:00:00Z",
      "word_count": 1580,
      "diff": null                          // null = first fetch
    },
    {
      "at": "2025-04-14T12:00:00Z",
      "word_count": 1597,
      "diff": { /* noxa_core::ContentDiff */ }
    }
  ]
}
```

**Rules:**
- Entry appended only when content changes.
- Identical re-fetch: update `last_fetched` + `fetch_count` only, no `.md` rewrite.
- Uses existing `noxa_core::diff::diff()` — no new diff code.
- Lazy migration of old-format `.json` files on first access — migration must use same-directory
  tmp+rename (not system `/tmp`) to avoid `EXDEV` cross-filesystem failures.
- `FetchMethod` field in changelog entries is **deferred** — no consumer exists yet. Add when a
  code path branches on it.
- Changelog capped at **52 entries** (one year of weekly watches). Oldest entry dropped when cap
  is exceeded. *(Implement before unbounded growth is observed in the wild.)*

**What is NOT stored in the sidecar:**
- `raw_html` — stripped before write (persistent XSS surface for downstream renderers)
- `metadata.file_path` — stripped before write (leaks local filesystem paths)
- `metadata.search_query` — stripped before write (leaks user search intent)

All three are stripped in `ContentStore::write` alongside the existing `metadata.url` query-param strip.

---

## `.operations.ndjson` Schema — Approved

Location: `<store_root>/<domain>/.operations.ndjson`

**Format: NDJSON** — one JSON object per line. O(1) appends via `OpenOptions::append(true)`.
No wrapping array. No read-modify-write cycle. No mutex needed for appends.

```jsonc
// Each line is a complete JSON object. No commas between lines.
{"op":"map","at":"2025-04-13T00:00:00Z","url":"https://modelcontextprotocol.io","input":{...},"output":[...]}
{"op":"brand","at":"2025-04-13T00:00:00Z","url":"https://modelcontextprotocol.io","input":{},"output":{...}}
{"op":"summarize","at":"2025-04-13T00:00:00Z","url":"https://...","input":{"sentences":5,"provider":"gemini","model":"gemini-2.5-pro"},"output":"The tools specification defines..."}
{"op":"extract","at":"2025-04-13T00:00:00Z","url":"https://...","input":{"kind":"json","schema":{...},"provider":"gemini","model":"gemini-2.5-pro"},"output":{...}}
{"op":"extract","at":"2025-04-13T00:00:00Z","url":"https://...","input":{"kind":"prompt","prompt":"Get all method names","provider":"gemini","model":"gemini-2.5-pro"},"output":"..."}
{"op":"diff","at":"2025-04-14T12:00:00Z","url":"https://...","input":{"source":"file"},"output":{...}}
{"op":"diff","at":"2025-04-14T13:00:00Z","url":"https://...","input":{"source":"watch","interval_secs":300},"output":{...}}
```

**`op` field is a typed enum** (`Op::Map`, `Op::Brand`, `Op::Summarize`, `Op::Extract`, `Op::Diff`)
serialized as lowercase string. Using `String` would lose compile-time enforcement of valid variants.

**`output` size guard:** entries with `output` exceeding 1 MB are truncated with `"truncated": true`
marker — same philosophy as the 2 MiB `max_content_bytes` guard in `ContentStore`.

### Decisions on `.operations.ndjson`

- **`noxa-store` owns all persistence** — implementation lives exclusively in `noxa-store`.
  Service crates trigger persistence by calling `OperationsLog::append()` after their operation.
  CLI/MCP never call `append()` or `store.write()` directly — they only wire config.
- **NDJSON not JSON array** — O(1) append, no read-modify-write, no mutex for appends. Querying
  scans lines. This was changed from the initial JSON-array design after engineering review
  identified O(n) write cost as a blocking scalability issue.
- **diff/watch stay in CLI** — moved to `noxa-fetch` is over-scoped. `append()` is called inline
  in `run_diff` and `run_watch_single`/`run_watch_multi` after computing the result.
- **watch baseline**: on startup, read last stored snapshot from `ContentStore` as `previous`.
  If no stored snapshot exists, fetch and store as baseline. The initial reconciliation write
  must NOT fire `--on-change` — gate on `is_initial_baseline: bool` flag.

---

## `output_dir` Semantics

| `output_dir` value     | Effective store root                 |
|------------------------|--------------------------------------|
| _(unset)_              | `~/.noxa/content/`                   |
| `/home/jmagar`         | `/home/jmagar/.noxa/content/`        |

Never writes loose files directly into `output_dir`.

`dirs::home_dir()` returning `None` is a **hard error** — process exits with a clear message.
The previous `unwrap_or_else(|| PathBuf::from("."))` fallback is removed. Using `"."` as the
store root breaks path containment guarantees and scatters data unpredictably in containers.

---

## Architecture

```
noxa-core    pure extraction, diff, types — WASM-safe, no I/O
             ↳ diff::diff() — reuse as-is

noxa-store   PERSISTENCE LAYER (new crate — no network, pure filesystem)
             ↳ FilesystemContentStore — per-URL sidecar + .md files + index
             ↳ FilesystemOperationsLog — domain-level .operations.ndjson
             ↳ path utilities: url_to_store_path(), content_store_root(), domain_from_url()
             ↳ Op enum, OperationEntry, StoreResult
             deps: noxa-core, serde, serde_json, tokio/fs, chrono, url, dirs, rand, tracing

noxa-fetch   SERVICE LAYER — depends on noxa-store
             ↳ FetchClient::fetch_and_extract_with_options()
               single convergence point — store.write() called here automatically
             ↳ map, brand business logic — calls ops_log.append() after each
             ↳ diff/watch stay in noxa-cli (see decisions); ops_log.append() called inline there

noxa-llm     SERVICE LAYER — depends on noxa-store
             ↳ summarize(), extract() — calls ops_log.append() after each

noxa-cli     CLI shim — constructs FilesystemContentStore + FilesystemOperationsLog,
             passes to service crates; calls ops_log.append() for diff/watch inline
             NOTE: noxa-cli must add noxa-store as an explicit Cargo.toml dependency
noxa-mcp     MCP shim — same construction pattern
```

**Key invariant:** `noxa-store` owns all persistence implementation. Service crates trigger
persistence by calling store/log APIs after their operations complete. CLI and MCP own the
construction of store objects and the wiring — nothing else.

**On trait abstraction:** `ContentBackend` and `OperationsBackend` traits are **deferred**.
Start with concrete types (`FilesystemContentStore`, `FilesystemOperationsLog`) passed directly.
Introduce traits when a second concrete backend is actively being implemented — not before.
`Arc<dyn Trait>` overhead and `Send+Sync` constraint spread across all callers is not worth
paying for a backend that doesn't exist.

---

## `noxa-store` Crate — Approved Design

**Location:** `crates/noxa-store/`

**Why a separate crate:** `noxa-llm` needs to write to `OperationsLog`. `noxa-llm` depending on
`noxa-fetch` would be wrong — an LLM provider chain shouldn't pull in an HTTP client. Persistence
lives here so both service crates can share it without a circular dependency.

### Cargo.toml dependencies

```toml
[dependencies]
noxa-core = { path = "../noxa-core" }   # for ContentDiff, ExtractionResult types
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["fs", "io-util"] }
chrono = { version = "0.4", features = ["serde"] }
url = "2"
dirs = "5"
rand = "0.8"                             # atomic write tmp suffix
tracing = "0.1"
```

No reqwest, no primp, no network dependencies.

### Module structure

```
crates/noxa-store/src/
  lib.rs               — re-exports all public types
  content_store.rs     — FilesystemContentStore (moved from noxa-fetch/src/store.rs)
  operations_log.rs    — FilesystemOperationsLog (NDJSON)
  index.rs             — IndexStore (.index.ndjson — updated on every store.write())
  paths.rs             — url_to_store_path(), content_store_root(), domain_from_url()
  types.rs             — StoreResult, OperationEntry, Op enum, IndexEntry
```

No `traits.rs` — deferred until a second backend exists.

### `FilesystemContentStore`

Moved from `crates/noxa-fetch/src/store.rs`. No behavior changes during the move.

**Additional responsibilities after move:**

**Security stripping** — applied in `write()` before serialization, alongside existing query-param strip:
```rust
to_store.content.raw_html = None;          // persistent XSS surface
to_store.metadata.file_path = None;        // leaks local filesystem paths
to_store.metadata.search_query = None;     // leaks user search intent
```

**File permissions** — all created directories and files use explicit permissions:
```rust
// After create_dir_all:
std::fs::set_permissions(&dir, Permissions::from_mode(0o700))?;
// After atomic rename:
std::fs::set_permissions(&final_path, Permissions::from_mode(0o600))?;
```

**Hard error on missing home dir:**
```rust
dirs::home_dir().ok_or_else(|| "cannot determine home directory: $HOME is unset".to_string())?
```

**Index update** — on every successful `write()`, append a line to `.index.ndjson`:
```jsonc
{"url":"https://...","path":"domain/path","title":"...","word_count":1597,"fetched_at":"..."}
```

**Sidecar migration** — old-format `.json` (raw `ExtractionResult`, no `schema_version`) detected
on read and migrated inline. Migration tmp file written to the **same directory** as the target
(not system `/tmp`) to guarantee rename is always on the same filesystem.

### `FilesystemOperationsLog`

New. NDJSON format — one JSON object per line.

```rust
pub struct FilesystemOperationsLog {
    root: PathBuf,
}
```

No `DashMap`, no per-domain mutex. NDJSON appends are O(1) and safe:

```rust
impl FilesystemOperationsLog {
    pub async fn append(&self, domain: &str, entry: &OperationEntry) -> Result<(), String> {
        let path = self.root.join(domain).join(".operations.ndjson");
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        // Set dir permissions 0o700 on first create
        let line = serde_json::to_string(entry)? + "\n";
        let mut file = tokio::fs::OpenOptions::new()
            .create(true).append(true).open(&path).await?;
        file.write_all(line.as_bytes()).await?;
        Ok(())
    }
}
```

`OpenOptions::append(true)` on POSIX: each `write()` is atomic for writes up to `PIPE_BUF`
(4096 bytes on Linux). Entries under ~4 KB (the common case) are atomic without a mutex.
For larger entries (e.g. large LLM outputs), a per-call file lock is acquired.

`query()` is **stubbed** — returns `unimplemented!()` or empty vec until a read consumer exists.
Do not design the query API until `--list-ops` or similar is being built.

### `Op` Enum

Replaces `op: String` in `OperationEntry`. Compile-time enforcement of valid variants:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Map,
    Brand,
    Summarize,
    Extract,
    Diff,
}
```

### `OperationEntry`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationEntry {
    pub op: Op,                            // typed enum, not String
    pub at: chrono::DateTime<Utc>,
    pub url: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,         // truncated to 1 MB with "truncated":true marker
}
```

`output` remains `serde_json::Value` for pragmatic flexibility in the append path. If `query()`
is ever built for callers that need typed outputs, introduce a typed variant at that point.

### `NullOperationsLog` *(deferred)*

Needed before the test suite ships — a no-op implementation for tests that need a fake store
and for cases where all persistence is truly disabled. Implement alongside the first test that
constructs a `FetchConfig` with a store.

### `NOXA_NO_CONTENT_STORE` / `NOXA_NO_OPERATIONS_LOG`

`NOXA_NO_STORE` is renamed to `NOXA_NO_CONTENT_STORE` for clarity. A user setting "no store"
reasonably expects nothing is persisted — but the operations log (which stores LLM prompts,
schemas, and extracted data) would still write. Separate controls make the scope explicit:

| Env var | Effect |
|---|---|
| `NOXA_NO_CONTENT_STORE` | Disables `FilesystemContentStore` — no `.md` or `.json` sidecar writes |
| `NOXA_NO_OPERATIONS_LOG` | Disables `FilesystemOperationsLog` — no `.operations.ndjson` writes |
| *(both set)* | All persistence disabled — equivalent to old `NOXA_NO_STORE` |

`NOXA_NO_STORE` is kept as a backwards-compatible alias that sets both flags.

### What moves vs what is new

| Item | Status | Notes |
|---|---|---|
| `url_to_store_path()` | moves from `noxa-fetch` | no changes |
| `sanitize_component()` | moves from `noxa-fetch` | internal, not re-exported |
| `FilesystemContentStore` | moves from `noxa-fetch` | adds security stripping, permissions, index update |
| `StoreResult` | moves from `noxa-fetch` | no changes |
| `content_store_root()` | moves from `noxa-cli` | belongs in persistence layer |
| `FilesystemOperationsLog` | new | NDJSON, no mutex |
| `OperationEntry` | new | |
| `Op` enum | new | replaces `op: String` |
| `IndexEntry` | new | for `.index.ndjson` |
| `FetchMethod` | deferred | add when a code path branches on it |
| `ContentBackend` trait | deferred | add when second backend exists |
| `OperationsBackend` trait | deferred | same |

**No re-export shim.** `noxa-fetch` call sites are updated atomically in the same PR that moves
the types. No transition period.

---

## Security Decisions

| Finding | Decision |
|---|---|
| `raw_html` stored unsanitized | Strip in `ContentStore::write` — never persisted |
| `metadata.file_path` leaked | Strip in `ContentStore::write` alongside query-param strip |
| `metadata.search_query` leaked | Strip in `ContentStore::write` alongside query-param strip |
| World-readable files (umask 022) | Explicit `0o700`/`0o600` on all created dirs/files |
| `dirs::home_dir()` fallback to `"."` | Hard error — no fallback |
| `NOXA_NO_STORE` misleading scope | Renamed; separate flags for content vs ops |
| TOCTOU between `resolve_path` and write | Deferred — low exploitability; address with `O_NOFOLLOW` in a security hardening PR |
| `output_dir` inside git repo | Deferred — add startup warning when store root is inside a git repository |

---

## Performance Decisions

| Finding | Decision |
|---|---|
| `collect_docs` O(n) full walk on `--list` | Fix with `.index.ndjson` (implement before >1k pages) |
| `FilesystemOperationsLog` O(n) read-modify-write | Fixed by NDJSON design (O(1) append) |
| Sidecar changelog unbounded growth | Cap at 52 entries; implement before observed in wild |
| `store.write()` reads full previous sidecar | Deferred — content hash optimization when profiling shows it's hot |
| `resolve_path` sync stat on async executor | Deferred — restructure after `create_dir_all` |
| Watch loop `HashMap<String, ExtractionResult>` | Deferred — switch to `(hash, word_count)` at 100+ watched URLs |
| Directory listing (no sharding) | Deferred — hash sharding before >5k files per domain |

---

## All Write Paths

| Operation | Content store (`store.write`) | Operations log (`.operations.ndjson`) |
|---|---|---|
| `noxa <url>` | ✅ via `FetchClient` | — |
| `noxa --crawl` | ✅ via `FetchClient` per page | — |
| `noxa --batch` | ✅ via `FetchClient` per URL | — |
| `noxa --search` | ✅ via `FetchClient` per result | — |
| `noxa --map` | — | ✅ append entry |
| `noxa --brand` | — | ✅ append entry |
| `noxa --summarize` | — | ✅ append entry |
| `noxa --extract-json` | — | ✅ append entry |
| `noxa --extract-prompt` | — | ✅ append entry |
| `noxa --diff-with` | ✅ via `FetchClient` (fetches current; reads previous from file) | ✅ append entry (inline in CLI) |
| `noxa --watch` (on change) | ✅ via `FetchClient` each interval | ✅ append entry per change (inline in CLI) |
| MCP `scrape` | ✅ via `FetchClient` | — |
| MCP `crawl` | ✅ via `FetchClient` per page | — |
| MCP `batch` | ✅ via `FetchClient` per URL | — |
| MCP `search` | ✅ via `FetchClient` per result | — |
| MCP `map` | — | ✅ append entry |
| MCP `brand` | — | ✅ append entry |
| MCP `summarize` | — | ✅ append entry |
| MCP `extract` | — | ✅ append entry |
| MCP `diff` | ✅ via `FetchClient` (fetches current; reads previous from store) | ✅ append entry |
