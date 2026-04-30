# Relationship Graph

## Purpose

The relationship graph captures how code connects across files and symbols. It answers questions the symbol index alone cannot:

- "What does this module depend on?"
- "What calls this function?"
- "What would break if I changed this struct?"
- "Show me the import chain from main.rs to this utility"

## Relationship Types

| Kind | Meaning | Example |
|---|---|---|
| `imports` | File A imports/uses file B | `use crate::auth::verify_token` |
| `calls` | Symbol A calls symbol B | `fn handler() { verify_token() }` |
| `implements` | Symbol A implements trait/interface B | `impl Handler for AuthHandler` |
| `extends` | Class A extends class B | `class Admin extends User` |
| `depends_on` | Generic dependency | Module-level inferred dependency |
| `contains` | Structural containment | `impl UserService` contains `fn create()` |

## Extraction Strategy

### Phase 1: Import-level relationships (file → file)

Parse import/use/require statements from the tree-sitter AST. Most grammars expose these as named nodes:

| Language | AST node type | Resolves to |
|---|---|---|
| Rust | `use_declaration` | Crate-relative path → file |
| Python | `import_statement`, `import_from_statement` | Module path → file |
| Go | `import_declaration` | Package path → directory |
| TypeScript/JS | `import_statement`, `call_expression` (require) | Relative path → file |
| Java | `import_declaration` | Package path → file |
| C/C++ | `preproc_include` | Header path → file |
| Ruby | `call` (require/require_relative) | String arg → file |
| PHP | `namespace_use_declaration` | Namespace → file |
| C# | `using_directive` | Namespace → files |

Resolution rules:
1. Relative paths (`./auth`, `../utils`) resolve directly.
2. Absolute/package paths resolve via convention (Go: directory = package, Python: module path = file path, Rust: `crate::` = project root).
3. External dependencies (third-party crates, npm packages) are noted but not followed — we only track project-internal relationships.
4. Unresolvable imports are stored with a `resolved: false` flag for later refinement.

### Phase 2: Symbol-level relationships (symbol → symbol)

This is harder and more heuristic-based:

**Calls:** Scan function bodies for identifiers that match known symbol names from imported files. This has false positives (name collisions) but provides useful signal.

**Implements/extends:** Tree-sitter exposes these directly in most languages:
- Rust: `impl_item` has a type and optional trait
- TypeScript/Java/C#: `class_declaration` has `extends`/`implements` clauses
- Python: `class_definition` has a base class list
- Go: struct embedding and interface satisfaction (partial)

**Contains:** Already captured by the existing nested symbol extraction. `impl Foo` contains `fn bar()`.

### Accuracy vs. Completeness

This is NOT a type checker or LSP. The relationship graph is:
- **Best-effort:** 80% accuracy is the target. False positives are acceptable if they're rare.
- **Conservative:** when ambiguous, store the relationship with a confidence flag rather than guessing.
- **Refinable:** the agent can correct relationships via memory annotations ("this import doesn't actually mean X depends on Y because it's feature-gated").

## Storage

```sql
-- File-level relationships
relationships(
    from_symbol  INTEGER REFERENCES symbols(id),
    to_symbol    INTEGER REFERENCES symbols(id),
    kind         TEXT,      -- 'imports', 'calls', 'implements', 'extends', 'depends_on'
    confidence   REAL,      -- 0.0 to 1.0, higher = more certain
    metadata     TEXT        -- JSON: {"import_path": "crate::auth", "resolved": true}
)
```

## Query Interface

### CLI

```bash
# What does src/auth/jwt.rs import?
rexicon graph imports my-api --file src/auth/jwt.rs

# What imports src/auth/jwt.rs? (reverse lookup)
rexicon graph importers my-api --file src/auth/jwt.rs

# What does UserService call?
rexicon graph calls my-api --symbol UserService

# Full dependency chain from main.rs
rexicon graph deps my-api --from src/main.rs --depth 3

# What would be affected if I change this file?
rexicon graph impact my-api --file src/database/schema.rs
```

### MCP Tools

```
rexicon_get_imports(project, file) → [{target_file, import_path}]
rexicon_get_importers(project, file) → [{source_file, import_path}]
rexicon_get_calls(project, symbol) → [{target_symbol, target_file}]
rexicon_get_callers(project, symbol) → [{source_symbol, source_file}]
rexicon_get_dependencies(project, from, depth?) → tree of dependencies
rexicon_get_impact(project, file_or_symbol) → [{affected, via}]
```

## Graph Visualization (Export)

```bash
rexicon graph export my-api --format dot > deps.dot
rexicon graph export my-api --format json > deps.json
```

The DOT output can be rendered with Graphviz. The JSON output is structured for programmatic consumption.

## Incremental Updates

When a file changes during re-indexing:
1. Delete all `relationships` where `from_symbol` or `to_symbol` references a symbol in that file.
2. Re-extract imports and calls for the changed file.
3. Re-create relationship rows.

Relationships pointing TO unchanged symbols from the changed file are recreated. Relationships FROM unchanged symbols TO changed symbols are preserved (the unchanged file's imports didn't change, even if the target symbol was renamed — staleness detection handles this).

## Future: LSP Integration

The tree-sitter heuristic graph is a pragmatic starting point. For higher accuracy, a future phase could optionally use a running LSP server:
- `rust-analyzer` for Rust
- `pyright` / `pylsp` for Python
- `gopls` for Go
- `tsserver` for TypeScript

The LSP can provide precise call graphs, type-resolved references, and goto-definition data. This would replace the heuristic graph with a ground-truth graph but requires the user to have the LSP installed and running. rexicon would query it opportunistically: if an LSP is available, use it; otherwise fall back to tree-sitter heuristics.
