# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Purpose

**rexicon** is a local, agent-native project intelligence layer. It indexes a codebase into a SQLite database that stores symbols, file relationships (import graphs), a directory-derived room/topic hierarchy, and persistent agent memory — all queryable from the CLI, an MCP server, or exported as files.

The original v1 behavior (walk a directory, emit a `rexicon.txt` box-drawing tree) is preserved as both a legacy CLI path and the `export` subcommand.

Symbol extraction uses **tree-sitter** parse trees. Relationship extraction (imports, references) uses line-level parsers for 14 languages. No regex is used for symbol extraction. The MCP server exposes every CLI command as a JSON-RPC tool over stdio.

## Commands

```bash
# Build & test
cargo build                                   # debug build
cargo build --release                         # release build
cargo test                                    # run all tests
cargo test <test_name>                        # run a single test
cargo clippy -- -D warnings                   # lint
cargo fmt                                     # format

# Legacy mode (v1 — writes rexicon.txt)
cargo run -- <target-dir>                     # index a project → writes rexicon.txt
cargo run -- <target-dir> --output <path>     # custom output path

# Database-backed commands (v2)
cargo run -- index <dir>                      # index project into SQLite (incremental)
cargo run -- index <dir> --force              # full re-index ignoring hashes
cargo run -- list                             # list all indexed projects
cargo run -- show <project>                   # project overview (rooms, topics)
cargo run -- show <project> <room>            # room detail (files, symbols)
cargo run -- query <text> --project <p>       # search symbols and memory
cargo run -- diff <project>                   # changed/added/removed since last index
cargo run -- export <project>                 # export rexicon.txt from DB
cargo run -- export <project> --full          # full export to .rexicon/ folder
cargo run -- export <project> --memory-only   # export memory as markdown

# Relationship graph
cargo run -- graph children <project> <file>  # direct dependencies of a file
cargo run -- graph parents <project> <file>   # what depends on this file
cargo run -- graph tree <project> <file>      # full dependency tree downward
cargo run -- graph impact <project> <file>    # reverse tree (change impact)

# Agent memory
cargo run -- memory list [project] [scope] [article]  # browse memory hierarchy
cargo run -- memory add -p <proj> -s <scope> <title> <body>
cargo run -- memory update <id> --title ... --body ...
cargo run -- memory delete <project> <scope> [article]
cargo run -- memory search <query>

# MCP server
cargo run -- serve                            # start MCP server over stdio
```

## Design philosophy

- **Flat module structure**: all source files live directly in `src/`. No submodules or nested `mod` directories.
- **Functional and pure**: each function has one discrete job and takes all it needs as arguments. Side effects only at the explicit I/O boundaries (`walk`, `extract` for file read, `write_output`). The extraction core (`extract_from_bytes`) is pure from bytes to `FileIndex`.
- **Data flows forward**: the pipeline is a series of transforms on plain data structures with no shared mutable state between stages.
- **Deterministic output**: files are processed in parallel (rayon) and results are sorted by path before formatting, so `rexicon.txt` is byte-for-byte identical across runs on the same input.

## Pipeline

### Legacy / export pipeline (v1)

```
walk(root, languages, ...)           → (Vec<PathBuf>, Vec<SourceFile>)   single parallel pass
  par_iter → extract(file)           → FileIndex                         parallel via rayon, pure per-file
    = fs::read + extract_from_bytes  → FileIndex                         bytes-in, FileIndex-out core
  collect + sort_by_path             → Vec<FileIndex>                    deterministic
  format(all_files, indices)         → String                            pure
  write_output(text, path)           → ()                                single I/O side-effect
```

### Index pipeline (v2)

```
walk(root, languages, ...)           → (Vec<PathBuf>, Vec<SourceFile>)   single parallel pass
  hash_file(path)                    → SHA-256 per file                  incremental: skip unchanged
  par_iter → extract(file)           → FileIndex                         only changed files
  hierarchy::generate_rooms(conn)    → rooms from directory structure
  hierarchy::store_symbols(conn, fi) → symbols into SQLite
  hierarchy::generate_topics(conn)   → topics from file grouping
  relationships::index_relationships → import/ref graph into SQLite      14-language parser
  schema::flag_stale_memory(conn)    → mark memory articles when code changes
```

`walk()` returns both the full file list (for the tree structure) and the
language-matched subset (for extraction) in one pass.

`extract_from_bytes(rel_path, lang_name, source)` is the pure core — unit-testable
without filesystem access — and dispatches to:
- ATX heading line scanner for `markdown` (no tree-sitter grammar needed).
- tree-sitter parse + per-language `LangRules` for everything else.

`extract(file)` is a thin wrapper that reads the file and forwards to
`extract_from_bytes`.

## Source layout

```
src/
  lib.rs            ← re-exports all modules so tests and main.rs share the same crate
  main.rs           ← CLI (clap subcommands), orchestration
  walker.rs         ← walk(), SourceFile, parallel .gitignore-aware walk, file hashing (SHA-256)
  registry.rs       ← Language type, extension→language table
  symbol.rs         ← Symbol, SymbolKind, FileIndex — shared data types
  treesitter.rs     ← tree-sitter extraction, per-language LangRules, markdown scanner
  formatter.rs      ← format()/format_plain() → String, box-tree rendering
  output.rs         ← write_output(), single file-write function
  db.rs             ← SQLite connection, schema migrations, WAL mode
  schema.rs         ← all DB types and CRUD (Project, Room, Topic, DbSymbol, Memory, MemoryScope, Relationship)
  hierarchy.rs      ← room/topic generation from directories, symbol storage
  relationships.rs  ← import/reference parsing for 14 languages, path resolution, graph traversal
  mcp.rs            ← MCP server (JSON-RPC over stdio, 15 tools)

tests/
  languages.rs      ← integration tests; one per supported language plus regressions
```

The crate is both a binary and a library. `src/lib.rs` just re-exports the
modules above. `main.rs` consumes them via `use rexicon::{...}`, and the
integration tests in `tests/languages.rs` do the same. This is why the extraction
core is exposed as `pub fn extract_from_bytes(rel_path, lang_name, source)` in
`treesitter.rs` — tests drive it directly without touching the filesystem.

## Language support

| Language | Extensions | Tree-sitter crate |
|---|---|---|
| Rust | `.rs` | `tree-sitter-rust` |
| Python | `.py .pyi` | `tree-sitter-python` |
| Go | `.go` | `tree-sitter-go` |
| C | `.c .h` | `tree-sitter-c` |
| C++ | `.cpp .cc .cxx .hpp` | `tree-sitter-c` (same grammar) |
| JavaScript | `.js .jsx .mjs` | `tree-sitter-javascript` |
| TypeScript | `.ts .tsx .mts` | `tree-sitter-typescript` |
| C# | `.cs` | `tree-sitter-c-sharp` |
| Java | `.java` | `tree-sitter-java` |
| Shell | `.sh .bash` | `tree-sitter-bash` |
| Ruby | `.rb .rake` | `tree-sitter-ruby` |
| PHP | `.php` | `tree-sitter-php` |
| Lua | `.lua` | `tree-sitter-lua` |
| Zig | `.zig` | `tree-sitter-zig` |
| Swift | `.swift` | `tree-sitter-swift` |
| Scala | `.scala .sc` | `tree-sitter-scala` |
| Markdown | `.md .mdx` | ATX line scanner (no crate) |

## Core types

### Extraction types (symbol.rs, walker.rs)

```rust
struct SourceFile {
    path: PathBuf,       // absolute
    rel_path: PathBuf,   // relative to project root
    language: Language,
}

struct FileIndex {
    rel_path: PathBuf,
    language: String,
    symbols: Vec<Symbol>,
}

struct Symbol {
    kind: SymbolKind,
    signature: String,   // full declaration; bodies replaced with { ... } or = ...
    line_start: u32,     // 1-indexed
    line_end: u32,
    children: Vec<Symbol>,
}

enum SymbolKind {
    Function, Method, Struct, Enum, Trait, Interface, Class,
    Constant, TypeAlias, Module, Impl, Variant, Macro,
    Heading(u8),  // markdown only; u8 = heading level 1–6
}
```

### Database types (schema.rs)

```rust
struct Project {
    id: i64,
    name: String,
    root_path: String,
    tech_stack: Option<Vec<String>>,
    architecture: Option<String>,
    entry_points: Option<Vec<String>>,
    head_commit: Option<String>,
    last_indexed: String,
    created_at: String,
    updated_at: String,
}

struct Room {
    id: i64,
    project_id: i64,
    name: String,
    path: Option<String>,
    summary: Option<String>,
    parent_room_id: Option<i64>,
}

struct Topic {
    id: i64,
    room_id: i64,
    name: String,
    kind: String,          // "file", "group", etc.
    summary: Option<String>,
}

struct DbSymbol {
    id: i64,
    project_id: i64,
    content_id: Option<i64>,
    kind: String,
    signature: String,
    name: String,
    file_path: String,
    line_start: i64,
    line_end: i64,
    parent_symbol_id: Option<i64>,
}

struct MemoryScope {
    id: i64,
    project_id: i64,
    name: String,
}

struct Memory {
    id: i64,
    scope_id: i64,
    title: String,
    body: String,
    tags: Option<Vec<String>>,
    author: String,
    stale: bool,           // flagged when code changes after memory was written
    created_at: String,
    updated_at: String,
}

struct Relationship {
    id: i64,
    project_id: i64,
    source_file: String,
    target: String,        // raw import path
    target_file: Option<String>,  // resolved file path
    kind: String,          // "import", "reference", "config_path"
    source_line: Option<i64>,
    metadata: Option<String>,
}
```

## Extraction rules (`LangRules`)

Each language is described by a `LangRules` value with three static slices:

- **`top_level`** — `(node_kind, SymbolKind)` pairs matched against direct children of the parse tree root.
- **`nested`** — `(container_kind, child_kind, SymbolKind)` triples for symbols found inside containers (impl methods, enum variants, class members, etc.). The search descends the subtree of the container, stopping when a match is found.
- **`body_kinds`** — node kinds that mark the start of a body block; the signature is truncated here and replaced with `{ ... }`.
- **`value_kinds`** — node kinds that carry a tree-sitter `"value"` named field (const/static/type-alias declarations); the value is replaced with `= ...`.

## Expected output (`rexicon.txt`)

One unified box-drawing tree. Symbols are children of their file node; nested symbols (methods, variants) are children of their container symbol. Every symbol has `[start:end]` line range (or just `[line]` if single-line).

```
# rexicon — my-project

my-project/
├── Cargo.toml
├── README.md  [markdown]
│   ├── # my-project  [1]
│   ├── ## Installation  [5]
│   │   └── ### Prerequisites  [7]
│   └── ## Usage  [12]
└── src/
    ├── main.rs  [rust]
    │   └── fn main() -> Result<()> { ... }  [5:32]
    └── lib.rs  [rust]
        ├── pub struct Config { ... }  [3:8]
        ├── pub enum Error { ... }  [10:14]
        │   ├── Io(std::io::Error)  [11]
        │   └── Parse(String)  [12]
        └── impl Config { ... }  [16:40]
            ├── pub fn new(path: &Path) -> Result<Self> { ... }  [17:25]
            └── pub fn validate(&self) -> bool { ... }  [27:39]
```

Key rules:
- All entries sorted alphabetically at every level.
- `[language]` tag on the same line as the file name.
- Bodies always elided: `{ ... }` for blocks, `= ...` for value assignments.
- Markdown headings nest by level (`##` is a child of the preceding `#`, etc.).
- The output file itself (`rexicon.txt`) is excluded from its own tree.

## Testing

Tests live in `tests/languages.rs` (integration tests against the library crate).
There is one `#[test]` per supported language — each fixture is an inline source
snippet that exercises the language's top-level declarations plus at least one
nested case, and asserts on the symbol kinds and signatures returned by
`extract_from_bytes`. Additional regression tests cover:

- `rust_impl_methods_nested` — nested symbols must retain their own children
  (regression for the `collect_nested` recursion fix).
- `ruby_extract` — the bare `module` keyword token must not leak in as a spurious
  nested symbol (regression for the `is_named()` filter fix in `find_in_subtree`).
- `unknown_language_errors` — `extract_from_bytes` returns `Err` for unknown
  language names.
- `line_numbers_one_indexed` — line numbers are 1-indexed.

Run with `cargo test`. Add a new test alongside a new language.

### Extraction architecture notes

- `find_in_subtree` filters to named nodes only (`child.is_named()`). Some
  grammars (notably tree-sitter-ruby) expose keyword tokens with the same
  `kind()` as their parent node kind — e.g. the `module` keyword inside a
  `module` declaration — and those tokens must not match `nested` rules.
- `find_in_subtree` recurses into found symbols via `collect_nested` so
  multi-level nesting (e.g. Scala `class` → inner `class` → `def`, Ruby
  `module` → `class` → `method`) is preserved.

### Known gap

`export class Foo {}` in JavaScript/TypeScript is matched as a top-level
`export_statement` and currently doesn't recurse into the class's method
children. The JS test uses the plain `class` form until this is fixed.

### End-to-end smoke tests (`cargo run`)

`cargo test` only drives the extraction core. The walker, globbing, output
path logic, and CLI wiring are exercised by running the binary against a
throwaway fixture. After any change to `main.rs`, `walker.rs`, `formatter.rs`,
or `output.rs`, run the smoke checks below. They take under a minute.

Set up a fixture with `.gitignore`, multiple languages, and an ignored
directory:

```bash
mkdir -p /c/tmp/rexicon-smoke/{src,vendor,target,docs}
cat > /c/tmp/rexicon-smoke/.gitignore <<'EOF'
target/
EOF
# add a .rs, .py, .go, .md file under src/, vendor/, target/, docs/ …
(cd /c/tmp/rexicon-smoke && git init -q && git add . \
    && git -c user.email=t@t -c user.name=t commit -q -m init)
```

The `git init` step matters: the `ignore` crate only honours a local
`.gitignore` when there's a `.git` directory present.

Flag matrix to cover:

| # | Command | What to verify |
|---|---------|----------------|
| 1 | `cargo run -- <dir>` | default box tree, `target/` excluded by `.gitignore`, `<dir>/rexicon.txt` written |
| 2 | `cargo run -- <dir> --output <path>` | output lands at the custom path |
| 3 | `cargo run -- <dir> --no-ignore` | `target/` contents re-appear |
| 4 | `cargo run -- <dir> --include 'src/**'` | only `src/` paths in the tree |
| 5 | `cargo run -- <dir> --exclude 'vendor'` | prefix-style exclude removes `vendor/` entirely |
| 6 | `cargo run -- <dir> --format plain` | flat `path:line\tsignature` lines, no box-drawing |
| 7 | Combined `--include 'src/**' --exclude '**/*.py' --format plain` | filters compose; `.py` files gone |
| 8 | Run default twice, `diff` the outputs | byte-identical (determinism invariant) |
| 9 | Default run, then `grep -c rexicon.txt <output>` | `0` — output file must not list itself |
| 10 | `cargo run -- --help` | all flags present with descriptions |

Every command should exit `0` and print a `wrote <path> (N files indexed,
M total)` line to stderr. If any expected file is missing from the tree,
or appears when it shouldn't, that's the bug to chase.

## MCP Server Parity Rule

The MCP server (`src/mcp.rs`) must expose every CLI command as a tool. There is a strict 1:1 mapping between CLI commands and MCP tools. When you add, remove, or change a CLI command, you must update the MCP server to match.

Current mapping:

| CLI command | MCP tool |
|---|---|
| `rexicon list` | `list_projects` |
| `rexicon show <project>` | `get_project` |
| `rexicon show <project> <room>` | `get_room` |
| `rexicon query` | `query` |
| `rexicon index` | `index` |
| `rexicon diff` | `diff` |
| `rexicon graph children` | `get_children` |
| `rexicon graph parents` | `get_parents` |
| `rexicon graph tree` | `get_tree` |
| `rexicon graph impact` | `get_impact` |
| `rexicon memory list` | `memory_list` |
| `rexicon memory add` | `memory_write` |
| `rexicon memory update` | `memory_update` |
| `rexicon memory delete` | `memory_delete` |
| `rexicon memory search` | `memory_search` |

After any change to CLI commands in `main.rs`, verify:
1. The corresponding MCP tool in `mcp.rs` has matching parameters and behavior
2. The `tool_definitions()` function lists the tool with correct input schema
3. The `handle_tools_call()` dispatch includes the tool name

## Relevant crates

| Crate | Purpose |
|---|---|
| `rayon` | Data-parallel file processing |
| `ignore` | `.gitignore`-aware directory walk |
| `clap` | CLI argument parsing (derive mode) |
| `tree-sitter` + grammar crates | Symbol extraction via parse trees |
| `anyhow` | Error propagation |
| `rusqlite` (bundled) | SQLite database (WAL mode) |
| `serde` / `serde_json` | JSON serialization for DB fields, MCP protocol, and export |
| `sha2` | SHA-256 file hashing for incremental indexing |
| `chrono` | Timestamps for DB records |
| `dirs` | Platform-appropriate data directory for the database |
| `toml` | Config file parsing |
| `globset` | Include/exclude glob pattern matching |
