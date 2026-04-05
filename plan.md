# rexicon — Feature Plan

Ideas for flags and features that would improve the program's utility and flexibility. Grouped roughly by effort and impact.

---

## Quick wins — flags with small scope

### `--quiet` / `-q`
Suppress the `wrote ...` stderr line. Useful when rexicon is invoked from scripts or editor integrations that don't want noise on stderr.

### `--no-line-numbers`
Omit the `[start:end]` tags from symbol output. Produces cleaner, more compact output for cases where line positions aren't needed.

### `--threads <n>`
Override the rayon thread pool size. Currently uses all available cores. Useful for CI environments where you want to cap resource usage, or for benchmarking.
```
rexicon . --threads 4
```


---

## Filtering — control what gets included

### `--lang <lang>` (repeatable)
Only index files of the specified language(s). All other files still appear in the tree but have no symbols extracted.
```
rexicon . --lang rust --lang python
```

### `--exclude <glob>` (repeatable)
Exclude additional path patterns on top of `.gitignore`. Useful for vendored directories that aren't gitignored, or for narrowing a large project.
```
rexicon . --exclude "vendor/**" --exclude "**/*.pb.go"
```

### `--include <glob>` (repeatable)
Only include files matching the given glob. The inverse of `--exclude` — useful for indexing a single subdirectory or file type without changing the root.
```
rexicon . --include "src/**"
```

### `--depth <n>`
Limit directory traversal to `n` levels deep. Useful for getting a high-level overview of a large monorepo without descending into every nested package.
```
rexicon . --depth 3
```

---

## Output formats — different consumers need different shapes

### `--format <txt|json>` (default: `txt`)
Emit output in an alternative format. The current box-drawing tree is ideal for LLM consumption but JSON would enable programmatic tooling (editors, CI checks, diff scripts).

JSON shape (sketch):
```json
{
  "project": "my-project",
  "files": [
    {
      "path": "src/main.rs",
      "language": "rust",
      "symbols": [
        { "kind": "Function", "signature": "fn main() -> Result<()> { ... }", "line_start": 5, "line_end": 32 }
      ]
    }
  ]
}
```

### `--format plain`
Flat list of `path:line  signature` entries, one per symbol. Easy to pipe into grep, fzf, or other text tools.
```
src/main.rs:5     fn main() -> Result<()>
src/walker.rs:14  pub fn walk(root, languages, exclude, no_ignore)
```

---

## Usability

### `--stats`
After writing the output file, print a summary table to stderr:
```
Language    Files   Symbols
rust            6      134
python          3       47
markdown        2        9
─────────────────────────
Total          11      190   (23 files in tree)
```

### `--watch` / `-w`
Re-run automatically whenever a source file changes, writing a fresh `rexicon.txt`. Uses OS file-system events (via the `notify` crate). Pairs well with an LLM workflow where you want the index to stay current while editing.


---

## Larger features


### LSP layer
Originally planned. Once the index exists, a lightweight LSP server could expose go-to-definition and hover using the symbol+line data already in `FileIndex`. Would sit on top of the existing extraction pipeline without changing it.
