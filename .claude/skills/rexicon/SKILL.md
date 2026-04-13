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

After this block runs, rexicon will have written an output file (by default
`rexicon.txt` in the target directory, or at the path passed with
`--output`). Read that file to inspect the project. The top of the file is a
single header comment, then a `rexicon — <project>` title, then the tree.

## Reading the output

- The tree mirrors the directory structure. Every file that rexicon could
  parse is annotated with `[language]`.
- Under each file, symbols appear as children — functions, structs, classes,
  impls, enum variants, markdown headings. Each symbol shows its full
  signature (bodies elided as `{ ... }` or `= ...`) and a `[start:end]` line
  range (or just `[line]` for single-line items).
- Entries are sorted alphabetically at every level and the output is
  deterministic across runs — safe to diff.

When the user asks about a specific file, class, or function, grep
`rexicon.txt` for it rather than re-reading the source. Jump to the source
using the line numbers rexicon reports only when you need the full body.

## Flag reference

| Flag | Meaning |
|---|---|
| `<target-dir>` | Root to index. Defaults to `.` if omitted. |
| `-o, --output <path>` | Output file (default: `<target>/rexicon.txt`). |
| `--no-ignore` | Include files normally excluded by `.gitignore` (`target/`, `node_modules/`, etc). |
| `--include <glob>` | Only index matching paths. Repeatable. |
| `--exclude <glob>` | Skip matching paths. Repeatable. Bare names like `vendor` expand to `{vendor,vendor/**}`. |
| `--format txt\|plain` | `txt` (default) = box-drawing tree. `plain` = one `path:line⇥signature` per symbol, grep-friendly. |

## Common invocations

- `/rexicon` — index the current directory, write `./rexicon.txt`.
- `/rexicon src/` — only index the `src/` subtree.
- `/rexicon . --format plain --output /tmp/rexicon.tsv` — flat output, suitable for piping into other tools.
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
