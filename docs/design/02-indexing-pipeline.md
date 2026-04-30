# Indexing Pipeline

## Overview

The indexing pipeline is the core of rexicon — it walks a project, extracts symbols, builds relationships, auto-generates the hierarchy, and stores everything in the database. It must be fast (parallel where possible), incremental (skip unchanged files), and deterministic.

## Pipeline Stages

```
rexicon index <dir> [--name <project-name>]
         │
         ▼
┌─────────────────────┐
│  1. Project Setup   │  Create or update project row in DB
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  2. File Discovery  │  Walk directory, respect .gitignore, apply filters
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  3. Hash + Diff     │  SHA-256 each file, compare against file_index table
└────────┬────────────┘  → produces changed_files and removed_files lists
         ▼
┌─────────────────────┐
│  4. Symbol Extract  │  tree-sitter parse changed files (parallel via rayon)
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  5. Relationship    │  Parse imports/use/require, resolve to project files
│     Analysis        │
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  6. Hierarchy       │  Map directories → rooms, group symbols → topics
│     Generation      │
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  7. Store + Update  │  Upsert all data into SQLite, flag stale memory
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  8. Embed (if RAG)  │  Generate embeddings for new/changed content
└────────┬────────────┘
         ▼
┌─────────────────────┐
│  9. Synthesize      │  Update architecture summary, room summaries
└─────────────────────┘
```

## Stage Details

### 1. Project Setup

- If `--name` is provided, use it. Otherwise derive from directory name.
- Look up existing project by name in DB. If found, this is a re-index. If not, create new.
- Read git HEAD commit hash (if git repo) and store as `head_commit`.

### 2. File Discovery

Uses the existing `walker.rs` module, enhanced:
- Respect `.gitignore` (existing behavior via `ignore` crate)
- Apply `--include` / `--exclude` glob filters (existing behavior)
- Apply `--no-ignore` flag (existing behavior)
- **New:** collect the full path + relative path for every file (not just language-matched ones)
- **New:** skip the `.rexicon/` directory if present in the project

Output: `Vec<DiscoveredFile>` where:
```rust
struct DiscoveredFile {
    abs_path: PathBuf,
    rel_path: PathBuf,
    language: Option<Language>,  // None for non-source files
}
```

### 3. Hash + Diff

For each discovered file:
- Compute SHA-256 of file contents
- Look up `file_index` row for this project + file path
- If hash matches → skip (file unchanged)
- If hash differs → mark as changed
- If file exists in `file_index` but not on disk → mark as removed

This is the key to incremental indexing. On a re-index of a 10,000-file project where 5 files changed, only those 5 are re-extracted.

```rust
struct IndexDiff {
    changed: Vec<DiscoveredFile>,   // new or modified files
    removed: Vec<PathBuf>,          // files deleted since last index
    unchanged: Vec<DiscoveredFile>, // files with matching hash
}
```

### 4. Symbol Extraction

Uses the existing `treesitter.rs` module, unchanged at its core:
- `extract_from_bytes(rel_path, lang_name, source)` → `FileIndex`
- Parallel via rayon over `changed` files only
- Returns `Vec<FileIndex>` with symbols, signatures, line numbers

No changes needed to the extraction logic. The `FileIndex` is then converted to DB rows.

### 5. Relationship Analysis

**New module: `relationships.rs`**

For each extracted file, parse import/use/require statements to discover dependencies:

| Language | Import syntax | Example |
|---|---|---|
| Rust | `use crate::`, `mod`, `use super::` | `use crate::auth::verify_token` |
| Python | `import`, `from X import` | `from auth.jwt import refresh` |
| Go | `import "path"` | `import "myapp/auth"` |
| TypeScript/JS | `import`, `require` | `import { Auth } from './auth'` |
| Java | `import` | `import com.myapp.auth.JwtService` |
| C/C++ | `#include` | `#include "auth/jwt.h"` |
| Ruby | `require`, `require_relative` | `require_relative 'auth/jwt'` |
| PHP | `use`, `require`, `include` | `use App\Auth\JwtService` |

Resolution strategy:
1. Parse the import statement from the tree-sitter AST (most grammars expose import nodes).
2. Attempt to resolve the path to a file within the project.
3. If resolved, create a `relationships` row: file A → imports → file B.
4. For function-level calls, this is best-effort — if a symbol name matches a known symbol in an imported file, create a `calls` relationship.

This is not a full type system or LSP — it's a heuristic graph that captures the obvious connections. 80% accuracy is fine; the agent can verify ambiguous edges.

### 6. Hierarchy Generation

**New module: `hierarchy.rs`**

Auto-generate rooms and topics from the project structure:

**Rooms from directories:**
```
src/
  auth/         → room "auth"
  database/     → room "database"
  api/
    handlers/   → room "api" with child room "handlers"
    middleware/  → room "api" with child room "middleware"
```

Rules:
- Each top-level directory under the project root (or `src/` if present) becomes a room.
- Nested directories become child rooms up to 2 levels deep.
- Files in the root (not in any directory) go into a room called `_root`.
- Room names are the directory names, lowercased.

**Topics from symbols:**
- Each file becomes a topic within its room.
- Significant symbols (public structs, classes, traits, interfaces) also become topics.
- Topics are named after their primary symbol or file name.

**Room summaries:**
- Auto-generated from the symbols contained: "Contains 3 structs, 12 functions, handles JWT validation and token refresh."
- Updated on re-index.

### 7. Store + Update

Within a single SQLite transaction:
1. Delete `symbols` and `content` rows for changed and removed files.
2. Insert new `symbols` and `content` rows for changed files.
3. Update `file_index` hashes for changed files; delete rows for removed files.
4. Upsert `rooms` and `topics` (merge with any manually-created ones).
5. Upsert `relationships`.
6. Flag `memory` rows as stale for the project.
7. Update `projects.last_indexed`, `projects.head_commit`.

### 8. Embedding Generation (Phase 5)

If RAG is enabled:
- For each new or changed content/symbol row, generate an embedding.
- Chunk large content into embedding-sized pieces (512 tokens typical).
- Use the configured embedding model (default: local via `fastembed`).
- Store in `embeddings` table.

### 9. Architecture Synthesis (Phase 6)

After all data is stored, regenerate the project's architecture summary:
- List top-level rooms and their purposes.
- Identify entry points (main functions, handler registrations).
- Detect tech stack from languages and dependency files (Cargo.toml, package.json, go.mod).
- Summarize the dependency graph (which rooms depend on which).

This is a structured template, not LLM-generated. The agent can enrich it later via memory.

## Incremental Indexing Performance

| Project size | First index | Re-index (5 files changed) |
|---|---|---|
| 100 files | ~1s | ~0.1s |
| 1,000 files | ~5s | ~0.2s |
| 10,000 files | ~30s | ~0.5s |
| 100,000 files | ~5min | ~1s |

The hash comparison is O(n) in file count but very fast (just read + hash, no parsing). Only changed files go through tree-sitter extraction.

## Backward Compatibility

`rexicon <dir>` (no subcommand) continues to work as today:
1. Index the project into the DB.
2. Export the symbol tree as `rexicon.txt` (or custom path with `--output`).

This is equivalent to `rexicon index <dir> && rexicon export <project> --format txt`.
