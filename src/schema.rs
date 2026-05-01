use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

fn timestamp() -> String {
    Utc::now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// Project
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    pub tech_stack: Option<Vec<String>>,
    pub architecture: Option<String>,
    pub entry_points: Option<Vec<String>>,
    pub head_commit: Option<String>,
    pub last_indexed: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn upsert_project(
    conn: &Connection,
    name: &str,
    root_path: &str,
    head_commit: Option<&str>,
) -> Result<i64> {
    let ts = timestamp();
    conn.execute(
        "INSERT INTO projects (name, root_path, head_commit, last_indexed, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(name) DO UPDATE SET
           root_path = excluded.root_path,
           head_commit = excluded.head_commit,
           last_indexed = excluded.last_indexed,
           updated_at = excluded.updated_at",
        params![name, root_path, head_commit, &ts, &ts, &ts],
    )?;
    let id = conn.query_row(
        "SELECT id FROM projects WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;
    Ok(id)
}

pub fn get_project_by_id(conn: &Connection, id: i64) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, root_path, tech_stack, architecture, entry_points,
                head_commit, last_indexed, created_at, updated_at
         FROM projects WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_project(row)?)),
        None => Ok(None),
    }
}

pub fn get_project_by_name(conn: &Connection, name: &str) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, root_path, tech_stack, architecture, entry_points,
                head_commit, last_indexed, created_at, updated_at
         FROM projects WHERE name = ?1",
    )?;
    let mut rows = stmt.query(params![name])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_project(row)?)),
        None => Ok(None),
    }
}

pub fn list_projects(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, root_path, tech_stack, architecture, entry_points,
                head_commit, last_indexed, created_at, updated_at
         FROM projects ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| Ok(row_to_project(row).unwrap()))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn update_project_architecture(
    conn: &Connection,
    project_id: i64,
    architecture: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE projects SET architecture = ?1, updated_at = ?2 WHERE id = ?3",
        params![architecture, timestamp(), project_id],
    )?;
    Ok(())
}

pub fn update_project_tech_stack(
    conn: &Connection,
    project_id: i64,
    tech_stack: &[String],
) -> Result<()> {
    let json = serde_json::to_string(tech_stack)?;
    conn.execute(
        "UPDATE projects SET tech_stack = ?1, updated_at = ?2 WHERE id = ?3",
        params![json, timestamp(), project_id],
    )?;
    Ok(())
}

fn row_to_project(row: &rusqlite::Row) -> Result<Project> {
    let tech_stack_raw: Option<String> = row.get(3)?;
    let entry_points_raw: Option<String> = row.get(5)?;
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        tech_stack: tech_stack_raw.and_then(|s| serde_json::from_str(&s).ok()),
        architecture: row.get(4)?,
        entry_points: entry_points_raw.and_then(|s| serde_json::from_str(&s).ok()),
        head_commit: row.get(6)?,
        last_indexed: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn get_project(conn: &Connection, input: &str) -> Result<Project> {
    if let Ok(id) = input.parse::<i64>() {
        get_project_by_id(conn, id)?
            .ok_or_else(|| anyhow::anyhow!("project #{id} not found"))
    } else {
        get_project_by_name(conn, input)?
            .ok_or_else(|| anyhow::anyhow!("project '{}' not found", input))
    }
}

// ---------------------------------------------------------------------------
// Room
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub path: Option<String>,
    pub summary: Option<String>,
    pub parent_room_id: Option<i64>,
}

pub fn upsert_room(
    conn: &Connection,
    project_id: i64,
    name: &str,
    path: Option<&str>,
    parent_room_id: Option<i64>,
) -> Result<i64> {
    let ts = timestamp();
    conn.execute(
        "INSERT INTO rooms (project_id, name, path, parent_room_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(project_id, name) DO UPDATE SET
           path = excluded.path,
           parent_room_id = excluded.parent_room_id,
           updated_at = excluded.updated_at",
        params![project_id, name, path, parent_room_id, &ts, &ts],
    )?;
    let id = conn.query_row(
        "SELECT id FROM rooms WHERE project_id = ?1 AND name = ?2",
        params![project_id, name],
        |row| row.get(0),
    )?;
    Ok(id)
}

fn row_to_room(row: &rusqlite::Row) -> rusqlite::Result<Room> {
    Ok(Room {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        path: row.get(3)?,
        summary: row.get(4)?,
        parent_room_id: row.get(5)?,
    })
}

pub fn list_rooms(conn: &Connection, project_id: i64) -> Result<Vec<Room>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name, path, summary, parent_room_id
         FROM rooms WHERE project_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map(params![project_id], row_to_room)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_room_by_id(conn: &Connection, id: i64) -> Result<Option<Room>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name, path, summary, parent_room_id
         FROM rooms WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_room(row)?)),
        None => Ok(None),
    }
}

pub fn delete_room_by_id(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM rooms WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_room_by_name(
    conn: &Connection,
    project_id: i64,
    name: &str,
) -> Result<Option<Room>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name, path, summary, parent_room_id
         FROM rooms WHERE project_id = ?1 AND name = ?2",
    )?;
    let mut rows = stmt.query(params![project_id, name])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_room(row)?)),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Topic
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: i64,
    pub room_id: i64,
    pub name: String,
    pub kind: String,
    pub summary: Option<String>,
}

pub fn upsert_topic(
    conn: &Connection,
    room_id: i64,
    name: &str,
    kind: &str,
) -> Result<i64> {
    let ts = timestamp();
    conn.execute(
        "INSERT INTO topics (room_id, name, kind, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(room_id, name) DO UPDATE SET
           kind = excluded.kind,
           updated_at = excluded.updated_at",
        params![room_id, name, kind, &ts, &ts],
    )?;
    let id = conn.query_row(
        "SELECT id FROM topics WHERE room_id = ?1 AND name = ?2",
        params![room_id, name],
        |row| row.get(0),
    )?;
    Ok(id)
}

pub fn list_topics(conn: &Connection, room_id: i64) -> Result<Vec<Topic>> {
    let mut stmt = conn.prepare(
        "SELECT id, room_id, name, kind, summary
         FROM topics WHERE room_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map(params![room_id], |row| {
        Ok(Topic {
            id: row.get(0)?,
            room_id: row.get(1)?,
            name: row.get(2)?,
            kind: row.get(3)?,
            summary: row.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn delete_topic_by_id(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM topics WHERE id = ?1", params![id])?;
    Ok(())
}


// ---------------------------------------------------------------------------
// Symbol (DB)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSymbol {
    pub id: i64,
    pub project_id: i64,
    pub content_id: Option<i64>,
    pub kind: String,
    pub signature: String,
    pub name: String,
    pub file_path: String,
    pub line_start: i64,
    pub line_end: i64,
    pub parent_symbol_id: Option<i64>,
}

#[allow(clippy::too_many_arguments)]
pub fn insert_symbol(
    conn: &Connection,
    project_id: i64,
    kind: &str,
    signature: &str,
    name: &str,
    file_path: &str,
    line_start: i64,
    line_end: i64,
    parent_symbol_id: Option<i64>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO symbols (project_id, kind, signature, name, file_path, line_start, line_end, parent_symbol_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![project_id, kind, signature, name, file_path, line_start, line_end, parent_symbol_id, timestamp()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_symbols_for_file(conn: &Connection, project_id: i64, file_path: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM symbols WHERE project_id = ?1 AND file_path = ?2",
        params![project_id, file_path],
    )?;
    Ok(())
}

pub fn list_symbols_for_file(conn: &Connection, project_id: i64, file_path: &str) -> Result<Vec<DbSymbol>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, content_id, kind, signature, name, file_path, line_start, line_end, parent_symbol_id
         FROM symbols WHERE project_id = ?1 AND file_path = ?2
         ORDER BY line_start",
    )?;
    let rows = stmt.query_map(params![project_id, file_path], |row| {
        Ok(DbSymbol {
            id: row.get(0)?,
            project_id: row.get(1)?,
            content_id: row.get(2)?,
            kind: row.get(3)?,
            signature: row.get(4)?,
            name: row.get(5)?,
            file_path: row.get(6)?,
            line_start: row.get(7)?,
            line_end: row.get(8)?,
            parent_symbol_id: row.get(9)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_symbols_for_project(conn: &Connection, project_id: i64) -> Result<Vec<DbSymbol>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, content_id, kind, signature, name, file_path, line_start, line_end, parent_symbol_id
         FROM symbols WHERE project_id = ?1
         ORDER BY file_path, line_start",
    )?;
    let rows = stmt.query_map(params![project_id], |row| {
        Ok(DbSymbol {
            id: row.get(0)?,
            project_id: row.get(1)?,
            content_id: row.get(2)?,
            kind: row.get(3)?,
            signature: row.get(4)?,
            name: row.get(5)?,
            file_path: row.get(6)?,
            line_start: row.get(7)?,
            line_end: row.get(8)?,
            parent_symbol_id: row.get(9)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn count_symbols(conn: &Connection, project_id: i64) -> Result<i64> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM symbols WHERE project_id = ?1",
        params![project_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// File Index (for incremental hashing)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FileIndexRow {
    pub file_path: String,
    pub file_hash: String,
    pub language: Option<String>,
}

pub fn get_file_hashes(conn: &Connection, project_id: i64) -> Result<Vec<FileIndexRow>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, file_hash, language FROM file_index WHERE project_id = ?1",
    )?;
    let rows = stmt.query_map(params![project_id], |row| {
        Ok(FileIndexRow {
            file_path: row.get(0)?,
            file_hash: row.get(1)?,
            language: row.get(2)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn upsert_file_hash(
    conn: &Connection,
    project_id: i64,
    file_path: &str,
    file_hash: &str,
    language: Option<&str>,
) -> Result<()> {
    let ts = timestamp();
    conn.execute(
        "INSERT INTO file_index (project_id, file_path, file_hash, language, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(project_id, file_path) DO UPDATE SET
           file_hash = excluded.file_hash,
           language = excluded.language,
           indexed_at = excluded.indexed_at",
        params![project_id, file_path, file_hash, language, ts],
    )?;
    Ok(())
}

pub fn delete_file_hash(conn: &Connection, project_id: i64, file_path: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM file_index WHERE project_id = ?1 AND file_path = ?2",
        params![project_id, file_path],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: i64,
    pub project_id: i64,
    pub source_file: String,
    pub target: String,
    pub target_file: Option<String>,
    pub kind: String,
    pub source_line: Option<i64>,
    pub metadata: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn insert_relationship(
    conn: &Connection,
    project_id: i64,
    source_file: &str,
    target: &str,
    target_file: Option<&str>,
    kind: &str,
    source_line: Option<i64>,
    metadata: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO relationships (project_id, source_file, target, target_file, kind, source_line, metadata, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(project_id, source_file, target, kind) DO UPDATE SET
           target_file = excluded.target_file,
           source_line = excluded.source_line,
           metadata = excluded.metadata",
        params![project_id, source_file, target, target_file, kind, source_line, metadata, timestamp()],
    )?;
    Ok(())
}

pub fn delete_relationships_for_file(conn: &Connection, project_id: i64, source_file: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM relationships WHERE project_id = ?1 AND source_file = ?2",
        params![project_id, source_file],
    )?;
    Ok(())
}

pub fn get_children(conn: &Connection, project_id: i64, file_path: &str) -> Result<Vec<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, source_file, target, target_file, kind, source_line, metadata
         FROM relationships WHERE project_id = ?1 AND source_file = ?2
         ORDER BY source_line, target",
    )?;
    let rows = stmt.query_map(params![project_id, file_path], row_to_relationship)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_parents(conn: &Connection, project_id: i64, file_path: &str) -> Result<Vec<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, source_file, target, target_file, kind, source_line, metadata
         FROM relationships WHERE project_id = ?1 AND target_file = ?2
         ORDER BY source_file",
    )?;
    let rows = stmt.query_map(params![project_id, file_path], row_to_relationship)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_all_relationships(conn: &Connection, project_id: i64) -> Result<Vec<Relationship>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, source_file, target, target_file, kind, source_line, metadata
         FROM relationships WHERE project_id = ?1
         ORDER BY source_file, target",
    )?;
    let rows = stmt.query_map(params![project_id], row_to_relationship)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn row_to_relationship(row: &rusqlite::Row) -> rusqlite::Result<Relationship> {
    Ok(Relationship {
        id: row.get(0)?,
        project_id: row.get(1)?,
        source_file: row.get(2)?,
        target: row.get(3)?,
        target_file: row.get(4)?,
        kind: row.get(5)?,
        source_line: row.get(6)?,
        metadata: row.get(7)?,
    })
}

// ---------------------------------------------------------------------------
// Memory Scopes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryScope {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
}

pub fn upsert_memory_scope(conn: &Connection, project_id: i64, name: &str) -> Result<i64> {
    let ts = timestamp();
    conn.execute(
        "INSERT INTO memory_scopes (project_id, name, created_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(project_id, name) DO NOTHING",
        params![project_id, name, ts],
    )?;
    let id = conn.query_row(
        "SELECT id FROM memory_scopes WHERE project_id = ?1 AND name = ?2",
        params![project_id, name],
        |row| row.get(0),
    )?;
    Ok(id)
}

pub fn list_memory_scopes(conn: &Connection, project_id: i64) -> Result<Vec<MemoryScope>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name FROM memory_scopes WHERE project_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map(params![project_id], |row| {
        Ok(MemoryScope {
            id: row.get(0)?,
            project_id: row.get(1)?,
            name: row.get(2)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_memory_scope_by_name(conn: &Connection, project_id: i64, name: &str) -> Result<Option<MemoryScope>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name FROM memory_scopes WHERE project_id = ?1 AND name = ?2",
    )?;
    let mut rows = stmt.query(params![project_id, name])?;
    match rows.next()? {
        Some(row) => Ok(Some(MemoryScope {
            id: row.get(0)?,
            project_id: row.get(1)?,
            name: row.get(2)?,
        })),
        None => Ok(None),
    }
}

pub fn get_memory_scope_by_id(conn: &Connection, id: i64) -> Result<Option<MemoryScope>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name FROM memory_scopes WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(MemoryScope {
            id: row.get(0)?,
            project_id: row.get(1)?,
            name: row.get(2)?,
        })),
        None => Ok(None),
    }
}

pub fn delete_memory_scope(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM memory_scopes WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_scope(conn: &Connection, project_id: i64, input: &str) -> Result<MemoryScope> {
    if let Ok(id) = input.parse::<i64>() {
        get_memory_scope_by_id(conn, id)?
            .ok_or_else(|| anyhow::anyhow!("scope #{id} not found"))
    } else {
        get_memory_scope_by_name(conn, project_id, input)?
            .ok_or_else(|| anyhow::anyhow!("scope '{}' not found", input))
    }
}

// ---------------------------------------------------------------------------
// Memory (articles)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub scope_id: i64,
    pub title: String,
    pub body: String,
    pub tags: Option<Vec<String>>,
    pub author: String,
    pub stale: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub fn insert_memory(
    conn: &Connection,
    scope_id: i64,
    title: &str,
    body: &str,
    tags: Option<&[String]>,
    author: &str,
) -> Result<i64> {
    let ts = timestamp();
    let tags_json = tags.map(|t| serde_json::to_string(t).unwrap_or_default());
    conn.execute(
        "INSERT INTO memory (scope_id, title, body, tags, author, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![scope_id, title, body, tags_json, author, &ts, &ts],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_memory_by_scope(conn: &Connection, scope_id: i64) -> Result<Vec<Memory>> {
    let mut stmt = conn.prepare(
        "SELECT id, scope_id, title, body, tags, author, stale, created_at, updated_at
         FROM memory WHERE scope_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map(params![scope_id], row_to_memory)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_memory_by_id(conn: &Connection, id: i64) -> Result<Option<Memory>> {
    let mut stmt = conn.prepare(
        "SELECT id, scope_id, title, body, tags, author, stale, created_at, updated_at
         FROM memory WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_memory(row)?)),
        None => Ok(None),
    }
}

pub fn update_memory(conn: &Connection, id: i64, title: Option<&str>, body: Option<&str>, tags: Option<&[String]>) -> Result<()> {
    let ts = timestamp();
    if let Some(t) = title {
        conn.execute("UPDATE memory SET title = ?1, updated_at = ?2 WHERE id = ?3", params![t, &ts, id])?;
    }
    if let Some(b) = body {
        conn.execute("UPDATE memory SET body = ?1, stale = 0, updated_at = ?2 WHERE id = ?3", params![b, &ts, id])?;
    }
    if let Some(tg) = tags {
        let json = serde_json::to_string(tg)?;
        conn.execute("UPDATE memory SET tags = ?1, updated_at = ?2 WHERE id = ?3", params![json, &ts, id])?;
    }
    Ok(())
}

pub fn delete_memory(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM memory WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn flag_stale_memory(conn: &Connection, project_id: i64) -> Result<u64> {
    let changed = conn.execute(
        "UPDATE memory SET stale = 1, updated_at = ?1
         WHERE scope_id IN (SELECT id FROM memory_scopes WHERE project_id = ?2)
         AND stale = 0",
        params![timestamp(), project_id],
    )?;
    Ok(changed as u64)
}

pub fn search_memory(conn: &Connection, project_id: Option<i64>, query: &str) -> Result<Vec<(Memory, String, String)>> {
    let pattern = format!("%{query}%");
    let sql = match project_id {
        Some(pid) => {
            let mut stmt = conn.prepare(
                "SELECT m.id, m.scope_id, m.title, m.body, m.tags, m.author, m.stale, m.created_at, m.updated_at,
                        mt.name, p.name
                 FROM memory m
                 JOIN memory_scopes mt ON m.scope_id = mt.id
                 JOIN projects p ON mt.project_id = p.id
                 WHERE mt.project_id = ?1 AND (m.title LIKE ?2 OR m.body LIKE ?2)
                 ORDER BY m.created_at",
            )?;
            let rows = stmt.query_map(params![pid, &pattern], |row| {
                Ok((row_to_memory(row)?, row.get::<_, String>(9)?, row.get::<_, String>(10)?))
            })?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        }
        None => {
            "SELECT m.id, m.scope_id, m.title, m.body, m.tags, m.author, m.stale, m.created_at, m.updated_at,
                    mt.name, p.name
             FROM memory m
             JOIN memory_scopes mt ON m.scope_id = mt.id
             JOIN projects p ON mt.project_id = p.id
             WHERE m.title LIKE ?1 OR m.body LIKE ?1
             ORDER BY m.created_at"
        }
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![&pattern], |row| {
        Ok((row_to_memory(row)?, row.get::<_, String>(9)?, row.get::<_, String>(10)?))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
    let tags_raw: Option<String> = row.get(4)?;
    let stale_int: i32 = row.get(6)?;
    Ok(Memory {
        id: row.get(0)?,
        scope_id: row.get(1)?,
        title: row.get(2)?,
        body: row.get(3)?,
        tags: tags_raw.and_then(|s| serde_json::from_str(&s).ok()),
        author: row.get(5)?,
        stale: stale_int != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

// ---------------------------------------------------------------------------
// Stats helpers
// ---------------------------------------------------------------------------

pub fn count_files(conn: &Connection, project_id: i64) -> Result<i64> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM file_index WHERE project_id = ?1",
        params![project_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn count_rooms(conn: &Connection, project_id: i64) -> Result<i64> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM rooms WHERE project_id = ?1",
        params![project_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn count_memory(conn: &Connection, project_id: i64) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory WHERE scope_id IN (SELECT id FROM memory_scopes WHERE project_id = ?1)",
        params![project_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn count_memory_scopes(conn: &Connection, project_id: i64) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_scopes WHERE project_id = ?1",
        params![project_id],
        |row| row.get(0),
    )?;
    Ok(count)
}
