---
name: rexicon
description: Index a codebase into a single symbol-tree file for LLM navigation. Use when the user asks for a codebase overview, project map, symbol tree, or structural summary — or when Claude needs to understand a large repo without reading every file.
argument-hint: [target-dir] [rexicon flags...]
---

# rexicon — codebase indexer

rexicon walks a project directory and emits a single text file (default
`rexicon.txt`) showing folder structure, every symbol's signature, and line
numbers as one unified box-drawing tree. Designed so an LLM can navigate a
repo without reading individual source files.

Supported languages: Rust, Python, Go, C, C++, JS, TS, C#, Java, Ruby, PHP,
Lua, Zig, Swift, Scala, Shell, Markdown.

## Bootstrap + run

The block below detects the host platform, downloads the pinned release
binary on first use (cached in the skill directory), then runs rexicon with
whatever arguments the user passed. The download is ~2 MB and only happens
once per platform per skill install.

```!
set -e

BIN_DIR="${CLAUDE_SKILL_DIR}/bin"
mkdir -p "$BIN_DIR"

# Detect platform → GitHub release artifact name.
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)                   ARTIFACT="rexicon-linux-x86_64";     BIN="$BIN_DIR/rexicon" ;;
  Darwin-x86_64)                  ARTIFACT="rexicon-macos-x86_64";     BIN="$BIN_DIR/rexicon" ;;
  Darwin-arm64)                   ARTIFACT="rexicon-macos-aarch64";    BIN="$BIN_DIR/rexicon" ;;
  MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64) \
                                  ARTIFACT="rexicon-windows-x86_64.exe"; BIN="$BIN_DIR/rexicon.exe" ;;
  *) echo "rexicon: unsupported platform $(uname -s)-$(uname -m)"; exit 1 ;;
esac

if [ ! -x "$BIN" ]; then
  URL="https://github.com/jacobhaig/rexicon/releases/latest/download/${ARTIFACT}"
  echo "rexicon: downloading ${ARTIFACT} from ${URL}"
  curl -fsSL "$URL" -o "$BIN"
  chmod +x "$BIN"
fi

# If the user passed no target, default to the current directory.
ARGS="$ARGUMENTS"
[ -z "$ARGS" ] && ARGS="."

"$BIN" $ARGS
```

After this block runs, rexicon indexes the project into a local SQLite
database and writes a `rexicon.txt` file (by default in the target directory,
or at the path passed with `--output`). The database persists across sessions,
enabling incremental re-indexing, memory, and relationship queries.

## MCP server (preferred for agent use)

The MCP server exposes rexicon's full capabilities as native tools. Configure
it in `~/.claude/settings.json` or the project `.claude/settings.json`:

```json
{
  "mcpServers": {
    "rexicon": {
      "command": "rexicon",
      "args": ["serve"],
      "env": {}
    }
  }
}
```

This makes all rexicon tools available as `mcp__rexicon__<tool_name>` in
Claude Code. Key tools: `list_projects`, `get_project`, `get_room`,
`get_topic`, `query`, `get_symbols`, `write_memory`, `search_memory`,
`index`, `diff`, `get_imports`, `get_impact`.

When the MCP server is running, prefer the structured tools over CLI
invocations — they return typed JSON and avoid text parsing.

## Reading the output

There are three ways to inspect what rexicon knows:

1. **`rexicon show`** — hierarchical navigation via CLI.
   - `rexicon show <project>` — project overview with rooms.
   - `rexicon show <project> <room>` — room details, topics, symbols, memory.
   - `rexicon show <project> <room> <topic>` — full topic content.

2. **`rexicon memory list`** — browse or search agent-written memory.
   - `rexicon memory list --project <name>` — topics with memory.
   - `rexicon memory list --project <name> --topic <topic>` — articles.
   - `rexicon memory search "<query>" --project <name>` — search memory.

3. **`rexicon.txt`** — the classic box-drawing tree file. Still written by
   default when indexing. The tree mirrors the directory structure; every
   file rexicon could parse is annotated with `[language]`. Under each file,
   symbols appear as children with `[start:end]` line ranges. Entries are
   sorted alphabetically and the output is deterministic across runs.

When the user asks about a specific file, class, or function, use
`rexicon show` or `rexicon query` first. Fall back to grepping
`rexicon.txt` if the database is not available. Jump to the source using
the line numbers rexicon reports only when you need the full body.

## Subcommand reference

| Subcommand | Purpose |
|---|---|
| `index <dir>` | Index or re-index a project into the database. |
| `list` | List all indexed projects. |
| `show <project> [room] [topic]` | Navigate the hierarchy: project, room, or topic. |
| `query <text>` | Search across symbols, memory, and content. |
| `memory add\|list\|get\|update\|delete\|search\|compact` | Manage agent-written memory. |
| `graph imports\|importers\|calls\|callers\|deps\|impact\|export` | Query the relationship graph. |
| `diff <project>` | Show what changed since last index (files + stale memory). |
| `export <project>` | Export project data to files (txt, json, md). |
| `serve` | Start the MCP server (stdio by default, or `--transport tcp`). |
| `config show\|set\|reset` | View or modify configuration. |

### Common flags (on `index`)

| Flag | Meaning |
|---|---|
| `<dir>` | Root to index. Defaults to `.` if omitted. |
| `--name <name>` | Project name (default: directory name). |
| `-o, --output <path>` | Output file (default: `<target>/rexicon.txt`). |
| `--no-ignore` | Include files normally excluded by `.gitignore`. |
| `--include <glob>` | Only index matching paths. Repeatable. |
| `--exclude <glob>` | Skip matching paths. Repeatable. Bare names like `vendor` expand to `{vendor,vendor/**}`. |
| `--format txt\|plain` | `txt` (default) = box-drawing tree. `plain` = flat, grep-friendly. |
| `--force` | Force full re-index (ignore file hashes). |

### Backward compatibility

Running `rexicon <dir>` without a subcommand still works as in v1 — it
indexes the project and writes `rexicon.txt`.

## Common invocations

- `/rexicon` — index the current directory, write `./rexicon.txt`.
- `/rexicon index . --name my-project` — index with an explicit project name.
- `/rexicon show my-project` — browse the project hierarchy.
- `/rexicon show my-project auth` — inspect the `auth` room's topics and symbols.
- `/rexicon query "authentication flow" --project my-project` — search across all content.
- `/rexicon memory list --project my-project --tags gotcha` — list gotchas.
- `/rexicon memory add --project my-project --topic conventions "Use snake_case for DB columns" "All database columns..." --tags convention` — record a team convention.
- `/rexicon diff my-project` — see what changed since last index.
- `/rexicon graph impact my-project --file src/auth/jwt.rs` — impact analysis.
- `/rexicon export my-project --format md --output docs/rexicon/` — export for team review.
- `/rexicon serve` — start the MCP server for native tool access.
- `/rexicon . --format plain --output /tmp/rexicon.tsv` — legacy flat output.
- `/rexicon . --exclude vendor --exclude '**/generated/**'` — skip third-party and generated code.
- `/rexicon . --no-ignore` — include `target/`, `node_modules/`, and other gitignored dirs.

## Troubleshooting

- **"unsupported platform"** — rexicon only ships binaries for Linux x86-64,
  macOS x86-64, macOS ARM, and Windows x86-64. On other platforms, build
  from source: `git clone https://github.com/jacobhaig/rexicon && cd rexicon && cargo install --path .`.
- **Download fails** — check network, or pin to a specific version by
  replacing `latest` in the URL with a tag like `v0.1.2`.
- **"0 files indexed"** — the walker respects `.gitignore`. If the target
  directory has no `.git` directory, the `ignore` crate won't honour a local
  `.gitignore` at all; files should still be found. If you see zero, verify
  the path actually contains source files in supported languages.
- **Binary refuses to run on macOS** — Gatekeeper may quarantine it. Run
  `xattr -d com.apple.quarantine "${CLAUDE_SKILL_DIR}/bin/rexicon"` once.
