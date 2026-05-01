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

# Include files that .gitignore would normally exclude (e.g. target/, node_modules/)
rexicon /path/to/project --no-ignore

# Only index matching paths (repeatable)
rexicon /path/to/project --include 'src/**' --include 'lib/**'

# Skip matching paths (repeatable)
rexicon /path/to/project --exclude 'vendor/' --exclude '**/generated/**'

# Flat "path:line  signature" output instead of the box-drawing tree
rexicon /path/to/project --format plain
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

## MCP Server

Rexicon can run as an MCP server for native integration with Claude Code and Claude Desktop.

```bash
rexicon serve
```

Add `.mcp.json` to your project root to configure it:

```json
{
  "mcpServers": {
    "rexicon": {
      "command": "rexicon",
      "args": ["serve"]
    }
  }
}
```

This exposes all rexicon commands as native `mcp__rexicon__*` tools with structured JSON responses. See `docs/guide-users.md` for the full tool list.

## Releasing

To publish a new release and build binaries for all platforms:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release CI will trigger automatically on any `v*` tag, build binaries for Linux x86-64, macOS x86-64, macOS ARM, and Windows x86-64, and upload them to a GitHub Release.

## Supported languages

| Language | Extensions |
|---|---|
| Rust | `.rs` |
| Python | `.py`, `.pyi` |
| Go | `.go` |
| C | `.c`, `.h` |
| C++ | `.cpp`, `.cc`, `.cxx`, `.hpp` |
| JavaScript | `.js`, `.jsx`, `.mjs` |
| TypeScript | `.ts`, `.tsx`, `.mts` |
| C# | `.cs` |
| Java | `.java` |
| Ruby | `.rb`, `.rake` |
| PHP | `.php` |
| Lua | `.lua` |
| Zig | `.zig` |
| Swift | `.swift` |
| Scala | `.scala`, `.sc` |
| Shell | `.sh`, `.bash` |
| Markdown | `.md`, `.mdx` |

Symbol extraction uses **tree-sitter** parse trees for every language except
Markdown, which uses a lightweight ATX heading scanner. No regex is used
anywhere. Hidden files and anything matched by `.gitignore` are excluded by
default — pass `--no-ignore` to include them.

## Development

```bash
cargo build                        # debug build
cargo build --release              # release build
cargo test                         # run the full integration test suite (tests/languages.rs)
cargo clippy --all-targets -- -D warnings
cargo fmt
```

Tests cover every supported language plus regression cases for nested-symbol
extraction. Add a `#[test]` in `tests/languages.rs` when adding a language.
