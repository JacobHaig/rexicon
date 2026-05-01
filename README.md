# rexicon

A local, agent-native project intelligence layer. Indexes codebases into a persistent SQLite database with hierarchical navigation, a relationship graph, agent memory, and an MCP server -- giving AI agents (and humans) structured access to project knowledge without reading individual source files.

## Installation

```bash
git clone https://github.com/jacobhaig/rexicon
cd rexicon
cargo build --release
# Binary is at target/release/rexicon
```

## Quick start

```bash
# Index your project into the database
rexicon index .

# Browse what was indexed
rexicon show my-project

# Search symbols and memory
rexicon query "authentication"

# See what changed since last index
rexicon diff my-project

# Start the MCP server for Claude Code / Desktop
rexicon serve
```

## Usage

### Indexing

```bash
rexicon index <dir>                           # Index into ~/.rexicon/store.db
rexicon index <dir> --name my-api             # Custom project name
rexicon index <dir> --force                   # Full re-index (ignore file hashes)
rexicon index <dir> --include 'src/**'        # Only matching paths (repeatable)
rexicon index <dir> --exclude vendor          # Skip matching paths (repeatable)
rexicon index <dir> --no-ignore               # Include gitignored files
```

Re-indexing is incremental -- only files whose content hash changed are re-processed.

### Navigating the hierarchy

Rexicon organizes every project into **projects > rooms > symbols**. Rooms are auto-generated from the directory structure. Use `show` to drill down.

```bash
rexicon show                                  # List all indexed projects
rexicon show <project>                        # Rooms in a project
rexicon show <project> <room>                 # Files and symbols in a room
rexicon show <project> --format json          # JSON output
```

### Searching

```bash
rexicon query <text>                          # Search all projects
rexicon query <text> --project my-api         # Scope to one project
rexicon query <text> --kind symbol            # Filter: symbol or memory
```

### Memory

Persistent notes organized as **Project > Scope > Article**. Browse with a 4-level drill-down.

```bash
rexicon memory list                           # Projects with memory
rexicon memory list <project>                 # Scopes in a project
rexicon memory list <project> <scope>         # Articles in a scope
rexicon memory list <project> <scope> <title> # Full article

rexicon memory add -p my-api -s "auth" "Title" "Body text" --tags "gotcha,auth"
rexicon memory update <id> --body "new body"
rexicon memory delete <project> <scope> [article]
rexicon memory search "keyword" --project my-api
```

### Relationship graph

Auto-extracted imports, references, markdown links, and config paths across all supported languages.

```bash
rexicon graph children <project> <file>       # Direct dependencies (alias: c)
rexicon graph parents <project> <file>        # Reverse dependencies (alias: p)
rexicon graph tree <project> <file>           # Full dependency tree downward
rexicon graph impact <project> <file>         # Everything affected by a change
```

### Diff

```bash
rexicon diff <project>                        # Changed/added/removed since last index
```

### Export

```bash
rexicon export <project>                      # Box-drawing tree to rexicon.txt
rexicon export <project> --format plain       # Flat path:line format
rexicon export <project> --memory-only        # Memory as markdown files
rexicon export <project> --full               # Everything to .rexicon/ folder
```

`--full` dumps `project.json`, `rooms.json`, `symbols.json`, `symbols.txt`, `relationships.json`, and memory markdown files.

### Legacy mode

The original v1 command still works -- writes `rexicon.txt` and stores data in the database as a side effect.

```bash
rexicon <dir>                                 # Writes <dir>/rexicon.txt
rexicon <dir> --output /tmp/index.txt         # Custom output path
rexicon <dir> --format plain                  # Flat output
rexicon <dir> --include 'src/**' --exclude vendor --no-ignore
```

## Output format

The legacy and export commands produce a unified box-drawing tree. Symbols nest under their file, nested declarations nest under their container. Each symbol shows its full signature and line range.

```
# rexicon -- my-project

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

- **Bodies are always elided** -- `{ ... }` for blocks, `= ...` for value assignments.
- **Line numbers** shown as `[line]` (single-line) or `[start:end]` (multi-line).
- **Markdown headings** nest by level.
- **Deterministic output** -- parallel processing, sorted before writing.
- **The output file itself** is excluded from its own tree.

## MCP Server

Rexicon runs as an MCP server for native integration with Claude Code and Claude Desktop.

```bash
rexicon serve
```

Add `.mcp.json` to your project root:

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

This exposes 15 native `mcp__rexicon__*` tools with structured JSON responses:

| Tool | Description |
|---|---|
| `list_projects` | List all indexed projects |
| `get_project` | Project overview (rooms, memory summary) |
| `get_room` | Room detail (files, symbols) |
| `query` | Search symbols and memory |
| `index` | Index or re-index a project |
| `diff` | What changed since last index |
| `get_children` | Direct dependencies of a file |
| `get_parents` | What depends on a file |
| `get_tree` | Full dependency tree downward |
| `get_impact` | Everything affected if a file changes |
| `memory_list` | Browse memory (projects, scopes, articles) |
| `memory_write` | Add a memory entry |
| `memory_update` | Update an existing memory entry |
| `memory_delete` | Delete a memory entry |
| `memory_search` | Search memory by keyword |

See [docs/guide-users.md](docs/guide-users.md) for the full user guide.

## Releasing

To publish a new release and build binaries for all platforms:

```bash
git tag v0.2.0
git push origin v0.2.0
```

The release CI builds binaries for Linux x86-64, macOS x86-64, macOS ARM, and Windows x86-64, and uploads them to a GitHub Release.

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

Symbol extraction uses **tree-sitter** parse trees for every language except Markdown, which uses a lightweight ATX heading scanner. Relationship extraction detects imports, references, markdown links, and config file paths across all languages. Hidden files and anything matched by `.gitignore` are excluded by default.

## Data location

| Path | Contents |
|---|---|
| `~/.rexicon/store.db` | SQLite database (projects, symbols, rooms, relationships, memory) |

The database is a single file. Back it up, copy it between machines, or delete it to start fresh.

## Development

```bash
cargo build                        # debug build
cargo build --release              # release build
cargo test                         # run the full integration test suite
cargo clippy --all-targets -- -D warnings
cargo fmt
```

Tests cover every supported language plus regression cases for nested-symbol extraction. Add a `#[test]` in `tests/languages.rs` when adding a language.
