# Agent Workflows

## Purpose

This document describes how an LLM agent (Claude) actually uses rexicon during a session. The MCP tools and CLI commands are the primitives — this is the playbook for combining them.

## Agent Access Model

The agent accesses rexicon through **MCP tools** (primary) or **CLI via bash** (fallback). Both hit the same SQLite store.

### MCP Tools (Native)

When rexicon runs as an MCP server, the agent gets these tools directly in its tool list:

#### Navigation (read the hierarchy top-down)
| Tool | Purpose | When to use |
|---|---|---|
| `list_projects` | See all indexed projects | Start of session, cross-project work |
| `get_project` | Project overview: architecture, rooms, tech stack | First time in a project, orientation |
| `get_room` | Room detail: topics, symbols, memory | Drilling into a domain |
| `get_topic` | Full content: symbols, relationships, memory | Working on specific code |

#### Search (find by meaning or name)
| Tool | Purpose | When to use |
|---|---|---|
| `query` | Search across symbols, memory, docs | "How does X work?", "Find the thing that does Y" |
| `get_symbols` | Find symbols by name, kind, or file | "Where is UserService defined?", "List all structs" |

#### Memory (write what you learn)
| Tool | Purpose | When to use |
|---|---|---|
| `write_memory` | Create a new note | Discovered something non-obvious |
| `update_memory` | Revise an existing note | Existing note is stale or incomplete |
| `delete_memory` | Remove obsolete note | Note is wrong or no longer relevant |
| `search_memory` | Find past notes | "What did I learn about auth?" |

#### Relationships (understand connections)
| Tool | Purpose | When to use |
|---|---|---|
| `get_imports` | What does this file import? | Understanding a file's dependencies |
| `get_importers` | What imports this file? | Understanding a file's consumers |
| `get_dependencies` | Full dependency chain | Impact analysis, architecture understanding |
| `get_impact` | What breaks if I change this? | Before modifying code |

#### Maintenance
| Tool | Purpose | When to use |
|---|---|---|
| `index` | Trigger re-indexing | After significant code changes, start of long session |
| `diff` | What changed since last index? | Checking if index is stale |

### CLI Fallback (via Bash)

If MCP is not configured, the agent invokes CLI commands through bash:

```bash
rexicon show my-api                    # equivalent to get_project
rexicon show my-api auth               # equivalent to get_room
rexicon query "authentication"         # equivalent to query
rexicon memory add --project my-api --topic gotchas "title" "body"  # equivalent to write_memory
rexicon graph imports my-api --file src/auth/jwt.rs  # equivalent to get_imports
```

Same capabilities, just through a different interface.

---

## Workflow 1: First Encounter (New Project)

The agent has never seen this project before. Goal: get oriented fast.

```
Step 1: Check if project is indexed
  → list_projects()
  → Project not found

Step 2: Index the project
  → index({ root_path: "/Users/jacob/code/my-api" })
  → "342 files indexed, 8 rooms created, 1247 symbols"

Step 3: Get the big picture
  → get_project("my-api")
  → Architecture summary, tech stack, room list, entry points

Step 4: Understand the key areas
  → get_room("my-api", "auth")     # for the area the user is asking about
  → Symbols, topics, relationships

Step 5: Write onboarding memory
  → write_memory({
      project: "my-api",
      topic: "onboarding",
      title: "Project overview",
      body: "REST API with actix-web. 3-layer architecture: api/ handlers → services/ logic → db/ queries. Auth is JWT-based with refresh tokens. Config via environment variables.",
      tags: ["onboarding", "architecture"]
    })
```

**Total tool calls: 4–5.** The agent now understands the project without reading a single source file.

---

## Workflow 2: Returning Session (Known Project)

The agent has worked on this project before. Goal: pick up where it left off.

```
Step 1: Check what's indexed
  → list_projects()
  → "my-api" found, last indexed 2 hours ago

Step 2: Check for changes
  → diff("my-api")
  → "3 files changed, 1 stale memory note"

Step 3: Re-index if needed
  → index({ root_path: "/Users/jacob/code/my-api" })
  → "3 files re-indexed"

Step 4: Get project context + memory
  → get_project("my-api")
  → Architecture + project-wide memory (including past learnings)

Step 5: Read relevant room for the task
  → get_room("my-api", "auth")
  → Symbols + memory (including "Clock skew causes 401" from last session)

Step 6: Review stale memory
  → The stale memory note appears with a warning
  → Agent checks if it's still valid, updates or dismisses
```

**The key difference from Workflow 1:** the agent doesn't start from zero. It sees its own past notes, knows the architecture, and spots what changed.

---

## Workflow 3: Deep Investigation

The user asks: "Why is the payment flow sometimes dropping webhooks?"

```
Step 1: Search for relevant context
  → query({ text: "payment webhook", project: "my-api" })
  → Results: PaymentService symbol, webhook handler, 2 memory notes

Step 2: Check existing memory
  → search_memory({ text: "webhook", project: "my-api" })
  → Found: "Stripe webhooks retry 3x but our handler isn't idempotent" (from a previous session)

Step 3: Navigate to the relevant code
  → get_topic("my-api", "payments", "webhook handler")
  → Full symbols + relationships + memory

Step 4: Trace dependencies
  → get_imports("my-api", "src/payments/webhook.rs")
  → Imports: db/transactions.rs, services/payment_service.rs, models/order.rs

  → get_callers("my-api", "process_webhook")
  → Called by: api/handlers/stripe.rs

Step 5: Check impact
  → get_impact("my-api", "src/payments/webhook.rs")
  → Would affect: 3 files, 2 test files

Step 6: Write findings as memory
  → write_memory({
      project: "my-api",
      topic: "gotchas",
      title: "Webhook drops caused by non-idempotent handler + DB timeout",
      body: "The webhook handler in stripe.rs calls process_webhook which does a DB write without checking if the event was already processed. When the DB is slow (>5s), Stripe retries and we get duplicate processing followed by a constraint violation that silently swallows the event. Fix: add idempotency key check before processing.",
      tags: ["fix", "gotcha", "payments", "production-incident"]
    })
```

**The agent now has a permanent record** of this investigation. Next time anyone asks about webhook issues, the memory surfaces immediately.

---

## Workflow 4: Cross-Project Query

The user asks: "How do our different projects handle authentication?"

```
Step 1: List all projects
  → list_projects()
  → ["my-api", "admin-portal", "mobile-backend"]

Step 2: Search across all projects
  → query({ text: "authentication", limit: 20 })
  → Results from all 3 projects: JWT in my-api, OAuth in admin-portal, API keys in mobile-backend

Step 3: Get specifics per project
  → get_room("my-api", "auth")
  → get_room("admin-portal", "auth")
  → get_room("mobile-backend", "auth")

Step 4: Write per-project memory
  → write_memory({
      project: "my-api",
      topic: "architecture",
      title: "Auth patterns across projects",
      body: "my-api: JWT with refresh tokens. admin-portal: OAuth2 via Auth0. mobile-backend: API key + device tokens. No shared auth library — each project implements its own. Consolidation opportunity.",
      tags: ["architecture", "auth", "cross-project"]
    })
  → (Repeat for admin-portal and mobile-backend if the note is relevant there too)
```

---

## Workflow 5: Pre-Change Impact Analysis

The user says: "I need to refactor the database schema for users."

```
Step 1: Find the relevant files
  → get_symbols({ project: "my-api", name: "User", kind: "struct" })
  → User struct in src/models/user.rs [12:35]

Step 2: Check what depends on this
  → get_impact("my-api", "src/models/user.rs")
  → 14 files import this, including auth, payments, admin handlers

Step 3: Get the dependency tree
  → get_dependencies("my-api", "src/models/user.rs", 2)
  → Tree showing User → [auth, payments, admin, api handlers, tests]

Step 4: Check memory for known constraints
  → search_memory({ text: "user schema", project: "my-api" })
  → Found: "User.legacy_id must stay nullable — mobile app v2.1 still references it"

Step 5: Report to user
  → "Changing User affects 14 files across 4 rooms. Past note: legacy_id must stay nullable for mobile app compatibility. Here's the full impact tree..."
```

---

## When to Write Memory

The agent should write memory when it discovers something that:

1. **Would save time if known upfront** — gotchas, non-obvious behavior, hidden constraints
2. **Explains WHY, not WHAT** — decisions, tradeoffs, incident context (the code shows what, memory records why)
3. **Crosses sessions** — anything the agent would need to re-discover next time
4. **Isn't in the code** — tribal knowledge, undocumented behavior, environment-specific quirks

### Memory write triggers

| Situation | Memory to write |
|---|---|
| Agent discovers a bug's root cause | Fix note: what broke, why, how to fix |
| User explains a non-obvious design decision | Decision note: what was decided and why |
| Agent encounters a gotcha | Gotcha note: what surprised it, how to avoid |
| Agent completes a complex investigation | Summary note: findings, conclusions |
| User corrects the agent | Convention note: what the correct pattern is |
| Agent notices a pattern across files | Pattern note: what the pattern is, where it applies |

### Memory write discipline

- **Pick the right topic** — put a JWT-specific note in a "gotchas" or "auth" topic, not a catch-all
- **Title as TL;DR** — the title alone should tell you if this note is relevant
- **Tag consistently** — use the standard tags (gotcha, fix, convention, architecture, decision, onboarding)
- **Update, don't duplicate** — search for existing memory before writing a new one on the same topic
- **Review stale notes** — when memory is flagged stale, update or delete rather than ignoring

---

## When NOT to Use Rexicon

Rexicon replaces grep/find for structural questions, but it doesn't replace reading code:

| Question | Use rexicon | Use source files |
|---|---|---|
| "What functions are in auth.rs?" | Yes — `get_room` or `get_symbols` | No |
| "What does `verify_token` do?" | Start with rexicon for signature + context | Read the source for implementation |
| "Where is UserService defined?" | Yes — `get_symbols` | No |
| "Why is line 47 doing X?" | Check memory first | Read the file if no memory |
| "What imports this file?" | Yes — `get_importers` | No |
| "What's in this JSON config?" | No — rexicon doesn't index config files | Read the file |

The pattern: **rexicon for navigation and context, source files for implementation details.**

---

## Updated Skill Definition

The Claude skill (`SKILL.md`) should instruct the agent to follow these workflows. Key behavioral instructions:

```
When starting work on a project:
1. Call list_projects() to check if the project is indexed.
2. If not indexed, call index() first.
3. If indexed, call diff() to check freshness. Re-index if stale.
4. Call get_project() to load architecture + memory.
5. Navigate to the relevant room/topic for the task at hand.

When you learn something non-obvious:
- Write it as memory immediately. Don't wait until the end of the session.
- Pick the most appropriate topic for the note (e.g. "gotchas", "architecture", "conventions").
- Check for existing memory on the topic first (search_memory) to update rather than duplicate.

When investigating a problem:
- Start with query() to find relevant content across the project.
- Use get_impact() before suggesting changes.
- Check search_memory() for past investigations on the same area.

When finishing a task:
- Write a memory note summarizing what you learned, especially:
  - What didn't work and why
  - Surprising constraints or behavior
  - Decisions made and their reasoning
```

---

## Agent Tool Call Patterns (Typical Session)

### Short session (quick question, ~5 tool calls)
```
list_projects → get_project → get_room → [answer user]
```

### Medium session (bug fix, ~10-15 tool calls)
```
list_projects → diff → get_project → query → get_room → get_topic
  → get_imports → [fix code] → write_memory
```

### Long session (feature work, ~20-30 tool calls)
```
list_projects → diff → index → get_project → query → get_room → get_topic
  → get_imports → get_impact → [work on code]
  → write_memory → [more code work] → get_room (different area)
  → get_topic → get_callers → [more code] → write_memory → update_memory
```

### Cross-project research (~15-20 tool calls)
```
list_projects → query (cross-project) → get_project (A) → get_room (A)
  → get_project (B) → get_room (B) → search_memory → write_memory (per-project)
```

---

## Error Recovery

| Error | Agent action |
|---|---|
| `not_found: Project 'X' not found` | Call `index()` to index the project |
| `not_found: Room 'Y' not found` | Call `get_project()` to see available rooms |
| `index_required: Project 'X' has never been indexed` | Call `index()` |
| `stale memory` warning in response | Review the note, call `update_memory` or `delete_memory` |
| MCP server not running | Fall back to CLI via bash |
