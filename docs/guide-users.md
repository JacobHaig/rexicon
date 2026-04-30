# Rexicon User Guide

Rexicon indexes your codebases into a local SQLite database so you (and AI agents) can navigate, search, and annotate them without reading individual source files.

All data is stored in a single file at `~/.rexicon/store.db`. No cloud, no server, no account.

---

## Quick Start

```bash
# Index your project
rexicon index .

# See what's indexed
rexicon list

# Browse the project
rexicon show my-project

# Search for something
rexicon query "authentication"
```

---

## Core Concepts

### The Hierarchy

Rexicon organizes every indexed project into a 4-level hierarchy:

```
Project    "my-project"           ← a codebase you've indexed
  Room     "auth"             ← a logical area (auto-generated from directories)
    Topic  "JwtService"       ← a file, struct, class, or concept
      Content                 ← symbols, signatures
```

You navigate top-down: project → room → topic.

### Rooms

Rooms are created automatically from your directory structure:

- `src/auth/` → room `auth`
- `src/database/` → room `database`
- `src/` (files directly in src) → room `src`
- `tests/` → room `tests`
- Root-level files (`Cargo.toml`, `README.md`) → room `_root`

### Memory

Memory is notes that persist in the database. You (or an AI agent) write memory to record things like:
- Why a design decision was made
- What broke and how it was fixed
- Conventions the team follows
- Gotchas and non-obvious behavior

Memory is organized into **projects** and **topics**. Each memory article belongs to a topic within a project, making it easy to browse and discover related notes.

You drill down through 4 levels to find what you need:

1. **Projects** — which projects have memory?
2. **Topics** — what topics exist in this project? (e.g. "architecture", "gotchas", "auth")
3. **Articles** — what articles exist in this topic?
4. **Article detail** — read the full article body.

---

## Commands

### Indexing

Index a project to store its structure and symbols in the database.

```bash
# Index the current directory
rexicon index .

# Index a specific directory with a custom name
rexicon index ~/code/my-api --name my-api

# Force full re-index (ignores file hashes, re-extracts everything)
rexicon index . --force

# Only index files in src/
rexicon index . --include 'src/**'

# Skip the vendor directory
rexicon index . --exclude vendor

# Include gitignored files (node_modules, target, etc.)
rexicon index . --no-ignore
```

Output:
```
indexed my-api: 342 files (5 changed, 1 removed), 1247 symbols, 68 relationships
```

Re-indexing is **incremental** — only files whose content changed since the last index are re-processed. A 10,000-file project re-indexes in under a second if only 5 files changed.

### Listing Projects

```bash
# Table view
rexicon list

# Output:
# PROJECT              FILES  SYMBOLS MEMORY LAST INDEXED
# my-api                 342     1247      5 2026-04-28T14:30
# frontend               189      823      2 2026-04-27T09:15

# JSON view
rexicon list --format json
```

### Navigating the Hierarchy

Use `show` to browse project → room → topic.

```bash
# Project overview: shows all rooms + memory summary
rexicon show my-api

# Output:
# my-api
#
# Rooms:
#   _root                                                     3 topics
#   auth                                                      5 topics
#   database                                                  6 topics
#   src                                                      12 topics
#   tests                                                     3 topics
#
# Memory: 3 topics, 4 articles (use `rexicon memory list --project my-api` to browse)

# Room detail: shows files and symbols grouped by file
rexicon show my-api auth

# Output:
# my-api / auth
# Path: src/auth/
#
# Files:
#   jwt.rs
#   rbac.rs
#
# Symbols:
#   jwt.rs:
#     [12:35]      pub struct JwtService { ... }
#     [37:52]      pub fn verify_token(&self) -> Result<Claims> { ... }
#   rbac.rs:
#     [8:22]       pub struct RbacMiddleware { ... }

# JSON output for programmatic use
rexicon show my-api --format json
rexicon show my-api auth --format json
```

### Searching

Search across symbols, memory, and documentation.

```bash
# Search all projects
rexicon query "authentication"

# Search one project
rexicon query "user" --project my-api

# Only search symbols
rexicon query "Service" --kind symbol

# Only search memory
rexicon query "broke" --kind memory

# Limit results
rexicon query "fn" --limit 5
```

### Managing Memory

Memory is a knowledge store organized by project and topic. Each article has a title, a body, optional tags, and an author.

#### Browsing memory (4-level drill-down)

```bash
# Level 1: What projects have memory?
rexicon memory list

# Level 2: What topics in this project? (select by name or ID)
rexicon memory list --project my-api
rexicon memory list --project 1

# Level 3: What articles in this topic? (select by name or ID)
rexicon memory list --project my-api --topic auth
rexicon memory list --project my-api --topic 1

# Level 4: Read the full article (select by name or ID)
rexicon memory list --project my-api --topic auth --title "JWT tokens expire after 1 hour"
rexicon memory list --project my-api --topic 1 --title 1
```

#### Adding memory

```bash
# Add a memory entry to a project and topic
rexicon memory add --project my-api --topic "auth" \
  "JWT tokens expire after 1 hour" \
  "The access token TTL is hardcoded in config.rs:42. Refresh tokens last 30 days."

# Add with tags
rexicon memory add --project my-api --topic "database" \
  "Migrations must be backward-compatible" \
  "We run blue-green deploys so old code reads the new schema during rollout." \
  --tags "convention,database"

# Add with a specific author (defaults to "claude")
rexicon memory add --project my-api --topic "auth" \
  "OAuth2 migration planned for Q3" \
  "Moving from JWT to OAuth2 via Auth0. See RFC-042." \
  --tags "decision,auth" \
  --author jacob
```

#### Filtering memory

```bash
# Filter by tags (matches any listed tag)
rexicon memory list --tags "convention"
rexicon memory list --tags "gotcha,security"

# Combine filters
rexicon memory list --project my-api --tags "gotcha"
```

#### Searching memory

```bash
# Search memory by keyword
rexicon memory search "migration"
rexicon memory search "JWT" --project my-api
```

#### Updating and deleting memory

```bash
# Get full detail on one entry (JSON)
rexicon memory get 42

# Update the body
rexicon memory update 42 --body "Updated: TTL is now configurable via AUTH_TOKEN_TTL env var"

# Update tags
rexicon memory update 42 --tags "auth,config,updated"

# Delete
rexicon memory delete 42
```

**Tags** are freeform. Common tags: `convention`, `gotcha`, `fix`, `architecture`, `decision`, `onboarding`, `performance`, `security`, `todo`.

### Checking What Changed

```bash
rexicon diff my-api

# Output:
# my-api — indexed at abc1234, current HEAD at def5678
#
# Changed files:
#   M  src/auth/jwt.rs
#   A  src/auth/oauth.rs
```

### Exploring Dependencies

The `graph` commands let you explore how files depend on each other. Relationships are detected automatically during indexing.

```bash
# What does this file depend on? (direct children)
rexicon graph children <project> --file <path>
rexicon graph c <project> --file <path>              # shorthand

# What depends on this file? (direct parents)
rexicon graph parents <project> --file <path>
rexicon graph p <project> --file <path>              # shorthand

# Full dependency tree downward
rexicon graph tree <project> --file <path>
rexicon graph tree <project> --file <path> --depth 3  # limit depth (default: 10)

# Everything affected if this file changes (reverse tree upward)
rexicon graph impact <project> --file <path>
rexicon graph impact <project> --file <path> --depth 3
```

Examples:

```bash
$ rexicon graph c rexicon --file src/hierarchy.rs
src/hierarchy.rs depends on:
  imports      src/schema.rs
  imports      src/symbol.rs

$ rexicon graph p rexicon --file src/symbol.rs
src/symbol.rs is depended on by:
  imports      src/formatter.rs
  imports      src/hierarchy.rs
  imports      src/lib.rs
  imports      src/main.rs
  imports      src/treesitter.rs
  imports      tests/languages.rs

$ rexicon graph impact rexicon --file src/schema.rs
Changing src/schema.rs affects:
src/schema.rs
├── src/hierarchy.rs
│   ├── src/lib.rs
│   └── src/main.rs
├── src/lib.rs ← (already shown)
├── src/main.rs ← (already shown)
└── src/relationships.rs
    ├── src/lib.rs ← (already shown)
    └── src/main.rs ← (already shown)
```

Relationship types detected automatically during indexing:
- Code imports (Rust, Python, JS/TS, Go, Java, C/C++, Ruby, PHP, C#, Swift, Scala, Lua, Zig, Shell)
- Markdown links and backtick file references
- Config file paths (Cargo.toml, package.json, YAML CI configs, Dockerfile, Makefile)

### Exporting

Export project data back to text files or memory to markdown.

```bash
# Box-drawing tree (default)
rexicon export my-api

# Flat path:line format
rexicon export my-api --format plain

# Custom output path
rexicon export my-api --output ~/Desktop/my-api-index.txt

# Export memory as markdown files (for team review / git commit)
rexicon export my-api --memory-only

# Export memory to a specific directory
rexicon export my-api --memory-only --output ./docs/memory
```

Memory export produces a directory of markdown files organized by topic (one file per topic):

```
.rexicon-export/
  memory/
    architecture.md       ← all articles in the "architecture" topic
    auth.md               ← all articles in the "auth" topic
    database.md           ← all articles in the "database" topic
    conventions.md        ← all articles in the "conventions" topic
```

These files can be committed to the repo and reviewed in PRs.

### Legacy Mode (v1 Compatibility)

The original `rexicon <dir>` command still works exactly as before — it writes a `rexicon.txt` file. It also stores the data in the database as a side effect.

```bash
# These all work like v1
rexicon .
rexicon ~/code/my-api --output /tmp/index.txt
rexicon . --format plain
rexicon . --include 'src/**' --exclude vendor
rexicon . --no-ignore
```

---

## Common Workflows

### First time indexing a project

```bash
cd ~/code/my-api
rexicon index .
rexicon show my-api
```

### Daily check: what changed?

```bash
rexicon diff my-api
rexicon index .          # re-index changed files
```

### Recording a discovery

You just spent 30 minutes debugging a payment issue. Record what you learned:

```bash
rexicon memory add --project my-api --topic "payments" \
  "Webhook handler is not idempotent" \
  "Stripe retries on timeout. If our DB is slow, we process the same event twice and hit a constraint violation that silently drops the event. Need idempotency key check." \
  --tags "gotcha,payments,bug" \
  --author jacob
```

### Finding all conventions

```bash
rexicon memory list --tags convention
```

### Browsing a project's memory

```bash
# See what topics exist
rexicon memory list --project my-api

# Drill into a topic
rexicon memory list --project my-api --topic auth

# Read a specific article
rexicon memory list --project my-api --topic auth --title "JWT tokens expire after 1 hour"
```

### Reviewing notes after a refactor

```bash
rexicon index . --force
# Browse memory to check if any notes need updating
rexicon memory list --project my-api
# Review and update or delete as needed
rexicon memory update 42 --body "Still valid after refactor"
rexicon memory delete 43
```

### Sharing memory with the team

```bash
rexicon export my-api --memory-only --output ./docs/rexicon-memory
git add docs/rexicon-memory
git commit -m "Export rexicon memory notes for team review"
```

### Searching across projects

```bash
rexicon query "authentication"           # all projects
rexicon memory search "convention"       # all memory
```

---

## Data Location

| Path | Contents |
|---|---|
| `~/.rexicon/store.db` | SQLite database with all projects, symbols, rooms, memory |

The database is a single file. Back it up, copy it between machines, or delete it to start fresh.

---

## Supported Languages

Rust, Python, Go, C, C++, JavaScript, TypeScript, C#, Java, Ruby, PHP, Lua, Zig, Swift, Scala, Shell, Markdown (17 languages).
