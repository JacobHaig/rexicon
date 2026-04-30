# CLI Interface

## Overview

The CLI is restructured from a single command (`rexicon <dir>`) into subcommands. The original behavior is preserved as backward-compatible shorthand.

## Command Structure

```
rexicon <subcommand> [options]

Subcommands:
  index       Index a project directory
  list        List indexed projects
  show        Navigate the hierarchy (project / room / topic)
  query       Search across content, symbols, and memory
  memory      Read and write agent memory
  graph       Query the relationship graph
  diff        Show changes since last index
  export      Export to files (txt, json, markdown)
  serve       Start the MCP server
  config      View or modify configuration
```

### Backward Compatibility

```bash
# v1 behavior: index + write rexicon.txt
rexicon <dir>                        # still works
rexicon <dir> --output <path>        # still works
rexicon <dir> --format plain         # still works
```

When invoked without a subcommand and with a directory argument, rexicon runs in legacy mode: index the project into the DB and write `rexicon.txt` (or the specified output path).

## Subcommand Details

### `rexicon index`

Index or re-index a project.

```bash
rexicon index <dir> [options]

Options:
  --name <name>         Project name (default: directory name)
  --no-ignore           Include gitignored files
  --include <glob>      Only include matching paths (repeatable)
  --exclude <glob>      Exclude matching paths (repeatable)
  --no-embed            Skip embedding generation even if RAG is enabled
  --force               Force full re-index (ignore file hashes)

Examples:
  rexicon index .
  rexicon index ~/code/my-api --name my-api
  rexicon index . --exclude vendor --exclude '**/generated/**'
  rexicon index . --force
```

Output (stderr):
```
indexed my-api: 342 files (5 changed, 1 removed), 1247 symbols, 3 stale memories flagged
```

### `rexicon list`

List all indexed projects.

```bash
rexicon list [options]

Options:
  --format <fmt>        Output format: table (default), json

Examples:
  rexicon list
  rexicon list --format json
```

Output:
```
PROJECT     FILES   SYMBOLS  MEMORY  LAST INDEXED
my-api      342     1247     15      2026-04-28 14:30
frontend    189     823      8       2026-04-27 09:15
rexicon     12      156      3       2026-04-28 16:00
```

### `rexicon show`

Navigate the hierarchy. Accepts 1–3 positional arguments for increasing specificity.

```bash
rexicon show <project> [room] [topic]

Examples:
  rexicon show my-api                    # project overview + rooms
  rexicon show my-api auth               # room details + topics + memory
  rexicon show my-api auth JwtService    # topic content + symbols + relationships
```

Project-level output:
```
my-api — REST API (rust, typescript)
Architecture: 3 layers: HTTP handlers → services → repositories. Entry: src/main.rs.

Rooms:
  api         HTTP handlers and middleware               12 topics
  auth        JWT authentication and RBAC                 5 topics
  db          Database access, migrations, queries        6 topics
  models      Data models and validation                  4 topics
  services    Business logic                              8 topics

Memory (project-wide):
  [3] All DB columns use snake_case  #convention
  [7] Avoid touching legacy_ prefixed files without Jake  #gotcha
```

Room-level output:
```
my-api / auth — JWT-based authentication and RBAC authorization
Path: src/auth/

Topics:
  JwtService       symbol_group   Token generation and validation
  RBAC middleware   flow           Role-based access control
  refresh flow     pattern        Token refresh lifecycle

Symbols:
  pub struct JwtService { ... }                          src/auth/jwt.rs [12:35]
  pub fn verify_token(token: &str) -> Result<Claims>     src/auth/jwt.rs [37:52]
  pub fn refresh_token(token: &str) -> Result<String>    src/auth/jwt.rs [54:78]
  pub struct RbacMiddleware { ... }                      src/auth/rbac.rs [8:22]

Memory:
  [42] Clock skew > 30s causes silent refresh failure  #gotcha #auth
  [43] Refresh tokens stored in HttpOnly cookies, not localStorage  #decision #security
```

### `rexicon query`

Search across everything.

```bash
rexicon query <text> [options]

Options:
  --project <name>      Scope to a project
  --topic <name>        Scope to a topic (requires --project)
  --kind <kind>         Filter: symbol, memory, content
  --limit <n>           Max results (default: 10)
  --format <fmt>        table (default), json

Examples:
  rexicon query "authentication"
  rexicon query "JWT" --project my-api
  rexicon query "what breaks" --kind memory --project my-api
  rexicon query "user validation" --format json
```

Output:
```
SCORE  KIND     SCOPE                      TITLE
0.95   memory   my-api/auth/JWT            Clock skew > 30s causes silent refresh failure
0.87   symbol   my-api/auth/JwtService     pub fn verify_token(token: &str) -> Result<Claims>
0.82   symbol   my-api/auth/JwtService     pub fn refresh_token(token: &str) -> Result<String>
0.71   memory   my-api/auth                Refresh tokens stored in HttpOnly cookies
0.65   content  my-api/api/auth_handler    src/api/handlers/auth.rs — handles POST /auth/refresh
```

### `rexicon memory`

Manage agent-written memory.

```bash
rexicon memory add <scope-path> <title> <body> [--tags t1,t2]
rexicon memory list [--project <name>] [--topic <name>] [--title <text>] [--tags <tags>] [--stale-only]
rexicon memory get <id>
rexicon memory update <id> [--title "..."] [--body "..."] [--tags t1,t2]
rexicon memory delete <id>
rexicon memory search <query> [--project <name>]
rexicon memory compact --project <name> [--topic <name>]

Examples:
  rexicon memory add "my-api/auth/JWT" "Clock skew causes 401" "The refresh endpoint..." --tags gotcha,auth
  rexicon memory list --project my-api --tags gotcha
  rexicon memory search "what broke" --project my-api
  rexicon memory update 42 --body "Updated: tolerance increased to 60s"
  rexicon memory delete 42
```

### `rexicon graph`

Query the relationship graph.

```bash
rexicon graph imports <project> --file <path>
rexicon graph importers <project> --file <path>
rexicon graph calls <project> --symbol <name>
rexicon graph callers <project> --symbol <name>
rexicon graph deps <project> --from <path> [--depth <n>]
rexicon graph impact <project> --file <path>
rexicon graph export <project> --format dot|json

Examples:
  rexicon graph imports my-api --file src/auth/jwt.rs
  rexicon graph impact my-api --file src/database/schema.rs
  rexicon graph deps my-api --from src/main.rs --depth 3
  rexicon graph export my-api --format dot > deps.dot
```

### `rexicon diff`

Show what changed since last index.

```bash
rexicon diff <project>

Examples:
  rexicon diff my-api
```

Output:
```
my-api — indexed at abc1234, current HEAD at def5678

Changed files:
  M  src/auth/jwt.rs
  M  src/api/handlers/auth.rs
  A  src/auth/oauth.rs

Stale memory:
  [42] Clock skew > 30s causes silent refresh failure
       └── src/auth/jwt.rs changed since this was written
```

### `rexicon export`

Export project data to files for sharing or review.

```bash
rexicon export <project> [options]

Options:
  --format txt|json|md     Output format (default: txt)
  --output <dir>           Output directory (default: .rexicon-export/)
  --memory-only            Only export memory files
  --symbols-only           Only export symbol tree

Examples:
  rexicon export my-api                          # full export as txt
  rexicon export my-api --format md --output docs/rexicon/
  rexicon export my-api --memory-only --format md
```

### `rexicon serve`

Start the MCP server.

```bash
rexicon serve [options]

Options:
  --transport stdio|tcp    Transport mode (default: stdio)
  --port <port>            TCP port (default: 3100, only with --transport tcp)

Examples:
  rexicon serve                       # stdio mode for Claude Code
  rexicon serve --transport tcp       # TCP mode on port 3100
```

### `rexicon config`

View or modify configuration.

```bash
rexicon config show
rexicon config set <key> <value>
rexicon config reset

Examples:
  rexicon config show
  rexicon config set rag.enabled true
  rexicon config set rag.provider openai
  rexicon config set rag.openai.model text-embedding-3-small
```

## Output Formats

All subcommands that produce output support `--format`:

| Format | Description |
|---|---|
| `table` | Human-readable table (default for terminal) |
| `json` | Machine-readable JSON (for scripting and MCP) |
| `plain` | Minimal text, one item per line (for piping) |

When stdout is not a terminal (piped), default to `json` instead of `table`.

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error (DB error, IO error) |
| 2 | Invalid arguments |
| 3 | Project not found |
| 4 | Nothing to do (e.g., `diff` with no changes) |
