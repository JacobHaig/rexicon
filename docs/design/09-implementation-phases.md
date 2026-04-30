# Implementation Phases

## Overview

The build is split into 6 phases. Each phase produces a working, shippable increment — no phase depends on a later phase being complete. Phase 1 is the foundation; phases 2–6 add capabilities on top.

```
Phase 1: Foundation ──→ Phase 2: Memory ──→ Phase 3: Relationships
                                                      │
Phase 4: MCP Server ◄────────────────────────────────┘
         │
         ▼
Phase 5: RAG ──→ Phase 6: Synthesis
```

Phases 4–6 can be reordered based on priority. Phase 4 (MCP) is listed after 3 because it benefits from having all three subsystems to expose, but it could be built earlier with whatever subsystems exist at that point.

---

## Phase 1: Foundation

**Goal:** SQLite storage, incremental indexing, CLI restructure, hierarchy auto-generation. Everything in the current `rexicon` still works, plus data goes into a database.

### Tasks

1. **Add `rusqlite` dependency** and create `db.rs`
   - Connection management (open/create `~/.rexicon/store.db`)
   - Schema migration system (embed SQL migrations, run on first connect)
   - Initial schema: `projects`, `rooms`, `topics`, `content`, `symbols`, `file_index`
   - Transaction helpers

2. **Create `schema.rs`**
   - Rust types mirroring each table (row structs)
   - Insert/update/delete/query functions per table
   - Scope path parsing utilities

3. **Create `hierarchy.rs`**
   - `generate_rooms(project_id, files)` — map directories to rooms
   - `generate_topics(room_id, symbols)` — group symbols into topics
   - `get_project_overview(name)` → structured project data
   - `get_room_detail(project, room)` → room data with topics and symbols
   - `get_topic_detail(project, room, topic)` → full topic content

4. **Modify `walker.rs`**
   - Add file content hashing (SHA-256 via `sha2` crate)
   - Return `DiscoveredFile` with hash
   - Add logic to diff against `file_index` table for incremental indexing

5. **Modify `main.rs`**
   - Restructure with clap subcommands: `index`, `list`, `show`, `export`
   - Legacy mode: `rexicon <dir>` detects no subcommand, runs index + export
   - `index` subcommand: walk → hash → extract changed → store in DB
   - `list` subcommand: query `projects` table
   - `show` subcommand: navigate hierarchy (1–3 args)
   - `export` subcommand: read from DB, format via existing `formatter.rs`

6. **Add `serde` + `serde_json` dependencies**
   - JSON output support for `--format json`
   - JSON fields in DB (tech_stack, tags, etc.)

7. **Config file**
   - Create `~/.rexicon/config.toml` on first run
   - Minimal config: database path, default output format

### Definition of Done
- `rexicon index .` stores everything in SQLite
- `rexicon list` shows indexed projects
- `rexicon show my-project` displays project overview with rooms
- `rexicon show my-project auth` displays room details with symbols
- `rexicon <dir>` still writes `rexicon.txt` (backward compat)
- Re-running `rexicon index .` only re-extracts changed files
- All existing tests still pass
- New tests for DB operations, hierarchy generation, incremental indexing

### New Dependencies
- `rusqlite = { version = "0.32", features = ["bundled"] }`
- `serde = { version = "1", features = ["derive"] }`
- `serde_json = "1"`
- `sha2 = "0.10"`
- `toml = "0.8"` (for config)
- `dirs = "5"` (for `~/.rexicon/` path)

### Estimated Effort
- 2–3 weeks for an experienced Rust developer
- ~2000 lines of new code, ~500 lines of modified code

---

## Phase 2: Memory System

**Goal:** Agent can write, read, update, delete, and search scoped memory entries. Memory is visible in hierarchy navigation.

### Tasks

1. **Create `memory.rs`**
   - CRUD operations on `memory` table
   - Topic validation (must match existing project)
   - Tag parsing and filtering
   - Search (LIKE-based keyword search for now, upgraded to FTS5/semantic in Phase 5)
   - Staleness checking: given a set of changed files, flag affected memory

2. **Add `memory` subcommand to CLI**
   - `memory add <scope> <title> <body> [--tags]`
   - `memory list [--project] [--topic] [--title] [--tags] [--stale-only]`
   - `memory get <id>`
   - `memory update <id> [--title] [--body] [--tags]`
   - `memory delete <id>`
   - `memory search <query> [--project]`
   - `memory compact --project <name> [--topic <name>]`

3. **Integrate memory into hierarchy navigation**
   - `rexicon show <project>` includes project-wide memory
   - `rexicon show <project> <room>` includes room memory + inherited project memory
   - `rexicon show <project> <room> <topic>` includes topic memory

4. **Integrate staleness into indexing**
   - After re-indexing changed files, flag memory entries whose scope references changed content
   - `rexicon diff <project>` reports stale memory

5. **Add `diff` subcommand**
   - Compare current file state against `file_index` hashes
   - Report changed, added, removed files
   - Report stale memory

6. **Memory export/import**
   - `rexicon export <project> --memory-only --format md` → markdown files
   - `rexicon import <project> <dir>` → read markdown files into memory table

### Definition of Done
- Agent can write a memory note scoped to any level
- Memory appears when navigating the hierarchy
- Re-indexing flags stale memory
- `rexicon diff` reports changes and stale notes
- Memory can be exported to markdown and imported back
- Tests for all memory operations, staleness detection, scope inheritance

### Estimated Effort
- 1–2 weeks
- ~1500 lines of new code

---

## Phase 3: Relationship Graph

**Goal:** Import-level and symbol-level relationships are extracted and queryable.

### Tasks

1. **Create `relationships.rs`**
   - Per-language import parsers using tree-sitter AST nodes
   - Path resolution logic (relative paths, crate paths, module paths)
   - Relationship storage (upsert into `relationships` table)
   - Query functions: imports, importers, callers, callees, dependency chain, impact analysis

2. **Add import extraction to the indexing pipeline**
   - After symbol extraction, run import analysis on each file
   - Resolve import paths to project-internal files
   - Store as `imports` relationships between symbols

3. **Add symbol-level call detection (best-effort)**
   - For each function body, scan for identifiers matching known symbols from imported files
   - Store as `calls` relationships with a confidence score

4. **Add `graph` subcommand to CLI**
   - `graph imports <project> --file <path>`
   - `graph importers <project> --file <path>`
   - `graph calls <project> --symbol <name>`
   - `graph callers <project> --symbol <name>`
   - `graph deps <project> --from <path> [--depth]`
   - `graph impact <project> --file <path>`
   - `graph export <project> --format dot|json`

5. **Integrate relationships into hierarchy navigation**
   - `rexicon show <project> <room> <topic>` includes relationship data
   - Room summaries include dependency information

6. **Incremental relationship updates**
   - On re-index, delete relationships for changed files, re-extract

### Language Priority
Start with the languages that have the clearest import syntax:
1. Rust (`use`, `mod`)
2. Python (`import`, `from`)
3. TypeScript/JavaScript (`import`, `require`)
4. Go (`import`)
5. Others added incrementally

### Definition of Done
- `rexicon graph imports my-api --file src/auth/jwt.rs` returns imported files
- `rexicon graph impact my-api --file src/database/schema.rs` shows affected files
- Relationships visible in `show` output
- DOT export renders a readable graph
- Tests for import parsing per language, resolution, graph queries

### Estimated Effort
- 2–3 weeks
- ~2500 lines of new code (import parsing is per-language)

---

## Phase 4: MCP Server

**Goal:** All rexicon capabilities exposed as MCP tools for native agent integration.

### Tasks

1. **Create `mcp.rs`**
   - MCP protocol handler (JSON-RPC 2.0 over stdio)
   - Tool registration (one handler per tool)
   - Input validation and error formatting
   - Evaluate MCP SDK options: `rmcp` crate vs. hand-rolled

2. **Implement all MCP tools**
   - Navigation: `list_projects`, `get_project`, `get_room`, `get_topic`
   - Search: `query`, `get_symbols`
   - Memory: `write_memory`, `update_memory`, `delete_memory`, `search_memory`
   - Graph: `get_imports`, `get_importers`, `get_dependencies`, `get_impact`
   - Indexing: `index`, `diff`

3. **Add `serve` subcommand**
   - `rexicon serve` (stdio mode, default)
   - `rexicon serve --transport tcp --port 3100` (TCP mode)
   - Add `tokio` runtime for async handling

4. **MCP resources**
   - `rexicon://projects` — project list
   - `rexicon://project/{name}/overview` — architecture summary
   - `rexicon://project/{name}/symbols` — full symbol tree

5. **Update the Claude skill (`SKILL.md`)**
   - Update to document MCP setup as the primary integration
   - Keep CLI fallback for environments without MCP

6. **Configuration documentation**
   - Claude Code `settings.json` configuration
   - Claude Desktop `claude_desktop_config.json` configuration

### Definition of Done
- `rexicon serve` starts and responds to MCP tool calls
- All navigation, search, memory, graph, and indexing tools work
- Claude Code can invoke rexicon tools natively
- Error handling follows MCP spec
- Tests for each tool handler (input validation, expected output)

### Estimated Effort
- 2–3 weeks
- ~2000 lines of new code

### Dependencies
- `tokio = { version = "1", features = ["full"] }` (async runtime)
- MCP crate TBD (evaluate `rmcp`, `mcp-server`, or custom JSON-RPC)

---

## Phase 5: RAG + Semantic Search

**Goal:** Semantic search across all content using local embeddings.

### Tasks

1. **Create `embeddings.rs`**
   - Embedding generation via `fastembed` (local model)
   - Chunking logic for large content
   - Vector storage in `embeddings` table
   - Cosine similarity search (brute force initially)
   - Optional: sqlite-vss integration for larger datasets

2. **Add embedding generation to indexing pipeline**
   - After storing content/symbols, generate embeddings for new/changed rows
   - Separate pass so it doesn't slow the core index
   - Configurable: `--no-embed` flag, `rag.enabled` config

3. **Upgrade search to hybrid (keyword + semantic)**
   - FTS5 for keyword search (add to schema)
   - Semantic search via embeddings
   - Reciprocal rank fusion to merge results
   - `search_mode` field in responses

4. **Configuration**
   - `config.toml` RAG section: enabled, provider, model, dimensions
   - Support for API-based embeddings (OpenAI, Voyage) as alternatives

5. **Embedding lifecycle**
   - Delete embeddings when content is deleted
   - Re-embed when content changes
   - Model migration: re-embed everything when model changes

### Definition of Done
- `rexicon query "how does auth work?"` returns semantically relevant results
- Local embedding model downloads and runs without configuration
- Embedding generation completes within 2x the indexing time
- Hybrid search outperforms keyword-only on natural language queries
- Tests for embedding generation, similarity search, hybrid ranking

### Estimated Effort
- 2 weeks
- ~1500 lines of new code

### Dependencies
- `fastembed = "4"` (local embeddings)

---

## Phase 6: Architecture Synthesis

**Goal:** Auto-generated project and room-level architecture summaries.

### Tasks

1. **Create `synthesis.rs`**
   - Tech stack detection from manifest files
   - Project type classification heuristic
   - Layer detection from room dependency graph
   - Entry point detection per language
   - Room purpose inference from symbol names and kinds

2. **Integrate into indexing pipeline**
   - Run after all symbols, rooms, and relationships are stored
   - Generate `projects.architecture` text
   - Generate `rooms.summary` text

3. **Update `show` output**
   - Project overview includes the architecture narrative
   - Room view includes auto-generated summary

### Definition of Done
- `rexicon show my-api` displays a useful architecture narrative
- Room summaries describe what each room does
- Tech stack correctly detected for Rust, Python, JS/TS, Go, Java projects
- Layer structure identified for layered architectures
- Tests for each detection heuristic

### Estimated Effort
- 1–2 weeks
- ~1000 lines of new code

---

## Phase Summary

| Phase | What ships | Effort | Cumulative |
|---|---|---|---|
| 1 — Foundation | SQLite store, incremental index, hierarchy, CLI restructure | 2–3 weeks | 2–3 weeks |
| 2 — Memory | Composable agent memory, staleness, export | 1–2 weeks | 4–5 weeks |
| 3 — Relationships | Import/call graph, impact analysis | 2–3 weeks | 7–8 weeks |
| 4 — MCP Server | Native agent tools | 2–3 weeks | 9–11 weeks |
| 5 — RAG | Semantic search, local embeddings | 2 weeks | 11–13 weeks |
| 6 — Synthesis | Architecture auto-summaries | 1–2 weeks | 12–15 weeks |

Total: ~12–15 weeks of focused development for the complete system.

## Risk Mitigation

| Risk | Mitigation |
|---|---|
| SQLite performance at scale | Benchmark at 100K files early in Phase 1. SQLite handles this, but index tuning may be needed. |
| Import resolution accuracy | Start with the simplest languages (Rust, Python). Accept 80% accuracy. Agent memory fills gaps. |
| MCP protocol complexity | Evaluate existing Rust MCP crates early. If none are mature, a hand-rolled JSON-RPC handler over stdio is straightforward. |
| Embedding model size / speed | `bge-small` is 33MB and fast. If too slow on older machines, make RAG optional (it already is). |
| Backward compatibility | Phase 1 explicitly preserves `rexicon <dir>` behavior. Run the existing test suite on every change. |
| Scope creep | Each phase has a clear "Definition of Done." Ship each phase before starting the next. |

## What's NOT in Scope

These are explicitly deferred beyond the 6 phases:

- **LSP integration** — future enhancement for higher-accuracy relationships
- **File watcher / daemon mode** — auto-reindex on file changes (use git hooks or manual for now)
- **Web UI** — all interaction is via CLI, MCP, or exported files
- **Cloud sync** — the database is local-only
- **Multi-user concurrent writes** — SQLite's write lock is sufficient for single-user + agent
- **Custom embedding model training** — use pre-trained models
