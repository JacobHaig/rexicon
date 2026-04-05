# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Purpose

**rexicon** is a multi-language code indexer that walks a project directory and emits a single `rexicon.txt` file. The file is designed to give large language models a complete, navigable picture of a codebase: where things live, what every symbol's signature looks like, and how files are organized — all in one unified tree.

Symbol extraction uses **LSP (Language Server Protocol)** servers as the primary source, with **tree-sitter** as the fallback. **No regex is used anywhere** in symbol extraction or parsing.

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
- **Functional and pure**: each function has one discrete job and takes all it needs as arguments. Avoid hidden state and side effects except at the explicit I/O boundaries (`walk`, `lsp_symbols`, `write_output`).
- **Data flows forward**: the pipeline is a series of transforms on plain data structures. No shared mutable state between pipeline stages.
- **Deterministic output**: files are processed in parallel (rayon) and results are sorted by path before formatting, so `rexicon.txt` is byte-for-byte identical across runs on the same input.

## Pipeline

```
walk(root)                   → Vec<SourceFile>          sorted by path
  par_iter → extract(file)   → FileIndex                parallel, pure per-file
  collect + sort_by_path     → Vec<FileIndex>           deterministic
  format(indices)            → String                   pure
  write_output(text, path)   → ()                       single I/O side-effect
```

`extract(file)` dispatches to:
- `lsp_symbols(file, server)` — JSON-RPC `textDocument/documentSymbol` call
- `ts_symbols(file, grammar)` — tree-sitter parse + `.scm` query (fallback)

LSP servers live outside rayon: one `tokio` async task per language, started lazily, results sent into the rayon workers via channels.

## Source layout

```
src/
  main.rs        ← CLI (clap), wires pipeline stages together
  walker.rs      ← walk(), SourceFile type, .gitignore filtering
  registry.rs    ← Language type, built-in extension→language table, languages.toml loading
  symbol.rs      ← Symbol, SymbolKind, FileIndex — shared data types only
  lsp.rs         ← JSON-RPC transport, LSP lifecycle, documentSymbol → Symbol mapping
  treesitter.rs  ← tree-sitter extractor, .scm query dispatch
  formatter.rs   ← format(indices) → String, pure
  output.rs      ← write_output(text, path), single file-write function
queries/         ← per-language tree-sitter .scm capture files (rust.scm, python.scm, …)
```

## Language support

| Language | Extensions | LSP server |
|---|---|---|
| Rust | `.rs` | `rust-analyzer` |
| Python | `.py` | `pylsp` / `pyright` |
| Go | `.go` | `gopls` |
| Zig | `.zig` | `zls` |
| C / C++ | `.c .h .cpp .hpp` | `clangd` |
| C# | `.cs` | `OmniSharp` |
| TypeScript / JS | `.ts .tsx .js .jsx` | `typescript-language-server --stdio` |
| Markdown | `.md` | tree-sitter-markdown (heading nodes only) |

An optional `languages.toml` in the target project root can override or extend this table.

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
    signature: String,       // full signature; bodies replaced with { ... }
    children: Vec<Symbol>,   // enum variants, impl methods, class members, etc.
}

enum SymbolKind {
    Function, Method, Struct, Enum, Trait,
    Interface, Class, Constant, TypeAlias, Module,
    // Markdown only:
    Heading { level: u8 },
}
```

## Expected output (`rexicon.txt`)

The entire project is one unified tree. Symbols are nested directly under their file entries, which are nested under directories. There are no separate "Folder Structure" and "File Sections" — it is all one structure.

```
# rexicon — my-project

my-project/
├── Cargo.toml
├── README.md                                    [markdown]
│   ├── # rexicon
│   ├── ## Installation
│   │   └── ### Prerequisites
│   ├── ## Usage
│   ├── ## Output Format
│   └── ## Supported Languages
├── src/
│   ├── formatter.rs                             [rust]
│   │   ├── fn format(indices: &[FileIndex]) -> String
│   │   └── fn render_node(node: &TreeNode, prefix: &str, is_last: bool) -> String
│   ├── lsp.rs                                   [rust]
│   │   ├── struct LspClient { ... }
│   │   ├── fn start_server(lang: &Language) -> Result<LspClient>
│   │   ├── fn initialize(client: &mut LspClient, root: &Url) -> Result<()>
│   │   ├── fn document_symbols(client: &mut LspClient, file: &SourceFile) -> Result<Vec<Symbol>>
│   │   └── fn shutdown(client: LspClient) -> Result<()>
│   ├── main.rs                                  [rust]
│   │   └── fn main()
│   ├── registry.rs                              [rust]
│   │   ├── struct Language { name: String, extensions: Vec<String>, lsp_command: Option<String>, lsp_args: Vec<String> }
│   │   ├── fn built_in_languages() -> Vec<Language>
│   │   └── fn load_languages(root: &Path) -> Vec<Language>
│   ├── symbol.rs                                [rust]
│   │   ├── enum SymbolKind { Function, Method, Struct, Enum, Trait, Interface, Class, Constant, TypeAlias, Module, Heading { level: u8 } }
│   │   ├── struct Symbol { kind: SymbolKind, signature: String, children: Vec<Symbol> }
│   │   └── struct FileIndex { rel_path: PathBuf, language: String, symbols: Vec<Symbol> }
│   ├── treesitter.rs                            [rust]
│   │   ├── fn ts_symbols(file: &SourceFile, grammar: Language) -> Result<Vec<Symbol>>
│   │   └── fn node_to_symbol(node: Node, src: &[u8]) -> Option<Symbol>
│   ├── output.rs                                [rust]
│   │   └── fn write_output(text: &str, path: &Path) -> Result<()>
│   └── walker.rs                                [rust]
│       ├── struct SourceFile { path: PathBuf, rel_path: PathBuf, language: Language }
│       └── fn walk(root: &Path, languages: &[Language]) -> Vec<SourceFile>
└── queries/
    ├── rust.scm
    ├── python.scm
    ├── go.scm
    └── ...
```

Key formatting rules:
- File entries carry `[language]` on the same line
- Every symbol is a direct child of its file entry using the same box-drawing prefix system
- `children` of a symbol (enum variants, impl methods) are indented one level further
- Bodies are always elided to `{ ... }` — signatures only
- Markdown files list headings as symbols, with heading hierarchy preserved as nesting: `##` headings are children of the preceding `#`, `###` are children of the preceding `##`, etc.
- The tree is sorted: directories before files (both alphabetically) at each level

## Relevant crates

| Crate | Purpose |
|---|---|
| `rayon` | Data-parallel file processing |
| `tokio` | Async runtime for LSP server I/O |
| `lsp-types` | LSP request/response types |
| `serde` / `serde_json` | JSON-RPC serialization |
| `walkdir` | Recursive directory traversal |
| `ignore` | `.gitignore`-aware filtering |
| `clap` | CLI argument parsing |
| `tree-sitter` + grammars | Fallback symbol extraction |
| `toml` | Optional `languages.toml` config |
