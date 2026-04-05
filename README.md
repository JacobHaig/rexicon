# rexicon

Indexes a codebase into a single `rexicon.txt` file designed for LLM consumption. The output is a unified box-drawing tree that shows folder structure, every symbol's full signature, and line numbers — giving a language model an accurate map of a project without reading individual source files.

## Installation

```bash
git clone https://github.com/jacobhaig/rexicon
cd rexicon
cargo build --release
# Binary is at target/release/rexicon
```

## Usage

```bash
# Index a project — writes rexicon.txt in the project root
rexicon /path/to/project

# Write to a custom location
rexicon /path/to/project --output /path/to/output.txt
```

## Output format

The entire project is one tree. Symbols nest under their file, nested declarations (methods, enum variants) nest under their container. Each symbol shows its full signature and line range.

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

- **Bodies are always elided** — `{ ... }` for blocks, `= ...` for value assignments (consts, type aliases).
- **Line numbers** are shown as `[line]` for single-line declarations and `[start:end]` for multi-line ones.
- **Markdown headings** are nested by level (`##` becomes a child of the preceding `#`).
- **Output is deterministic** — files processed in parallel, sorted by path before writing.
- **The output file itself** (`rexicon.txt`) is excluded from its own tree.

## Supported languages

| Language | Extensions |
|---|---|
| Rust | `.rs` |
| Python | `.py`, `.pyi` |
| Go | `.go` |
| C | `.c`, `.h` |
| C++ | `.cpp`, `.cc`, `.cxx`, `.hpp` |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` |
| TypeScript | `.ts`, `.tsx`, `.mts`, `.cts` |
| C# | `.cs` |
| Zig | `.zig` |
| Markdown | `.md`, `.mdx` |

Symbol extraction uses **tree-sitter** parse trees. No regex is used anywhere. Hidden files and anything matched by `.gitignore` are excluded.
