use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: i32 = 1;

const MIGRATIONS: &[&str] = &[
    // Version 1: initial schema
    r#"
    CREATE TABLE IF NOT EXISTS projects (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        name          TEXT UNIQUE NOT NULL,
        root_path     TEXT NOT NULL,
        tech_stack    TEXT,
        architecture  TEXT,
        entry_points  TEXT,
        head_commit   TEXT,
        last_indexed  TEXT NOT NULL,
        created_at    TEXT NOT NULL,
        updated_at    TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS rooms (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id      INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name            TEXT NOT NULL,
        path            TEXT,
        summary         TEXT,
        parent_room_id  INTEGER REFERENCES rooms(id) ON DELETE CASCADE,
        created_at      TEXT NOT NULL,
        updated_at      TEXT NOT NULL,
        UNIQUE(project_id, name)
    );
    CREATE INDEX IF NOT EXISTS idx_rooms_project ON rooms(project_id);

    CREATE TABLE IF NOT EXISTS topics (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        room_id     INTEGER NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
        name        TEXT NOT NULL,
        kind        TEXT NOT NULL,
        summary     TEXT,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        UNIQUE(room_id, name)
    );
    CREATE INDEX IF NOT EXISTS idx_topics_room ON topics(room_id);

    CREATE TABLE IF NOT EXISTS content (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        scope_id      INTEGER REFERENCES topics(id) ON DELETE CASCADE,
        kind          TEXT NOT NULL,
        body          TEXT NOT NULL,
        source_file   TEXT,
        line_start    INTEGER,
        line_end      INTEGER,
        language      TEXT,
        created_at    TEXT NOT NULL,
        updated_at    TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_content_topic ON content(scope_id);
    CREATE INDEX IF NOT EXISTS idx_content_source ON content(source_file);

    CREATE TABLE IF NOT EXISTS symbols (
        id                INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id        INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        content_id        INTEGER REFERENCES content(id) ON DELETE SET NULL,
        kind              TEXT NOT NULL,
        signature         TEXT NOT NULL,
        name              TEXT NOT NULL,
        file_path         TEXT NOT NULL,
        line_start        INTEGER NOT NULL,
        line_end          INTEGER NOT NULL,
        parent_symbol_id  INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
        created_at        TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_symbols_project ON symbols(project_id);
    CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
    CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_path);
    CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);

    CREATE TABLE IF NOT EXISTS relationships (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        source_file TEXT NOT NULL,
        target      TEXT NOT NULL,
        target_file TEXT,
        kind        TEXT NOT NULL,
        source_line INTEGER,
        metadata    TEXT,
        created_at  TEXT NOT NULL,
        UNIQUE(project_id, source_file, target, kind)
    );
    CREATE INDEX IF NOT EXISTS idx_rel_source ON relationships(project_id, source_file);
    CREATE INDEX IF NOT EXISTS idx_rel_target ON relationships(project_id, target_file);

    CREATE TABLE IF NOT EXISTS memory_scopes (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name        TEXT NOT NULL,
        created_at  TEXT NOT NULL,
        UNIQUE(project_id, name)
    );
    CREATE INDEX IF NOT EXISTS idx_memscopes_project ON memory_scopes(project_id);

    CREATE TABLE IF NOT EXISTS memory (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        scope_id    INTEGER NOT NULL REFERENCES memory_scopes(id) ON DELETE CASCADE,
        title       TEXT NOT NULL,
        body        TEXT NOT NULL,
        tags        TEXT,
        author      TEXT NOT NULL DEFAULT 'claude',
        stale       INTEGER NOT NULL DEFAULT 0,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_memory_scope ON memory(scope_id);

    CREATE TABLE IF NOT EXISTS file_index (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        file_path   TEXT NOT NULL,
        file_hash   TEXT NOT NULL,
        language    TEXT,
        indexed_at  TEXT NOT NULL,
        UNIQUE(project_id, file_path)
    );
    CREATE INDEX IF NOT EXISTS idx_fileindex_project ON file_index(project_id);
    "#,
];

pub fn default_db_path() -> Result<PathBuf> {
    let dir = dirs::home_dir()
        .context("cannot determine home directory")?
        .join(".rexicon");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("store.db"))
}

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_default() -> Result<Connection> {
    let path = default_db_path()?;
    open(&path)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")?;

    let current: Option<i32> = conn
        .query_row(
            "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();
    let current = current.unwrap_or(0);

    for (i, migration) in MIGRATIONS.iter().enumerate() {
        let ver = (i as i32) + 1;
        if ver > current {
            conn.execute_batch(migration)
                .with_context(|| format!("migration v{ver} failed"))?;
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [ver],
            )?;
        }
    }

    assert!(
        MIGRATIONS.len() as i32 == SCHEMA_VERSION,
        "SCHEMA_VERSION does not match MIGRATIONS count"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        migrate(&conn).unwrap();

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
