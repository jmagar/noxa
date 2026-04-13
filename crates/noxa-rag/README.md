# noxa-rag

RAG pipeline for [noxa](https://github.com/jmagar/noxa) — watches noxa's output directory for `ExtractionResult` JSON files, chunks them, embeds via [HF TEI](https://github.com/huggingface/text-embeddings-inference), and upserts to [Qdrant](https://qdrant.tech/).

## System Requirements

- **Qdrant** running locally (REST port 6333)
- **HF TEI** with GPU (tested on RTX 4070)
- **CUDA** for TEI inference (CPU mode is possible but slow)
- **Rust 1.82+**
- **huggingface-cli** to download the tokenizer

## CRITICAL: TEI Launch Command

```bash
# CRITICAL: --pooling last-token is REQUIRED for Qwen3-0.6B
# Qwen3 is a decoder-only model. Mean pooling (TEI default) produces
# semantically incorrect embeddings. This flag is NOT optional.
docker run --gpus all -p 8080:80 \
  ghcr.io/huggingface/text-embeddings-inference:latest \
  --model-id Qwen/Qwen3-Embedding-0.6B \
  --pooling last-token \
  --max-batch-tokens 32768 \
  --max-client-batch-size 128 \
  --dtype float16
```

### Verify TEI is working

```bash
curl http://localhost:8080/health
# {"status":"ok"}

# Check embedding dimensions (must be 1024 for Qwen3-0.6B)
curl -s http://localhost:8080/embed \
  -H "Content-Type: application/json" \
  -d '{"inputs": ["test"], "normalize": true}' | python3 -c "import sys,json; v=json.load(sys.stdin)[0]; print(f'{len(v)} dims')"
# 1024 dims
```

## Quickstart

### 1. Download the tokenizer

The Rust `tokenizers` crate cannot download from HF Hub at runtime. Download once:

```bash
pip install huggingface_hub
huggingface-cli download Qwen/Qwen3-Embedding-0.6B tokenizer.json --local-dir ~/.cache/noxa-rag/tokenizer
```

### 2. Create config file

```toml
# noxa-rag.toml

[source]
type = "fs_watcher"
watch_dir = "/home/user/.noxa/output"
debounce_ms = 500

[embed_provider]
type = "tei"
url = "http://localhost:8080"
model = "Qwen/Qwen3-Embedding-0.6B"
# REQUIRED: path to directory containing tokenizer.json
local_path = "/home/user/.cache/noxa-rag/tokenizer"

[vector_store]
type = "qdrant"
# REST port 6333
url = "http://localhost:6333"
collection = "noxa_rag"
# api_key = "..."          # or set NOXA_RAG_QDRANT_API_KEY env var

[chunker]
target_tokens = 512
overlap_tokens = 64
min_words = 50
max_chunks_per_page = 100

[pipeline]
embed_concurrency = 4
# Must be an absolute path (daemon may run with CWD = /)
failed_jobs_log = "/home/user/.noxa/noxa-rag-failed.jsonl"
```

### 3. Start Qdrant

```bash
docker run -p 6333:6333 -p 6334:6334 \
  -v ~/.noxa/qdrant:/qdrant/storage \
  qdrant/qdrant
```

### 4. Run the daemon

```bash
cargo build --release -p noxa-rag
./target/release/noxa-rag-daemon --config noxa-rag.toml
```

### 5. Index content with noxa

```bash
# Extract a page — the daemon will pick up the output file automatically
noxa https://docs.example.com --output ~/.noxa/output/
```

The daemon watches `watch_dir` for `.json` files. When noxa writes an `ExtractionResult` to that directory, the daemon detects it (within `debounce_ms` ms), chunks it, embeds it, and upserts to Qdrant.

## Configuration Reference

| Field | Default | Description |
|-------|---------|-------------|
| `source.watch_dir` | — | Directory to watch for `.json` files |
| `source.debounce_ms` | `500` | Debounce window for filesystem events (ms) |
| `embed_provider.url` | — | TEI server URL |
| `embed_provider.model` | — | Model name (used in logs) |
| `embed_provider.local_path` | **required** | Directory containing `tokenizer.json` |
| `vector_store.url` | — | Qdrant REST URL (port 6333) |
| `vector_store.collection` | — | Qdrant collection name |
| `vector_store.api_key` | `null` | Qdrant API key (or `NOXA_RAG_QDRANT_API_KEY` env var) |
| `chunker.target_tokens` | `512` | Target chunk size in tokens |
| `chunker.overlap_tokens` | `64` | Sliding window overlap tokens |
| `chunker.min_words` | `50` | Skip chunks shorter than this |
| `chunker.max_chunks_per_page` | `100` | Cap chunks per document |
| `pipeline.embed_concurrency` | `4` | Concurrent embed workers (must be > 0) |
| `pipeline.failed_jobs_log` | `null` | Absolute path for NDJSON error log |

## Architecture

```text
noxa-cli (writes .json) → watch_dir
                                ↓
              notify-debouncer-mini (500ms debounce)
                                ↓
              bounded mpsc channel (256 capacity)
                                ↓
          embed_concurrency worker tasks (default: 4)
                                ↓
              ┌─────────────────────────────────────┐
              │  process_job()                       │
              │  1. Read file (TOCTOU-safe)          │
              │  2. Parse ExtractionResult JSON      │
              │  3. Validate URL scheme (http/https) │
              │  4. chunk() → Vec<Chunk>             │
              │  5. embed() → Vec<Vec<f32>>          │
              │  6. UUID v5 point IDs                │
              │  7. Per-URL mutex: delete + upsert   │
              └─────────────────────────────────────┘
                                ↓
                         Qdrant (REST)
```

## Notes

- **Recursive watch**: The daemon watches `watch_dir` recursively, so crawl output saved under nested path-based directories is indexed automatically.
- **Vim/Emacs compatibility**: The daemon watches all filesystem events (not just Create/Modify). Atomic saves via rename are detected correctly.
- **Idempotent indexing**: Re-indexing the same URL deletes old chunks first (delete-before-upsert), so chunk count changes are handled correctly.
- **Point IDs**: UUID v5 deterministic — same URL + chunk index always produces the same Qdrant point ID.
- **Failed jobs**: Parse failures and oversized files (>50MB) are logged to `failed_jobs_log` as NDJSON and skipped (the daemon keeps running).
