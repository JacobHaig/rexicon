# Data Model

## Storage

All data lives in a single SQLite database at `~/.rexicon/store.db`. SQLite is chosen for:
- Single-file portability (copy the DB, you have everything)
- No server process required
- ACID transactions
- Mature Rust bindings (`rusqlite`)
- Vector search extensions available (`sqlite-vss`)

A global config file at `~/.rexicon/config.toml` stores preferences (embedding model, watched projects, MCP server port).

## Schema

### projects

The top of the hierarchy. Each indexed codebase is a project.

```sql
CREATE TABLE projects (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT UNIQUE NOT NULL,
    root_path     TEXT NOT NULL,
    tech_stack    TEXT,                -- JSON array: ["rust", "typescript", "python"]
    architecture  TEXT,                -- auto-generated narrative summary
    entry_points  TEXT,                -- JSON array of file paths
    head_commit   TEXT,                -- git HEAD at last index
    last_indexed  TEXT NOT NULL,       -- ISO 8601 timestamp
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
```

### rooms

Logical domains within a project. Auto-generated from directory structure, can be manually refined.

```sql
CREATE TABLE rooms (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id      INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    path            TEXT,              -- relative dir path (e.g. "src/auth")
    summary         TEXT,              -- auto-generated or agent-written description
    parent_room_id  INTEGER REFERENCES rooms(id) ON DELETE CASCADE,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE(project_id, name)
);

CREATE INDEX idx_rooms_project ON rooms(project_id);
```

### topics

Specific subjects within a room: a symbol, a flow, a pattern, an issue.

```sql
CREATE TABLE topics (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id     INTEGER NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,         -- 'symbol_group', 'flow', 'pattern', 'issue', 'concept'
    summary     TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE(room_id, name)
);

CREATE INDEX idx_topics_room ON topics(room_id);
```

### content

The actual payload. Every piece of indexed or written content is a row here.

```sql
CREATE TABLE content (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    topic_id      INTEGER REFERENCES topics(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL,       -- 'symbol', 'documentation', 'example', 'annotation'
    body          TEXT NOT NULL,
    source_file   TEXT,                -- relative path within project
    line_start    INTEGER,
    line_end      INTEGER,
    language      TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX idx_content_topic ON content(topic_id);
CREATE INDEX idx_content_source ON content(source_file);
```

### symbols

Structured symbol data extracted by tree-sitter. Linked to content rows.

```sql
CREATE TABLE symbols (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id        INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    content_id        INTEGER REFERENCES content(id) ON DELETE SET NULL,
    kind              TEXT NOT NULL,    -- 'function', 'method', 'struct', 'enum', 'class', etc.
    signature         TEXT NOT NULL,    -- full declaration, body elided
    name              TEXT NOT NULL,    -- extracted symbol name for search
    file_path         TEXT NOT NULL,    -- relative path
    line_start        INTEGER NOT NULL,
    line_end          INTEGER NOT NULL,
    parent_symbol_id  INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
    created_at        TEXT NOT NULL
);

CREATE INDEX idx_symbols_project ON symbols(project_id);
CREATE INDEX idx_symbols_name ON symbols(name);
CREATE INDEX idx_symbols_file ON symbols(file_path);
CREATE INDEX idx_symbols_kind ON symbols(kind);
```

### relationships

Edges between symbols or content items. Captures imports, calls, dependencies.

```sql
CREATE TABLE relationships (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    from_symbol INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    to_symbol   INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,         -- 'imports', 'calls', 'implements', 'extends', 'depends_on'
    metadata    TEXT,                  -- JSON for extra context
    created_at  TEXT NOT NULL,
    UNIQUE(from_symbol, to_symbol, kind)
);

CREATE INDEX idx_rel_from ON relationships(from_symbol);
CREATE INDEX idx_rel_to ON relationships(to_symbol);
CREATE INDEX idx_rel_kind ON relationships(kind);
```

### memory_topics

User-defined categories within a project for organizing memory (e.g. "architecture", "gotchas", "conventions"). These are not code paths — they are conceptual groupings chosen by the user.

```sql
CREATE TABLE memory_topics (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    UNIQUE(project_id, name)
);

CREATE INDEX idx_memory_topics_project ON memory_topics(project_id);
```

### memory

Agent-written notes. The core of the "composable memory" system. Each entry belongs to a project and a topic within that project.

```sql
CREATE TABLE memory (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    topic_id    INTEGER NOT NULL REFERENCES memory_topics(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    tags        TEXT,                  -- JSON array: ["auth", "gotcha", "performance"]
    author      TEXT NOT NULL DEFAULT 'claude',
    stale       INTEGER NOT NULL DEFAULT 0,  -- 1 if underlying code changed since write
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_memory_topic ON memory(topic_id);
CREATE INDEX idx_memory_tags ON memory(tags);
CREATE INDEX idx_memory_stale ON memory(stale);
```

### embeddings

Vector embeddings for RAG. Links to any content or memory row.

```sql
CREATE TABLE embeddings (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    source_table  TEXT NOT NULL,       -- 'content', 'memory', 'symbols'
    source_id     INTEGER NOT NULL,
    vector        BLOB NOT NULL,       -- float32 array, dimension depends on model
    model         TEXT NOT NULL,       -- embedding model identifier
    created_at    TEXT NOT NULL,
    UNIQUE(source_table, source_id, model)
);

CREATE INDEX idx_embed_source ON embeddings(source_table, source_id);
```

### file_index

Tracks per-file state for incremental indexing.

```sql
CREATE TABLE file_index (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    file_path   TEXT NOT NULL,         -- relative path
    file_hash   TEXT NOT NULL,         -- SHA-256 of file contents
    language    TEXT,
    indexed_at  TEXT NOT NULL,
    UNIQUE(project_id, file_path)
);

CREATE INDEX idx_fileindex_project ON file_index(project_id);
```

## Memory Organization

Memory is organized as **Project -> Topic -> Article**:

| Level | What it represents |
|---|---|
| Project | The indexed codebase (from the `projects` table) |
| Topic | A user-defined category within the project (from `memory_topics`) |
| Article | An individual memory entry (from `memory`) |

Topics are conceptual categories chosen by the user, such as "architecture", "gotchas", "conventions", or "onboarding". They are not code paths or filesystem directories.

Memory is always project-scoped. There is no cross-project or global scope.

## Staleness Detection

When a file is re-indexed and its content hash has changed:
1. All `symbols` rows for that file are deleted and re-created.
2. All `memory` rows whose topic is associated with symbols in that file have their `stale` flag set to `1`.
3. The agent is informed of stale memory on next access so it can review and update or dismiss.

## Data Lifecycle

| Event | What happens |
|---|---|
| `rexicon index <dir>` | Walk files → hash → extract changed → upsert DB → flag stale memory |
| `rexicon memory add ...` | Insert into `memory` table under a project and topic |
| `rexicon query "..."` | Search across content + memory + symbols (keyword or semantic) |
| `rexicon diff <project>` | Compare current files against `file_index` hashes, report changes |
| Project deleted | `CASCADE` deletes all rooms, topics, content, symbols, relationships, memory_topics, memory |
