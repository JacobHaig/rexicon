# rexicon — Feature Roadmap

---

## Language filtering

Allow the user to scope symbol extraction to specific languages. Files for excluded languages still appear in the tree but have no symbols extracted beneath them.

```
rexicon . --lang rust --lang python
```

---

## Depth limit

Stop descending after `n` directory levels. Good for getting a high-level map of a large monorepo without going into every nested package.

```
rexicon . --depth 3
```

---

## Symbol kind filtering

Extract (or suppress) only specific symbol kinds. Makes it easy to produce a focused view — for example, the full public API surface without private helpers or impl blocks.

```
rexicon . --kinds fn,struct,trait
rexicon . --skip-kinds impl,variant
```

---

## JSON output format

Machine-readable output for programmatic tooling — editor integrations, CI checks, diff scripts.

```
rexicon . --format json
```

Sketch:
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

---

## Stats

Print a per-language summary table to stderr after writing the output file.

```
Language    Files   Symbols
rust            6      134
python          3       47
markdown        2        9
─────────────────────────
Total          11      190   (23 files in tree)
```

---

## Watch mode

Re-index automatically whenever a source file changes, keeping `rexicon.txt` current without manual re-runs. Uses OS filesystem events via the `notify` crate.

```
rexicon . --watch
```

---

## Config file

Project-level defaults in `.rexicon.toml` so flags don't need to be repeated on every run. CLI flags take precedence over config values.

```toml
exclude = ["vendor/**", "third_party/**"]
max_file_size = 524288
format = "txt"
```

---

## Max file size

Skip files above a byte threshold. Prevents accidentally parsing generated files, minified JS, or large vendored blobs that happen not to be gitignored.

```
rexicon . --max-file-size 512000
```

---

## Diff mode

Compare two rexicon index files and emit a structured diff — which symbols were added, removed, or had their signatures changed. Useful for code-review summaries or automated changelog generation.

```
rexicon diff rexicon_before.txt rexicon_after.txt
```

---

## Additional languages

All of the following have both a published `tree-sitter-*` crate and an active LSP server.

| Language | Extensions | Tree-sitter crate | LSP server |
|---|---|---|---|
| Ruby | `.rb` | `tree-sitter-ruby` | `ruby-lsp` |
| Swift | `.swift` | `tree-sitter-swift` | `sourcekit-lsp` |
| Kotlin | `.kt .kts` | `tree-sitter-kotlin` | `kotlin-language-server` |
| Lua | `.lua` | `tree-sitter-lua` | `lua-language-server` |
| PHP | `.php` | `tree-sitter-php` | `intelephense` |
| Haskell | `.hs .lhs` | `tree-sitter-haskell` | `haskell-language-server` |
| Elixir | `.ex .exs` | `tree-sitter-elixir` | `elixir-ls` |
| Erlang | `.erl .hrl` | `tree-sitter-erlang` | `erlang-ls` |
| Dart | `.dart` | `tree-sitter-dart` | `dart` (built-in) |
| Scala | `.scala .sbt` | `tree-sitter-scala` | `metals` |
| R | `.r .R` | `tree-sitter-r` | `languageserver` |
| TOML | `.toml` | `tree-sitter-toml` | `taplo` |
| YAML | `.yml .yaml` | `tree-sitter-yaml` | `yaml-language-server` |
| JSON | `.json` | `tree-sitter-json` | `vscode-json-languageserver` |
| SQL | `.sql` | `tree-sitter-sql` | `sqls` |
| HTML | `.html .htm` | `tree-sitter-html` | `vscode-html-languageserver` |
| CSS | `.css` | `tree-sitter-css` | `vscode-css-languageserver` |
| Vue | `.vue` | `tree-sitter-vue` | `@vue/language-server` |
| Svelte | `.svelte` | `tree-sitter-svelte-ng` | `svelte-language-server` |
| OCaml | `.ml .mli` | `tree-sitter-ocaml` | `ocaml-lsp-server` |

---

## LSP layer

A lightweight LSP server built on top of the existing extraction pipeline. Once the index exists, go-to-definition and hover can be served directly from the `FileIndex` symbol+line data without re-parsing on every request.
