# Rexicon Agent Guide

Instructions for AI agents (Claude, etc.) on how to use rexicon to understand, navigate, and remember information about codebases.

---

## What Rexicon Is

Rexicon is a local project intelligence layer. It indexes codebases into a SQLite database and provides commands to navigate the structure, search symbols, and read/write persistent memory — all without grepping or reading source files directly.

**Use rexicon instead of `grep`, `find`, or reading files** when you need to:
- Understand what's in a project
- Find where something is defined
- Check what depends on what
- Recall past discoveries about the codebase

**Read the actual source files** when you need:
- The implementation body of a specific function
- Line-by-line logic details
- Config file contents

---

## Session Startup

At the beginning of every session that involves working with code:

**If rexicon is available as an MCP server** (preferred — you'll see `mcp__rexicon__*` tools), use the native tools directly. They return structured JSON and require no text parsing. The workflow is the same, just use the MCP tool names instead of bash commands.

**Otherwise, use the CLI:**

```bash
# 1. Check what's indexed
rexicon list

# 2. If the project isn't listed, index it
rexicon index /path/to/project

# 3. If it is listed, check freshness
rexicon diff <project-name>

# 4. Re-index if files changed
rexicon index /path/to/project

# 5. Load project context
rexicon show <project-name>

# 6. Browse memory from previous sessions
rexicon memory list --project <project-name>
```

This gives you the architecture, rooms, and a summary of any memory from previous sessions — in 4-6 tool calls.

---

## Command Reference

### Navigate the hierarchy (top-down)

```bash
# What projects exist?
rexicon list

# What's in this project? (rooms, memory count)
rexicon show <project>

# What's in this room? (files, symbols grouped by file)
rexicon show <project> <room>

# JSON output for structured parsing
rexicon show <project> --format json
rexicon show <project> <room> --format json
```

**Always navigate top-down.** Don't guess room or topic names — ask the hierarchy first.

The `show` command displays a memory count rather than inline memory:
```
Memory: 3 topics, 4 articles (use `rexicon memory list --project rexicon` to browse)
```

Use `rexicon memory list` to drill into memory separately (see below).

### Search

```bash
# Search across everything (symbols + memory)
rexicon query "<text>"

# Scope to one project
rexicon query "<text>" --project <name>

# Only symbols
rexicon query "<text>" --kind symbol

# Only memory
rexicon query "<text>" --kind memory

# Limit results
rexicon query "<text>" --limit 5
```

### Write memory

When you discover something non-obvious, **write it down immediately**.

```bash
rexicon memory add --project <project> --topic "<topic>" "<title>" "<body>" [--tags tag1,tag2] [--author name]
```

**Projects and topics:**
- `--project` selects which project this memory belongs to (must already be indexed)
- `--topic` is a user-defined category that groups related articles together

**Choosing good topic names:**
Topics are conceptual categories, NOT code directory paths. Pick names that describe what kind of knowledge the article contains:

| Good topics | Why |
|---|---|
| `architecture` | Structural decisions, module boundaries, data flow |
| `gotchas` | Surprising behavior, footguns, things that waste time |
| `conventions` | Team rules, naming patterns, code style agreements |
| `decisions` | Why something was chosen over alternatives |
| `debugging` | Root cause analyses, investigation summaries |
| `onboarding` | Context a newcomer would need |
| `patterns` | Recurring code patterns, idioms used across the project |

Bad topic names: `src/treesitter`, `auth/jwt`, `models/user` — these are code paths, not knowledge categories. If you want to note something about the treesitter module, put it in the `gotchas` or `architecture` topic with a descriptive title.

**Author** defaults to `claude`. Set `--author <name>` when writing on behalf of a user.

**Examples:**

```bash
# Record a gotcha
rexicon memory add --project my-api --topic "gotchas" \
  "Clock skew > 30s causes silent 401" \
  "The refresh endpoint validates exp with a 30s tolerance. Client clock drift beyond that causes silent failures." \
  --tags "gotcha,auth"

# Record a convention (visible when browsing the conventions topic)
rexicon memory add --project my-api --topic "conventions" \
  "All DB columns use snake_case" \
  "Team convention. Enforced by the migration linter in CI." \
  --tags "convention,database"

# Record what you learned from debugging
rexicon memory add --project my-api --topic "debugging" \
  "Webhook drops caused by non-idempotent handler" \
  "process_webhook does a DB write without checking if the event was already processed. Duplicate Stripe retries hit a constraint violation that silently swallows the event." \
  --tags "fix,gotcha,payments"

# Record a design decision
rexicon memory add --project my-api --topic "decisions" \
  "SQLite over files" \
  "Chose SQLite because it provides atomic writes, schema enforcement, and FTS5 for search — all without a server process." \
  --tags "decision" --author jacob

# Record on behalf of the user
rexicon memory add --project my-api --topic "decisions" \
  "OAuth2 migration planned for Q3" \
  "Moving from JWT to OAuth2 via Auth0. See RFC-042." \
  --tags "decision" --author jacob
```

### Read memory (4-level drill-down)

Memory is organized as a hierarchy: **projects > topics > articles**. Use the drill-down to navigate it level by level.

```bash
# Level 1: What projects have memory?
rexicon memory list

# Level 2: What topics in this project? (select by name or ID)
rexicon memory list --project rexicon
rexicon memory list --project 1

# Level 3: What articles in this topic? (select by name or ID)
rexicon memory list --project rexicon --topic architecture
rexicon memory list --project rexicon --topic 1

# Level 4: Read the full article (select by name or ID)
rexicon memory list --project rexicon --topic architecture --title "Flat module structure"
rexicon memory list --project rexicon --topic 1 --title 1
```

**Always drill down level by level.** Don't guess topic or article names — list first, then select.

```bash
# Search memory by keyword (across all projects or scoped)
rexicon memory search "query"
rexicon memory search "query" --project my-api
```

### Update and clean up memory

```bash
# Update body
rexicon memory update <id> --body "Updated: ..."

# Update title
rexicon memory update <id> --title "New title"

# Update tags
rexicon memory update <id> --tags "new,tags"

# Delete obsolete memory
rexicon memory delete <id>
```

### Export memory for team review

```bash
# Export memory as markdown files
rexicon export <project> --memory-only

# Export to a specific directory
rexicon export <project> --memory-only --output ./docs/memory
```

### Check what changed

```bash
rexicon diff <project>
```

Reports: changed files, added files, removed files, and any memory entries flagged stale.

### Explore dependencies

Relationships are extracted automatically during `rexicon index` — no manual setup needed. Covers code imports (14 languages), markdown links, backtick code references, and config file paths.

```bash
# What does this file depend on? (direct)
rexicon graph children <project> --file <path>
rexicon graph c <project> --file <path>              # shorthand

# What depends on this file? (direct)
rexicon graph parents <project> --file <path>
rexicon graph p <project> --file <path>              # shorthand

# Full dependency tree downward
rexicon graph tree <project> --file <path>
rexicon graph tree <project> --file <path> --depth 3

# What breaks if I change this? (reverse tree upward)
rexicon graph impact <project> --file <path>
```

### Index and re-index

```bash
# Incremental (only changed files)
rexicon index /path/to/project

# Full re-index
rexicon index /path/to/project --force

# Custom project name
rexicon index /path/to/project --name my-api
```

### MCP Server (native tools)

When rexicon is configured as an MCP server (`rexicon serve`), all commands are available as native tools with structured JSON responses instead of CLI text output. The agent does not need to parse text — responses come back as typed data.

The 15 MCP tools map 1:1 to CLI commands:

| MCP tool | CLI equivalent |
|---|---|
| `list_projects` | `rexicon list` |
| `get_project` | `rexicon show <project>` |
| `get_room` | `rexicon show <project> <room>` |
| `query` | `rexicon query` |
| `index` | `rexicon index` |
| `diff` | `rexicon diff` |
| `get_children` | `rexicon graph children` |
| `get_parents` | `rexicon graph parents` |
| `get_tree` | `rexicon graph tree` |
| `get_impact` | `rexicon graph impact` |
| `memory_list` | `rexicon memory list` |
| `memory_write` | `rexicon memory add` |
| `memory_update` | `rexicon memory update` |
| `memory_delete` | `rexicon memory delete` |
| `memory_search` | `rexicon memory search` |

To configure, add `.mcp.json` to the project root:

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

---

## When to Write Memory

Write memory when you discover something that:

| Situation | What to write | Topic | Tags |
|---|---|---|---|
| Found a bug's root cause | What broke, why, how to fix | `debugging` | `fix`, `gotcha` |
| User explains a design decision | What was decided and why | `decisions` | `decision`, `architecture` |
| Hit surprising behavior | What surprised you, how to avoid it | `gotchas` | `gotcha` |
| Completed a complex investigation | Summary of findings | `debugging` | `fix`, `investigation` |
| User corrects your approach | The correct pattern | `conventions` | `convention` |
| Notice a cross-file pattern | What the pattern is, where it applies | `architecture` | `architecture`, `pattern` |
| Before refactoring a file | Check impact first | `debugging` | `architecture` |
| Something would help a newcomer | Onboarding context | `onboarding` | `onboarding` |

### Memory discipline

1. **Pick meaningful topics** — topics are knowledge categories (`gotchas`, `architecture`, `decisions`), not code paths. A good topic groups articles that a reader would want to browse together.
2. **Reuse existing topics** — before creating a new topic, run `rexicon memory list --project <name>` to see what topics already exist. Add to an existing topic rather than creating a near-duplicate.
3. **Title as TL;DR** — the title alone should tell you if this note matters
4. **Search before writing** — check `rexicon memory search "<keyword>"` first. Update rather than duplicate.
5. **Review stale notes** — when `diff` reports stale memory, update or delete it
6. **Don't memorize code** — the code is in the index. Memorize the *why*, not the *what*.
7. **Set author correctly** — use `--author <name>` when recording something the user told you
8. **Keep topics lean** — aim for 3-8 topics per project. If you have 15+ topics, some should be merged.

---

## When NOT to Use Rexicon

| Question | Use rexicon? | Instead |
|---|---|---|
| "What structs exist in auth?" | Yes — `rexicon show <project> auth` | |
| "What does line 47 do?" | No | Read the source file |
| "Where is UserService?" | Yes — `rexicon query "UserService"` | |
| "What's in .env?" | No | Read the file (rexicon doesn't index config) |
| "What calls this function?" | File-level: `rexicon graph p <project> --file <path>` | Symbol-level: use grep |
| "What conventions apply here?" | Yes — `rexicon memory list --project my-api --topic conventions` | |

---

## Typical Session Patterns

### Short session (answering a question, ~3-5 commands)

```
rexicon list
rexicon show my-api
rexicon show my-api auth
-> answer the user's question
```

### Bug fix session (~8-12 commands)

```
rexicon list
rexicon diff my-api
rexicon index /path/to/project         # if stale
rexicon show my-api
rexicon query "payment webhook"
rexicon show my-api payments
rexicon graph impact my-api --file src/payments/webhook.rs
-> investigate, fix the bug
rexicon memory add --project my-api --topic "debugging" "Webhook not idempotent" "..." --tags "fix,gotcha"
```

### Feature work (~15-20 commands)

```
rexicon list
rexicon diff my-api
rexicon index /path/to/project
rexicon show my-api
rexicon query "user validation"
rexicon show my-api models
rexicon graph c my-api --file src/models/user.rs
-> write code
rexicon memory add --project my-api --topic "architecture" "User email validated at service layer" "..." --tags "architecture"
-> more code
rexicon show my-api tests
-> write tests
rexicon memory add --project my-api --topic "conventions" "Integration tests hit real DB" "..." --tags "convention"
```

### Cross-project research (~10-15 commands)

```
rexicon list
rexicon query "authentication"
rexicon show project-a auth
rexicon show project-b auth
rexicon memory search "auth"
rexicon memory add --project project-a --topic "architecture" "Auth patterns differ from project-b" "project-a uses JWT, project-b uses OAuth2 via Auth0." --tags "architecture,auth"
```

### Browsing memory from a previous session

```
rexicon memory list                                    # which projects have memory?
rexicon memory list --project my-api                   # what topics?
rexicon memory list --project my-api --topic gotchas   # what articles?
rexicon memory list --project my-api --topic gotchas --title "Clock skew causes 401"   # read full article
```

### Reviewing stale memory after changes

```
rexicon diff my-api
rexicon memory list --project my-api
-> for each topic, check articles:
rexicon memory list --project my-api --topic gotchas
rexicon memory list --project my-api --topic gotchas --title 1   # read the full note
rexicon memory update <id> --body "Still valid"                  # if still accurate
rexicon memory delete <id>                                       # if obsolete
```

---

## Error Handling

| Error | What to do |
|---|---|
| `project 'X' not found` | Run `rexicon index /path/to/X` |
| `room 'Y' not found in project 'X'` | Run `rexicon show X` to see available rooms |
| `no results for 'Z'` | Try broader search terms, or check `rexicon list` to confirm project is indexed |
| Stale memory warning | Read the note, then `rexicon memory update <id>` or `rexicon memory delete <id>` |

---

## Output Formats

All `show`, `list`, and `query` commands support `--format json` for structured output. Use JSON when you need to parse the response programmatically. Use the default table format when displaying to the user.

```bash
rexicon list --format json
rexicon show my-api --format json
rexicon show my-api auth --format json
```
