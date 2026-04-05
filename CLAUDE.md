# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Purpose

**rexicon** is a multi-language code indexer that walks a project directory and emits a single `rexicon.txt` file designed for LLM consumption. The file is a unified box-drawing tree showing folder structure, every symbol's signature, and line numbers — all in one document an LLM can navigate without reading source files.

Symbol extraction uses **tree-sitter** parse trees. No regex is used anywhere. LSP support is planned as a future layer on top.

## Commands

```bash
cargo build                                   # debug build
cargo build --release                         # release build
cargo run -- <target-dir>                     # index a project → writes rexicon.txt
cargo run -- <target-dir> --output <path>     # custom output path
cargo test                                    # run all tests
cargo test <test_name>                        # run a single test
cargo clippy -- -D warnings                   # lint
cargo fmt                                     # format
```

## Design philosophy

- **Flat module structure**: all source files live directly in `src/`. No submodules or nested `mod` directories.
- **Functional and pure**: each function has one discrete job and takes all it needs as arguments. Side effects only at the explicit I/O boundaries (`walk_all`, `walk`, `write_output`).
- **Data flows forward**: the pipeline is a series of transforms on plain data structures with no shared mutable state between stages.
- **Deterministic output**: files are processed in parallel (rayon) and results are sorted by path before formatting, so `rexicon.txt` is byte-for-byte identical across runs on the same input.

## Pipeline

```
walk_all(root)               → Vec<PathBuf>             all files, sorted (for tree structure)
walk(root, languages)        → Vec<SourceFile>          parseable files, sorted
  par_iter → extract(file)   → FileIndex                parallel via rayon, pure per-file
  collect + sort_by_path     → Vec<FileIndex>           deterministic
  format(all_files, indices) → String                   pure
  write_output(text, path)   → ()                       single I/O side-effect
```

`extract(file)` dispatches to:
- `extract_markdown` — ATX heading line scanner (no tree-sitter grammar needed)
- tree-sitter parse + per-language `LangRules` for all other languages

## Source layout

```
src/
  main.rs        ← CLI (clap), orchestration
  walker.rs      ← walk_all(), walk(), SourceFile type, .gitignore filtering
  registry.rs    ← Language type, built-in extension→language table
  symbol.rs      ← Symbol, SymbolKind, FileIndex — shared data types only
  treesitter.rs  ← tree-sitter extraction, per-language LangRules, markdown scanner
  formatter.rs   ← format() → String, builds and renders the unified tree
  output.rs      ← write_output(), single file-write function
```

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
| Zig | `.zig` | registry entry only — no grammar crate yet |
| Markdown | `.md .mdx` | ATX line scanner (no crate) |

## Core types

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

## Relevant crates

| Crate | Purpose |
|---|---|
| `rayon` | Data-parallel file processing |
| `ignore` | `.gitignore`-aware directory walk |
| `clap` | CLI argument parsing |
| `tree-sitter` + grammar crates | Symbol extraction via parse trees |
| `anyhow` | Error propagation |
