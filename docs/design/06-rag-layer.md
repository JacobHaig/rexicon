# RAG Layer — Embeddings and Semantic Search

## Purpose

The RAG (Retrieval Augmented Generation) layer enables semantic search across all indexed content. Instead of keyword matching ("find me rows containing 'auth'"), the agent can ask natural language questions ("how does authentication work?") and get ranked results based on meaning.

This is Phase 5 — it builds on top of the symbol index, memory system, and relationship graph. The system works without RAG (keyword search + graph traversal), and RAG makes it dramatically better.

## Architecture

```
Query: "how does authentication work?"
         │
         ▼
┌─────────────────────┐
│  Embed the query    │  → vector (384 or 768 dimensions)
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  Vector similarity  │  Search embeddings table for nearest neighbors
│  search             │
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  Hydrate results    │  Join back to content/memory/symbols tables
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  Re-rank + merge    │  Combine with keyword results, deduplicate
└────────┬────────────┘
         ▼
        Results with scores, source references, and previews
```

## Embedding Model

### Default: Local embeddings via `fastembed`

The `fastembed` crate runs embedding models locally — no API calls, no network dependency, no cost per query.

Recommended model: `BAAI/bge-small-en-v1.5`
- 384 dimensions
- ~33MB model file (downloaded once, cached locally)
- ~10ms per embedding on modern hardware
- Good quality for code and technical text

Alternative for higher quality: `BAAI/bge-base-en-v1.5` (768 dimensions, ~110MB)

### Optional: API-based embeddings

For users who prefer higher-quality embeddings or have an API key:

```toml
# ~/.rexicon/config.toml
[embeddings]
provider = "local"           # "local", "openai", "voyage"
model = "bge-small-en-v1.5"  # or "text-embedding-3-small" for OpenAI
```

The embedding interface is abstracted so the provider is swappable.

## What Gets Embedded

| Source | Chunking strategy | Priority |
|---|---|---|
| **Memory entries** | Whole entry (title + body). Most are small enough. | Highest — agent-written knowledge is most valuable for semantic search |
| **Symbol signatures** | One embedding per symbol: `kind + signature + file path` | High — enables "find the function that validates tokens" |
| **Room summaries** | Whole summary | Medium — enables "which part of the code handles payments?" |
| **Architecture summary** | Chunked by section if long | Medium |
| **Content/documentation** | Chunked at ~512 tokens | Lower — supplementary context |

### Chunking Rules

- Entries under 512 tokens: embed as-is
- Entries over 512 tokens: split at paragraph boundaries, overlap by 50 tokens
- Each chunk stores a reference to its source row (table + ID)
- Symbols are never chunked — one embedding per symbol

## Storage

Embeddings are stored in the `embeddings` table as BLOBs (raw float32 arrays):

```sql
embeddings(
    source_table  TEXT,     -- 'content', 'memory', 'symbols'
    source_id     INTEGER,
    vector        BLOB,     -- 384 * 4 = 1,536 bytes per embedding
    model         TEXT
)
```

### Vector Search

**Option A: sqlite-vss** (SQLite extension for vector similarity)
- Integrates directly with existing SQLite database
- No additional process or library
- Performance: good for < 1M vectors

**Option B: In-process brute force**
- For small projects (< 10K embeddings), brute-force cosine similarity is fast enough (~1ms)
- Load all vectors into memory, compute similarities
- No extension needed

**Option C: HNSW index via `usearch` or `hnswlib`**
- For large projects (> 100K embeddings)
- Build an in-memory HNSW index from the SQLite vectors
- Sub-millisecond search even at scale

Recommendation: start with Option B (brute force), add sqlite-vss when projects exceed 10K embeddings.

## Search Pipeline

When `rexicon query "how does auth work?"` runs with RAG enabled:

1. **Keyword search** — standard FTS (full-text search) over content, memory, symbols
2. **Semantic search** — embed the query, find top-K nearest neighbors
3. **Graph augmentation** — for any result that's a symbol, include its immediate relationships
4. **Merge + deduplicate** — combine keyword and semantic results, remove duplicates, re-rank by combined score
5. **Return** — top N results with source references, scores, and previews

The merge step uses reciprocal rank fusion (RRF) or a simple weighted average of keyword and semantic scores.

## Incremental Embedding Updates

During re-indexing:
1. For each new or changed content/symbol row, generate a new embedding.
2. Delete old embeddings for removed content.
3. Memory embeddings update when memory is written or updated.

Embedding generation runs after the core indexing pipeline, so it doesn't slow down the fast path.

## Storage Size Estimates

| Project size | Symbols | Memory entries | Embedding storage |
|---|---|---|---|
| Small (100 files) | ~500 | ~20 | ~1 MB |
| Medium (1K files) | ~5,000 | ~100 | ~10 MB |
| Large (10K files) | ~50,000 | ~500 | ~100 MB |
| Huge (100K files) | ~500,000 | ~2,000 | ~1 GB |

This is well within SQLite's capabilities. The database file stays manageable.

## Configuration

```toml
# ~/.rexicon/config.toml

[rag]
enabled = true                        # false to skip embedding generation entirely
provider = "local"                    # "local" | "openai" | "voyage"
model = "bge-small-en-v1.5"          # model name
dimensions = 384                      # auto-detected from model
search_top_k = 20                     # max results from vector search before merge
min_similarity = 0.3                  # threshold below which results are dropped

[rag.openai]                          # only if provider = "openai"
api_key_env = "OPENAI_API_KEY"        # env var containing the key
model = "text-embedding-3-small"

[rag.voyage]                          # only if provider = "voyage"
api_key_env = "VOYAGE_API_KEY"
model = "voyage-code-2"               # optimized for code
```

## Fallback Behavior

If RAG is disabled or embeddings haven't been generated:
- `query` falls back to keyword search (FTS5)
- `memory search` falls back to LIKE matching on title + body + tags
- The response includes `"search_mode": "keyword"` so the agent knows

No feature depends on RAG being available. It's a quality multiplier, not a requirement.
