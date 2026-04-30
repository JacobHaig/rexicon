# Architecture Synthesis

## Purpose

Architecture synthesis auto-generates a high-level narrative description of a project's structure. This is the "describe a project without grep" piece — the agent reads the architecture summary and immediately understands the system's shape, layers, entry points, and key patterns.

## What Gets Generated

### Project-level architecture summary

A structured narrative covering:

1. **Tech stack** — languages, frameworks, key dependencies (detected from manifest files)
2. **Project type** — CLI tool, web server, library, monorepo, mobile app (heuristic classification)
3. **Layer structure** — how the rooms (directories) relate: handler → service → repository, or MVC, or flat
4. **Entry points** — main functions, handler registrations, exported modules
5. **Data flow** — inferred from the relationship graph: which rooms depend on which
6. **Key patterns** — singleton services, middleware chains, plugin systems (detected from symbol shapes)

### Room-level summaries

For each room:
1. **Purpose** — what this room is responsible for (inferred from symbol names and kinds)
2. **Key symbols** — the most important public symbols
3. **Dependencies** — which other rooms this room imports from
4. **Dependents** — which other rooms import from this room

## Detection Heuristics

### Tech Stack Detection

Read well-known manifest files:

| File | Detects |
|---|---|
| `Cargo.toml` | Rust, specific crates (actix, tokio, serde, etc.) |
| `package.json` | JavaScript/TypeScript, frameworks (React, Express, Next.js) |
| `go.mod` | Go, modules |
| `requirements.txt` / `pyproject.toml` / `setup.py` | Python, frameworks (Django, Flask, FastAPI) |
| `Gemfile` | Ruby, Rails |
| `pom.xml` / `build.gradle` | Java, Spring |
| `composer.json` | PHP, Laravel |
| `Package.swift` | Swift |
| `build.sbt` | Scala |
| `Dockerfile` / `docker-compose.yml` | Containerized deployment |
| `.github/workflows/` | CI/CD via GitHub Actions |

### Project Type Classification

Heuristic rules:

| Signal | Classification |
|---|---|
| Has `main()` + CLI arg parsing (clap, argparse, cobra) | CLI tool |
| Has HTTP handler setup (actix, express, gin, fastapi) | Web server |
| No `main()`, only `lib` exports | Library |
| Multiple `Cargo.toml` / `package.json` in subdirectories | Monorepo |
| Has `Podfile` / Xcode project / `AndroidManifest.xml` | Mobile app |
| Has `tests/` or test files but no src | Test suite |

### Layer Detection

Analyze the room dependency graph:

```
If rooms form a DAG (no cycles):
  → identify "top" rooms (no dependents within the project)
  → identify "bottom" rooms (no dependencies within the project)
  → layers = topological sort grouped by depth

If rooms have cycles:
  → identify strongly connected components
  → note the circular dependency
```

Common patterns to recognize:

| Pattern | Shape | Example |
|---|---|---|
| Layered | A → B → C, no reverse edges | handlers → services → repositories |
| MVC | controller ↔ model, view → model | Rails, Django |
| Hexagonal | core has no outward deps, adapters point inward | Clean architecture |
| Flat | everything at one level, few inter-room deps | Small projects, scripts |

### Entry Point Detection

Entry points are symbols that are "first in line" — called by the runtime or framework, not by other project code:

| Language | Entry point signal |
|---|---|
| Rust | `fn main()`, `#[tokio::main]`, `#[actix_web::main]` |
| Python | `if __name__ == "__main__"`, FastAPI/Flask route decorators |
| Go | `func main()`, `http.HandleFunc` |
| JavaScript/TS | `app.listen()`, `export default`, `module.exports` |
| Java | `public static void main`, `@SpringBootApplication` |

## Output Format

The architecture summary is stored as plain text in `projects.architecture`:

```
my-api — REST API

Tech: Rust (actix-web, sqlx, serde), PostgreSQL
Type: Web server
Entry: src/main.rs:61 fn main()

Layers:
  api/        → HTTP handlers, middleware, route definitions
  services/   → Business logic, orchestration
  db/         → Database queries, migrations, connection pool
  models/     → Shared data types, validation

Flow: api/ → services/ → db/, models/ used by all layers

Key patterns:
  - Middleware chain: auth → rate-limit → handler
  - Repository pattern: each DB table has a dedicated query module
  - Error types: custom AppError with From impls for all error sources
```

This is intentionally a text narrative, not JSON or a structured schema. The agent reads it as-is and understands the system. It's also human-readable for team members.

## Room Summary Generation

Room summaries are generated from their contained symbols:

```python
# Pseudocode
def summarize_room(room):
    symbols = get_symbols(room)
    kinds = Counter(s.kind for s in symbols)
    public = [s for s in symbols if "pub" in s.signature or is_exported(s)]

    summary = f"Contains {len(symbols)} symbols"
    if kinds["struct"] or kinds["class"]:
        summary += f", {kinds['struct'] + kinds['class']} types"
    if kinds["function"] or kinds["method"]:
        summary += f", {kinds['function'] + kinds['method']} functions"

    # Add purpose inference from symbol names
    name_tokens = extract_name_tokens(public)
    if overlaps(name_tokens, AUTH_TERMS):
        summary += ". Handles authentication/authorization"
    elif overlaps(name_tokens, DB_TERMS):
        summary += ". Database access layer"
    # etc.

    return summary
```

## Update Strategy

Architecture summaries are regenerated:
- On every full index (`rexicon index --force`)
- On re-index if room structure changed (new rooms, removed rooms)
- NOT on re-index if only file contents changed within existing rooms (wasteful)

The agent can override or enrich the auto-generated summary via memory:
```bash
rexicon memory add "my-api" "Architecture note" "The services/ layer also handles async job dispatch via a custom queue. This isn't obvious from the code structure." --tags architecture
```

## Limitations

- This is heuristic-based, not LLM-generated. It catches common patterns but may miss unusual architectures.
- The agent is expected to refine the summary over time via memory annotations.
- Cross-language projects (Rust backend + TypeScript frontend) are detected but the relationship between the two halves requires manual annotation.
