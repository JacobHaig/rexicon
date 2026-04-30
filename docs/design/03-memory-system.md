# Composable Memory System

## Purpose

The memory system is where the agent writes what it learns. Code analysis tells you what exists; memory records what it means, what broke, what to avoid, what works, and why decisions were made.

Memory is a first-class citizen -- it lives alongside code-derived content in the same database, is searchable, and persists across sessions.

## Memory Organization: Project -> Topic -> Article

Memory is organized as a three-level hierarchy:

```
Project
  └── Topic (user-defined category)
        └── Article (individual memory entry)
```

- **Project** -- the indexed codebase. Every memory entry belongs to exactly one project.
- **Topic** -- a user-defined category within the project. Topics are conceptual groupings like "architecture", "gotchas", "conventions", "onboarding", or "incident-log". They are not code paths or filesystem directories.
- **Article** -- an individual memory entry with a title, body, tags, and author.

There is no cross-project or global scope. Memory is always project-scoped. If information applies to multiple projects, add it to each project's relevant topic.

## Memory Anatomy

Each memory article has:

| Field | Description |
|---|---|
| `title` | Short label: `"JWT refresh silently fails on clock skew > 30s"` |
| `body` | Full note -- what happened, why it matters, what to do |
| `tags` | Categorization: `["gotcha", "auth", "production-incident"]` |
| `author` | Who wrote it: `"claude"`, `"jacob"`, a team member |
| `stale` | Whether the underlying code changed since this was written |

The article's project and topic are determined by its position in the hierarchy (via `topic_id` -> `memory_topics.project_id`).

## CLI Drill-Down

The CLI provides a 4-level drill-down for browsing memory:

| Command | Output |
|---|---|
| `rexicon memory list` | Projects with memory counts |
| `rexicon memory list --project my-api` | Topics in that project |
| `rexicon memory list --project my-api --topic gotchas` | Article titles in that topic |
| `rexicon memory list --project my-api --topic gotchas --title "Clock skew"` | Full article content |

## Memory Categories (via tags)

Tags are freeform, but the system recognizes common categories for smart filtering:

| Tag | Use case |
|---|---|
| `convention` | Team practices: naming, patterns, style |
| `gotcha` | Things that bite you: hidden constraints, surprising behavior |
| `fix` | What didn't work and what did: debugging knowledge |
| `architecture` | Why the system is structured this way |
| `todo` | Acknowledged gaps, planned improvements |
| `decision` | Decisions made and their reasoning |
| `onboarding` | What a newcomer (human or agent) should know first |
| `performance` | Performance characteristics, known bottlenecks |
| `security` | Security considerations, sensitive areas |

## Operations

### Add memory

```
rexicon memory add --project <project> --topic <topic> "<title>" "<body>" [--tags tag1,tag2] [--author name]
```

The topic is created automatically if it does not already exist in the project.

Example:
```bash
rexicon memory add --project my-api --topic gotchas \
  "Clock skew > 30s causes silent refresh failure" \
  "The refresh endpoint validates exp with a 30s tolerance. If the client clock is off by more than that, the refresh silently returns a 401 instead of a descriptive error. Found this during the 2026-04-15 incident. The fix would be to add a clock-skew header in the response, but that's not implemented yet." \
  --tags gotcha,auth,incident
```

### List memory

```
rexicon memory list [--project <name>] [--topic <topic>] [--title <title>] [--tags <tags>] [--include-stale]
```

By default, stale memory is included but marked. The `--include-stale` flag is the default; `--exclude-stale` hides them.

### Get memory

```
rexicon memory get <id>
```

### Update memory

```
rexicon memory update <id> [--title "..."] [--body "..."] [--tags tag1,tag2]
```

Updating a stale memory automatically clears the stale flag (the agent has reviewed it).

### Delete memory

```
rexicon memory delete <id>
```

### Search memory

```
rexicon memory search "<query>" [--project <name>] [--topic <topic>]
```

Keyword search across title + body + tags. When RAG is available, this becomes semantic search.

## Staleness Detection

When a file is re-indexed and its content has changed:

1. Identify all `symbols` that were in that file.
2. Find all `memory` rows whose topic is associated with symbols in that file.
3. Set `stale = 1` on those memory rows.

When the agent accesses a topic with stale memory, the response includes a notice:

```
[STALE] "Clock skew > 30s causes silent refresh failure"
  └── Last updated: 2026-04-15. Code in src/auth/jwt.rs changed since.
      Review this memory -- it may no longer be accurate.
```

The agent can then:
- Update the memory if it's still valid (clears stale flag)
- Delete it if it's obsolete
- Ignore it (stays stale)

## Memory in MCP responses

When the agent navigates to a room or topic via MCP tools, relevant memory is included based on the project:

```json
{
  "room": "auth",
  "summary": "Authentication and authorization. JWT-based with refresh tokens.",
  "topics": ["JWT refresh flow", "RBAC", "Session management"],
  "memory": [
    {
      "id": 42,
      "topic": "gotchas",
      "title": "Clock skew > 30s causes silent refresh failure",
      "tags": ["gotcha", "auth"],
      "stale": false,
      "preview": "The refresh endpoint validates exp with a 30s tolerance..."
    },
    {
      "id": 3,
      "topic": "conventions",
      "title": "All DB columns use snake_case",
      "tags": ["convention"]
    }
  ]
}
```

## Team Collaboration

Memory is stored in the SQLite database, but can be exported for team review:

```bash
rexicon export my-api --memory-only --format md
```

Produces:
```
.rexicon-export/
  memory/
    architecture/
      project-overview.md
      layer-responsibilities.md
    gotchas/
      clock-skew-jwt.md
    conventions/
      db-column-naming.md
```

These are human-readable markdown files organized by topic that can be committed to the repo, reviewed in PRs, and imported back:

```bash
rexicon import my-api .rexicon-export/memory/
```

This gives you the best of both worlds: SQLite for speed and querying, markdown files for collaboration.

## Memory Compaction

Over time, memory accumulates. The agent can compact old memories:

```bash
rexicon memory compact --project <project> [--topic <topic>]
```

This lists all memory in the scope, sorted by age and staleness, so the agent can review and consolidate. For example, 5 separate fix notes about the same JWT issue can be merged into one comprehensive note.
