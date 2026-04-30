# Rexicon v2 — Architecture Overview

## Vision

Rexicon evolves from a one-shot code indexer into a **local, agent-native project intelligence layer**. It combines static code analysis, a persistent knowledge store, composable agent-written memory, relationship tracking, and semantic search into a single binary that any AI agent can query.

The core premise: an agent working on a codebase should be able to **navigate a hierarchy** (Project → Room → Topic → Content) rather than grep/find its way through raw files. It should **accumulate knowledge** across sessions so that the 50th conversation about a project is dramatically more useful than the first. And it should work **cross-project** — patterns, decisions, and tribal knowledge are searchable across every codebase it has indexed.

## Design Principles

1. **Local-first** — everything runs on the developer's machine. SQLite database, no cloud dependency, no running server required for basic use. The MCP server mode is optional for real-time tool access.

2. **Agent-native** — outputs and interfaces are designed for programmatic consumption by AI agents, not for human reading (though humans can read everything too). Structured queries, typed responses, hierarchical navigation.

3. **Composable memory** — the agent writes what it learns. These notes are scoped to any level of the hierarchy (project-wide, room-scoped, topic-scoped) and persist across sessions. Memory is a first-class citizen alongside code-derived content.

4. **Self-updating** — incremental indexing via file hashing. Only changed files are re-processed. Stale memory (notes whose underlying code changed) is flagged automatically.

5. **Cross-project** — a single store holds every indexed project. Queries can span all projects or be scoped to one. "What authentication patterns do we use across all our projects?" is a valid query.

6. **Backward-compatible** — `rexicon <dir>` still works exactly as it does today, writing a `rexicon.txt` file. The new capabilities are additive.

## High-Level Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        Agent / Claude                        │
│                                                              │
│  list_projects  get_room  query  write_memory  diff  index   │
└────────┬──────────┬────────┬───────┬──────────┬──────┬───────┘
         │          │        │       │          │      │
    ┌────▼──────────▼────────▼───────▼──────────▼──────▼─────┐
    │                    MCP Server (optional)               │
    │              Structured tool interface                 │
    └────────────────────────┬───────────────────────────────┘
                             │
    ┌────────────────────────▼──────────────────────────────┐
    │                     CLI Interface                     │
    │  index | list | show | query | memory | diff | serve  │
    └───┬────────┬──────────┬───────────┬───────────┬───────┘
        │        │          │           │           │
   ┌────▼────┐ ┌──▼────┐ ┌──▼─────┐ ┌───▼───┐ ┌────▼─────┐
   │Indexing │ │Hierarc│ │ Search │ │Memory │ │Relationsh│
   │Pipeline │ │  hy   │ │ Engine │ │System │ │ip Graph  │
   └────┬────┘ └──┬────┘ └──┬─────┘ └───┬───┘ └────┬─────┘
        │         │         │           │           │
    ┌───▼─────────▼─────────▼───────────▼───────────▼──────┐
    │                  SQLite Database                     │
    │   projects | rooms | topics | content | memory |     │
    │   symbols | relationships | embeddings | metadata    │
    └──────────────────────────────────────────────────────┘
```

## The Hierarchy

Every piece of data in rexicon lives at a specific level:

```
Project   "my-api"              — a codebase, identified by name
  Room    "auth"                — a logical domain (auto: directory, manual: annotation)
    Topic "JWT refresh flow"    — a concept, symbol cluster, pattern, or issue
      Content                   — symbols, notes, relationships, examples
```

Navigation is always top-down. The agent asks "what projects exist?" before "what rooms does my-api have?" before "what's in the auth room?" This means the agent always has orientation before detail.

### Room auto-generation

Rooms are created automatically from the project's top-level directory structure during indexing. `src/auth/` becomes room `auth`. `src/database/migrations/` becomes room `database` with a nested room `migrations`. Flat source layouts (all files in `src/`) create rooms from filename prefixes or logical groupings.

Rooms can also be manually created or renamed by the agent via memory annotations.

## Access Models

Rexicon supports three access models, all reading/writing the same SQLite store:

| Model | How | Best for |
|---|---|---|
| **CLI** | `rexicon query "auth"` via bash | Humans, simple scripts, Claude skill |
| **MCP Server** | `mcp__rexicon__query` native tools | Claude Code/Desktop, real-time agent use |
| **File Export** | `rexicon export my-api` → `.rexicon/` directory | Team sharing, git-committable, code review |

## Crate Structure

```
src/
  lib.rs              ← re-exports all modules
  main.rs             ← CLI (clap subcommands)

  # Existing (modified)
  walker.rs           ← file discovery + hashing for incremental indexing
  registry.rs         ← language extension table (unchanged)
  symbol.rs           ← Symbol/FileIndex types (extended with DB IDs)
  treesitter.rs       ← tree-sitter extraction (unchanged core)
  formatter.rs        ← box-tree / plain formatting (for export)
  output.rs           ← file write (for export)

  # New
  db.rs               ← SQLite connection, migrations, connection pool
  schema.rs           ← table definitions, query builders, typed rows
  hierarchy.rs        ← Project → Room → Topic → Content CRUD + navigation
  relationships.rs    ← import parsing, dependency graph construction
  memory.rs           ← memory CRUD, scoping, staleness detection
  embeddings.rs       ← embedding generation, vector storage + search
  search.rs           ← unified query engine (keyword + graph + semantic)
  mcp.rs              ← MCP server implementation
  synthesis.rs        ← architecture summary auto-generation
```

## New Dependencies

| Crate | Purpose |
|---|---|
| `rusqlite` (bundled) | SQLite database |
| `serde` + `serde_json` | Serialization for MCP, config, JSON fields |
| `sha2` | File content hashing for incremental indexing |
| `tokio` | Async runtime for MCP server |
| `tower` / `axum` or `rmcp` | MCP protocol server (evaluate during Phase 4) |
| `fastembed` | Local embedding model (Phase 5) |
| `sqlite-vss` or `zerocopy` | Vector similarity search (Phase 5) |
