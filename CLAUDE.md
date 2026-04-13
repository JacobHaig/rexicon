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
- **Functional and pure**: each function has one discrete job and takes all it needs as arguments. Side effects only at the explicit I/O boundaries (`walk`, `extract` for file read, `write_output`). The extraction core (`extract_from_bytes`) is pure from bytes to `FileIndex`.
- **Data flows forward**: the pipeline is a series of transforms on plain data structures with no shared mutable state between stages.
- **Deterministic output**: files are processed in parallel (rayon) and results are sorted by path before formatting, so `rexicon.txt` is byte-for-byte identical across runs on the same input.

## Pipeline

```
walk(root, languages, ...)           → (Vec<PathBuf>, Vec<SourceFile>)   single parallel pass
  par_iter → extract(file)           → FileIndex                         parallel via rayon, pure per-file
    = fs::read + extract_from_bytes  → FileIndex                         bytes-in, FileIndex-out core
  collect + sort_by_path             → Vec<FileIndex>                    deterministic
  format(all_files, indices)         → String                            pure
  write_output(text, path)           → ()                                single I/O side-effect
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
  lib.rs         ← re-exports the modules below so tests and main.rs share the same crate
  main.rs        ← CLI (clap), orchestration (thin wrapper over the library)
  walker.rs      ← walk(), SourceFile type, parallel .gitignore-aware walk, include/exclude filters
  registry.rs    ← Language type, built-in extension→language table
  symbol.rs      ← Symbol, SymbolKind, FileIndex — shared data types only
  treesitter.rs  ← tree-sitter extraction, per-language LangRules, markdown scanner
  formatter.rs   ← format() / format_plain() → String, renders the unified tree or flat form
  output.rs      ← write_output(), single file-write function

tests/
  languages.rs   ← integration tests; one per supported language plus regressions
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

## Relevant crates

| Crate | Purpose |
|---|---|
| `rayon` | Data-parallel file processing |
| `ignore` | `.gitignore`-aware directory walk |
| `clap` | CLI argument parsing |
| `tree-sitter` + grammar crates | Symbol extraction via parse trees |
| `anyhow` | Error propagation |
