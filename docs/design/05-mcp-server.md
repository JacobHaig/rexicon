# MCP Server

## Purpose

The MCP (Model Context Protocol) server exposes rexicon's capabilities as native tools for AI agents. Instead of the agent calling a CLI via bash and parsing text output, it invokes structured tools with typed inputs and receives structured JSON responses.

This is the primary interface for agent use. The CLI remains for humans and scripting.

## Running the Server

```bash
# Start the MCP server (stdio mode for Claude Code/Desktop)
rexicon serve

# Start with TCP transport (for remote or multi-client use)
rexicon serve --transport tcp --port 3100
```

### Claude Code Configuration

In the user's `~/.claude/settings.json` or project `.claude/settings.json`:

```json
{
  "mcpServers": {
    "rexicon": {
      "command": "rexicon",
      "args": ["serve"],
      "env": {}
    }
  }
}
```

This makes all rexicon tools available as `mcp__rexicon__<tool_name>` in Claude Code.

### Claude Desktop Configuration

In `~/Library/Application Support/Claude/claude_desktop_config.json`:

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

## Tool Definitions

### Navigation Tools

#### `list_projects`

List all indexed projects.

**Input:** none

**Output:**
```json
{
  "projects": [
    {
      "name": "my-api",
      "root_path": "/Users/jacob/code/my-api",
      "tech_stack": ["rust", "typescript"],
      "last_indexed": "2026-04-28T14:30:00Z",
      "room_count": 8,
      "symbol_count": 342,
      "memory_count": 15
    }
  ]
}
```

#### `get_project`

Get project overview including rooms and architecture.

**Input:** `{ "project": "my-api" }`

**Output:**
```json
{
  "name": "my-api",
  "root_path": "/Users/jacob/code/my-api",
  "tech_stack": ["rust", "typescript"],
  "architecture": "REST API with 3 layers: HTTP handlers (src/api/) → services (src/services/) → repositories (src/db/). Entry point: src/main.rs. Auth via JWT middleware.",
  "entry_points": ["src/main.rs"],
  "rooms": [
    { "name": "api", "summary": "HTTP handlers and middleware", "topic_count": 12 },
    { "name": "services", "summary": "Business logic layer", "topic_count": 8 },
    { "name": "db", "summary": "Database access and migrations", "topic_count": 6 }
  ],
  "project_memory": [
    { "id": 3, "title": "All DB columns use snake_case", "tags": ["convention"] }
  ],
  "head_commit": "abc1234",
  "last_indexed": "2026-04-28T14:30:00Z"
}
```

#### `get_room`

Get room details including topics and memory.

**Input:** `{ "project": "my-api", "room": "auth" }`

**Output:**
```json
{
  "name": "auth",
  "path": "src/auth",
  "summary": "JWT-based authentication and RBAC authorization.",
  "topics": [
    { "name": "JwtService", "kind": "symbol_group", "summary": "Token generation and validation" },
    { "name": "RBAC middleware", "kind": "flow", "summary": "Role-based access control" },
    { "name": "refresh flow", "kind": "pattern", "summary": "Token refresh lifecycle" }
  ],
  "symbols": [
    { "signature": "pub struct JwtService { ... }", "kind": "struct", "file": "src/auth/jwt.rs", "lines": "12:35" },
    { "signature": "pub fn verify_token(token: &str) -> Result<Claims>", "kind": "function", "file": "src/auth/jwt.rs", "lines": "37:52" }
  ],
  "memory": [
    { "id": 42, "title": "Clock skew > 30s causes silent refresh failure", "tags": ["gotcha"], "stale": false }
  ]
}
```

#### `get_topic`

Get full topic content — symbols, documentation, memory, relationships.

**Input:** `{ "project": "my-api", "room": "auth", "topic": "JwtService" }`

**Output:**
```json
{
  "name": "JwtService",
  "kind": "symbol_group",
  "content": [
    {
      "kind": "symbol",
      "signature": "pub struct JwtService { ... }",
      "file": "src/auth/jwt.rs",
      "lines": "12:35",
      "children": [
        { "signature": "pub fn new(secret: &str) -> Self { ... }", "lines": "14:20" },
        { "signature": "pub fn generate(&self, claims: Claims) -> String { ... }", "lines": "22:34" }
      ]
    }
  ],
  "relationships": {
    "imports": ["src/config.rs", "src/models/user.rs"],
    "imported_by": ["src/api/handlers/auth.rs", "src/middleware/auth.rs"],
    "calls": ["Config::jwt_secret", "User::find_by_id"],
    "called_by": ["auth_handler", "refresh_handler"]
  },
  "memory": [
    { "id": 42, "title": "Clock skew > 30s causes silent refresh failure", "body": "...", "tags": ["gotcha"] }
  ]
}
```

### Search Tools

#### `query`

Search across all indexed content — symbols, memory, documentation.

**Input:**
```json
{
  "text": "authentication flow",
  "project": "my-api",       // optional: scope to project
  "scope": "auth",           // optional: scope to room
  "kind": "memory",          // optional: filter to content kind
  "limit": 10                // optional: max results
}
```

**Output:**
```json
{
  "results": [
    {
      "score": 0.95,
      "kind": "memory",
      "topic": "JWT",
      "title": "Clock skew > 30s causes silent refresh failure",
      "preview": "The refresh endpoint validates exp with a 30s tolerance...",
      "source": { "id": 42, "type": "memory" }
    },
    {
      "score": 0.87,
      "kind": "symbol",
      "topic": "JwtService",
      "title": "pub fn verify_token(token: &str) -> Result<Claims>",
      "preview": "src/auth/jwt.rs [37:52]",
      "source": { "id": 156, "type": "symbol" }
    }
  ],
  "search_mode": "keyword"   // or "semantic" if RAG is enabled
}
```

#### `get_symbols`

Find symbols by name, kind, or file.

**Input:**
```json
{
  "project": "my-api",
  "name": "User",            // optional: substring match
  "kind": "struct",          // optional: filter by kind
  "file": "src/models/",     // optional: filter by file path prefix
  "limit": 50
}
```

### Memory Tools

#### `write_memory`

Agent writes a new memory entry.

**Input:**
```json
{
  "topic": "JWT",
  "title": "Clock skew causes silent 401",
  "body": "The refresh endpoint validates exp with a 30s tolerance...",
  "tags": ["gotcha", "auth"]
}
```

**Output:** `{ "id": 42 }`

#### `update_memory`

Update an existing memory entry. Clears stale flag.

**Input:**
```json
{
  "id": 42,
  "body": "Updated: the tolerance was increased to 60s in commit abc123...",
  "tags": ["gotcha", "auth", "resolved"]
}
```

#### `delete_memory`

**Input:** `{ "id": 42 }`

#### `search_memory`

Search memory specifically (as opposed to `query` which searches everything).

**Input:**
```json
{
  "text": "JWT",
  "project": "my-api",
  "tags": ["gotcha"],
  "include_stale": true
}
```

### Indexing Tools

#### `index`

Trigger a (re-)index of a project.

**Input:**
```json
{
  "root_path": "/Users/jacob/code/my-api",
  "name": "my-api"           // optional: defaults to directory name
}
```

**Output:**
```json
{
  "project": "my-api",
  "files_indexed": 342,
  "files_changed": 5,
  "files_removed": 1,
  "symbols_extracted": 1247,
  "rooms_created": 2,
  "stale_memory_flagged": 3,
  "duration_ms": 1200
}
```

#### `diff`

Show what changed since last index.

**Input:** `{ "project": "my-api" }`

**Output:**
```json
{
  "head_commit": "def5678",
  "indexed_commit": "abc1234",
  "changed_files": ["src/auth/jwt.rs", "src/api/handlers/auth.rs"],
  "new_files": ["src/auth/oauth.rs"],
  "removed_files": [],
  "stale_memory": [
    { "id": 42, "title": "Clock skew causes silent 401", "reason": "src/auth/jwt.rs changed" }
  ]
}
```

### Graph Tools

#### `get_imports`

**Input:** `{ "project": "my-api", "file": "src/auth/jwt.rs" }`

#### `get_importers`

**Input:** `{ "project": "my-api", "file": "src/auth/jwt.rs" }`

#### `get_dependencies`

**Input:** `{ "project": "my-api", "from": "src/main.rs", "depth": 3 }`

#### `get_impact`

What would be affected by changing this file/symbol.

**Input:** `{ "project": "my-api", "file": "src/auth/jwt.rs" }`

## Error Handling

All tools return errors in a consistent format:

```json
{
  "error": {
    "code": "not_found",
    "message": "Project 'unknown' not found. Available: my-api, frontend"
  }
}
```

Error codes: `not_found`, `invalid_input`, `index_required`, `db_error`.

## MCP Resources (Read-Only Data)

In addition to tools, rexicon exposes MCP resources for passive context:

| Resource URI | Content |
|---|---|
| `rexicon://projects` | List of all projects (always current) |
| `rexicon://project/{name}/overview` | Project architecture summary |
| `rexicon://project/{name}/rooms` | Room listing |
| `rexicon://project/{name}/symbols` | Full symbol tree (equivalent to rexicon.txt) |

Resources are read-only and can be attached to Claude's context automatically.

## Implementation

The MCP server is implemented using the MCP protocol over stdio (primary) or TCP (optional). The Rust implementation will use:

- `tokio` for async runtime
- An MCP SDK or hand-rolled JSON-RPC 2.0 handler (evaluate `rmcp` crate vs. custom)
- Same `rusqlite` connection as the CLI (shared DB, no conflicts since SQLite handles concurrent reads)

The server is stateless between requests — each tool call opens a read/write transaction, executes, and returns. The database is the only state.
