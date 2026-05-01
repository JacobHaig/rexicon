use anyhow::Result;
use serde_json::{Value, json};
use std::io::{BufRead, Write};

use crate::{db, hierarchy, relationships, schema, walker};

pub fn serve() -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_response(&mut out, json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                }))?;
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => handle_initialize(&id),
            "initialized" => continue,
            "tools/list" => handle_tools_list(&id),
            "tools/call" => handle_tools_call(&id, &request),
            "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
            _ => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            }),
        };

        write_response(&mut out, response)?;
    }

    Ok(())
}

fn write_response(out: &mut impl Write, response: Value) -> Result<()> {
    let msg = serde_json::to_string(&response)?;
    writeln!(out, "{msg}")?;
    out.flush()?;
    Ok(())
}

fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "rexicon",
                "version": "0.2.0"
            }
        }
    })
}

fn handle_tools_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": tool_definitions()
        }
    })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "list_projects",
            "description": "List all indexed projects with file, symbol, and memory counts",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_project",
            "description": "Get project overview: rooms, architecture, memory count",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" }
                },
                "required": ["project"]
            }
        },
        {
            "name": "get_room",
            "description": "Get room detail: files, symbols grouped by file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" },
                    "room": { "type": "string", "description": "Room name" }
                },
                "required": ["project", "room"]
            }
        },
        {
            "name": "query",
            "description": "Search across symbols and memory",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Search text" },
                    "project": { "type": "string", "description": "Scope to project (optional)" },
                    "kind": { "type": "string", "description": "Filter: symbol or memory (optional)" },
                    "limit": { "type": "integer", "description": "Max results (default 10)" }
                },
                "required": ["text"]
            }
        },
        {
            "name": "index",
            "description": "Index or re-index a project directory",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path to index" },
                    "name": { "type": "string", "description": "Project name (optional, defaults to dir name)" },
                    "force": { "type": "boolean", "description": "Force full re-index" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "diff",
            "description": "Show what changed since last index",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" }
                },
                "required": ["project"]
            }
        },
        {
            "name": "get_children",
            "description": "What does this file depend on (direct imports/references)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" },
                    "file": { "type": "string", "description": "File path relative to project root" }
                },
                "required": ["project", "file"]
            }
        },
        {
            "name": "get_parents",
            "description": "What depends on this file (direct importers/referencers)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" },
                    "file": { "type": "string", "description": "File path relative to project root" }
                },
                "required": ["project", "file"]
            }
        },
        {
            "name": "get_tree",
            "description": "Full dependency tree downward from a file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" },
                    "file": { "type": "string", "description": "File path" },
                    "depth": { "type": "integer", "description": "Max depth (default 10)" }
                },
                "required": ["project", "file"]
            }
        },
        {
            "name": "get_impact",
            "description": "Everything affected if this file changes (reverse tree upward)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" },
                    "file": { "type": "string", "description": "File path" },
                    "depth": { "type": "integer", "description": "Max depth (default 10)" }
                },
                "required": ["project", "file"]
            }
        },
        {
            "name": "memory_list",
            "description": "Browse memory: no args = projects, project = scopes, project+scope = articles, project+scope+article = full article",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" },
                    "scope": { "type": "string", "description": "Scope name or ID" },
                    "article": { "type": "string", "description": "Article title or ID" }
                }
            }
        },
        {
            "name": "memory_write",
            "description": "Write a memory article to a project scope",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name" },
                    "scope": { "type": "string", "description": "Scope name (created if new)" },
                    "title": { "type": "string", "description": "Article title" },
                    "body": { "type": "string", "description": "Article body" },
                    "tags": { "type": "string", "description": "Comma-separated tags (optional)" },
                    "author": { "type": "string", "description": "Author name (default: claude)" }
                },
                "required": ["project", "scope", "title", "body"]
            }
        },
        {
            "name": "memory_update",
            "description": "Update an existing memory article",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "description": "Article ID" },
                    "title": { "type": "string", "description": "New title (optional)" },
                    "body": { "type": "string", "description": "New body (optional)" },
                    "tags": { "type": "string", "description": "New tags (optional)" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "memory_delete",
            "description": "Delete a memory article or entire scope",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name or ID" },
                    "scope": { "type": "string", "description": "Scope name or ID" },
                    "article": { "type": "string", "description": "Article title or ID (omit to delete entire scope)" }
                },
                "required": ["project", "scope"]
            }
        },
        {
            "name": "memory_search",
            "description": "Search across all memory by keyword",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search text" },
                    "project": { "type": "string", "description": "Scope to project (optional)" }
                },
                "required": ["query"]
            }
        }
    ])
}

fn handle_tools_call(id: &Value, request: &Value) -> Value {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let result = match tool_name {
        "list_projects" => tool_list_projects(),
        "get_project" => tool_get_project(&args),
        "get_room" => tool_get_room(&args),
        "query" => tool_query(&args),
        "index" => tool_index(&args),
        "diff" => tool_diff(&args),
        "get_children" => tool_get_children(&args),
        "get_parents" => tool_get_parents(&args),
        "get_tree" => tool_get_tree(&args),
        "get_impact" => tool_get_impact(&args),
        "memory_list" => tool_memory_list(&args),
        "memory_write" => tool_memory_write(&args),
        "memory_update" => tool_memory_update(&args),
        "memory_delete" => tool_memory_delete(&args),
        "memory_search" => tool_memory_search(&args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    };

    match result {
        Ok(content) => json!({
            "jsonrpc": "2.0", "id": id,
            "result": {
                "content": [{ "type": "text", "text": content.to_string() }]
            }
        }),
        Err(e) => json!({
            "jsonrpc": "2.0", "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Error: {e}") }],
                "isError": true
            }
        }),
    }
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn arg_i64(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

fn tool_list_projects() -> Result<Value> {
    let conn = db::open_default()?;
    let projects = schema::list_projects(&conn)?;
    let mut result = vec![];
    for p in &projects {
        let files = schema::count_files(&conn, p.id).unwrap_or(0);
        let symbols = schema::count_symbols(&conn, p.id).unwrap_or(0);
        let memory = schema::count_memory(&conn, p.id).unwrap_or(0);
        let scopes = schema::count_memory_scopes(&conn, p.id).unwrap_or(0);
        result.push(json!({
            "id": p.id, "name": p.name, "root_path": p.root_path,
            "files": files, "symbols": symbols, "memory_articles": memory,
            "memory_scopes": scopes, "last_indexed": p.last_indexed,
            "head_commit": p.head_commit
        }));
    }
    Ok(json!(result))
}

fn tool_get_project(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let name = arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?;
    let project = schema::get_project(&conn, name)?;
    let rooms = schema::list_rooms(&conn, project.id)?;
    let mem_scopes = schema::count_memory_scopes(&conn, project.id)?;
    let mem_articles = schema::count_memory(&conn, project.id)?;

    let room_list: Vec<Value> = rooms.iter().map(|r| {
        let topics = schema::list_topics(&conn, r.id).unwrap_or_default();
        json!({
            "name": r.name, "path": r.path, "summary": r.summary,
            "topics": topics.len()
        })
    }).collect();

    Ok(json!({
        "id": project.id, "name": project.name, "root_path": project.root_path,
        "tech_stack": project.tech_stack, "architecture": project.architecture,
        "head_commit": project.head_commit, "last_indexed": project.last_indexed,
        "rooms": room_list,
        "memory_scopes": mem_scopes, "memory_articles": mem_articles
    }))
}

fn tool_get_room(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let project = schema::get_project(&conn, arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?)?;
    let room_name = arg_str(args, "room").ok_or_else(|| anyhow::anyhow!("room required"))?;
    let room = schema::get_room_by_name(&conn, project.id, room_name)?
        .ok_or_else(|| anyhow::anyhow!("room '{}' not found", room_name))?;

    let symbols = schema::list_symbols_for_project(&conn, project.id)?;
    let room_symbols: Vec<Value> = symbols.iter()
        .filter(|s| hierarchy::room_for_file(&s.file_path) == room_name && s.parent_symbol_id.is_none())
        .map(|s| json!({
            "file": s.file_path, "kind": s.kind, "name": s.name,
            "signature": s.signature, "line_start": s.line_start, "line_end": s.line_end
        }))
        .collect();

    Ok(json!({
        "name": room.name, "path": room.path, "summary": room.summary,
        "symbols": room_symbols
    }))
}

fn tool_query(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let text = arg_str(args, "text").ok_or_else(|| anyhow::anyhow!("text required"))?;
    let limit = arg_i64(args, "limit").unwrap_or(10) as usize;
    let kind_filter = arg_str(args, "kind");

    let project_id = match arg_str(args, "project") {
        Some(name) => Some(schema::get_project(&conn, name)?.id),
        None => None,
    };

    let search = format!("%{text}%");
    let mut results: Vec<Value> = vec![];

    if kind_filter.is_none() || kind_filter == Some("symbol") {
        let sql = if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                "SELECT kind, signature, name, file_path, line_start, line_end
                 FROM symbols WHERE project_id = ?1 AND (signature LIKE ?2 OR name LIKE ?2) LIMIT ?3")?;
            let rows = stmt.query_map(rusqlite::params![pid, &search, limit as i64], |row| {
                Ok(json!({
                    "type": "symbol", "kind": row.get::<_, String>(0)?,
                    "signature": row.get::<_, String>(1)?, "name": row.get::<_, String>(2)?,
                    "file": row.get::<_, String>(3)?,
                    "line_start": row.get::<_, i64>(4)?, "line_end": row.get::<_, i64>(5)?
                }))
            })?;
            rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
        } else {
            let mut stmt = conn.prepare(
                "SELECT kind, signature, name, file_path, line_start, line_end
                 FROM symbols WHERE signature LIKE ?1 OR name LIKE ?1 LIMIT ?2")?;
            let rows = stmt.query_map(rusqlite::params![&search, limit as i64], |row| {
                Ok(json!({
                    "type": "symbol", "kind": row.get::<_, String>(0)?,
                    "signature": row.get::<_, String>(1)?, "name": row.get::<_, String>(2)?,
                    "file": row.get::<_, String>(3)?,
                    "line_start": row.get::<_, i64>(4)?, "line_end": row.get::<_, i64>(5)?
                }))
            })?;
            rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
        };
        results.extend(sql);
    }

    if kind_filter.is_none() || kind_filter == Some("memory") {
        let mem = schema::search_memory(&conn, project_id, text)?;
        for (m, scope_name, proj_name) in &mem {
            results.push(json!({
                "type": "memory", "id": m.id, "title": m.title,
                "project": proj_name, "scope": scope_name,
                "stale": m.stale
            }));
            if results.len() >= limit { break; }
        }
    }

    results.truncate(limit);
    Ok(json!(results))
}

fn tool_index(args: &Value) -> Result<Value> {
    let path = arg_str(args, "path").ok_or_else(|| anyhow::anyhow!("path required"))?;
    let root = std::path::Path::new(path).canonicalize()?;
    let name = arg_str(args, "name")
        .or_else(|| root.file_name().and_then(|n| n.to_str()))
        .unwrap_or("project");
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    let conn = db::open_default()?;
    let head = walker::git_head_short(&root);
    let project_id = schema::upsert_project(&conn, name, &root.to_string_lossy(), head.as_deref())?;

    let languages = crate::registry::built_in_languages();
    let empty = globset::GlobSetBuilder::new().build()?;
    let (all_files, source_files) = crate::walker::walk(&root, &languages, None, false, &empty, &empty);

    let existing: std::collections::HashMap<String, String> = if force {
        std::collections::HashMap::new()
    } else {
        schema::get_file_hashes(&conn, project_id)?
            .into_iter().map(|f| (f.file_path, f.file_hash)).collect()
    };

    let current_hashes: std::collections::HashMap<String, String> = source_files.iter()
        .filter_map(|sf| {
            let rel = sf.rel_path.to_string_lossy().into_owned();
            crate::walker::hash_file(&sf.path).map(|h| (rel, h))
        }).collect();

    let files_to_extract: Vec<_> = source_files.iter().filter(|sf| {
        let rel = sf.rel_path.to_string_lossy().into_owned();
        match current_hashes.get(&rel) {
            Some(hash) => match existing.get(&rel) {
                Some(ex) => ex != hash,
                None => true,
            },
            None => true,
        }
    }).collect();

    let changed = files_to_extract.len();

    let indices: Vec<_> = files_to_extract.iter()
        .filter_map(|f| crate::treesitter::extract(f).ok())
        .collect();

    for fi in &indices {
        hierarchy::store_symbols(&conn, project_id, fi)?;
        let fp = fi.rel_path.to_string_lossy();
        if let Some(hash) = current_hashes.get(fp.as_ref()) {
            schema::upsert_file_hash(&conn, project_id, &fp, hash, Some(&fi.language))?;
        }
    }

    let rel_strings: Vec<String> = all_files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    hierarchy::generate_rooms(&conn, project_id, &rel_strings)?;
    let all_indices: Vec<_> = source_files.iter().filter_map(|f| crate::treesitter::extract(f).ok()).collect();
    hierarchy::generate_topics(&conn, project_id, &all_indices)?;
    let rel_count = relationships::index_relationships(&conn, project_id, &root, &source_files, &all_files)?;
    let total_symbols = schema::count_symbols(&conn, project_id)?;

    Ok(json!({
        "project": name, "files_total": all_files.len(),
        "files_changed": changed, "symbols": total_symbols,
        "relationships": rel_count
    }))
}

fn tool_diff(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let project = schema::get_project(&conn, arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?)?;
    let root = std::path::Path::new(&project.root_path);
    if !root.exists() {
        anyhow::bail!("project root no longer exists");
    }

    let existing: std::collections::HashMap<String, String> = schema::get_file_hashes(&conn, project.id)?
        .into_iter().map(|f| (f.file_path, f.file_hash)).collect();

    let languages = crate::registry::built_in_languages();
    let empty = globset::GlobSetBuilder::new().build()?;
    let (_, source_files) = crate::walker::walk(root, &languages, None, false, &empty, &empty);

    let mut changed = vec![];
    let mut added = vec![];
    for sf in &source_files {
        let rel = sf.rel_path.to_string_lossy().into_owned();
        if let Some(hash) = crate::walker::hash_file(&sf.path) {
            match existing.get(&rel) {
                Some(ex) if ex != &hash => changed.push(rel),
                None => added.push(rel),
                _ => {}
            }
        }
    }
    let current: std::collections::HashSet<String> = source_files.iter()
        .map(|sf| sf.rel_path.to_string_lossy().into_owned()).collect();
    let removed: Vec<String> = existing.keys().filter(|k| !current.contains(k.as_str())).cloned().collect();

    Ok(json!({
        "head_commit": walker::git_head_short(root),
        "indexed_commit": project.head_commit,
        "changed": changed, "added": added, "removed": removed
    }))
}

fn tool_get_children(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let project = schema::get_project(&conn, arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?)?;
    let file = arg_str(args, "file").ok_or_else(|| anyhow::anyhow!("file required"))?;
    let rels = schema::get_children(&conn, project.id, file)?;
    let mut seen = std::collections::HashSet::new();
    let children: Vec<Value> = rels.iter()
        .filter(|r| r.target_file.is_some())
        .filter(|r| seen.insert(r.target_file.clone()))
        .map(|r| json!({ "kind": r.kind, "file": r.target_file }))
        .collect();
    Ok(json!(children))
}

fn tool_get_parents(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let project = schema::get_project(&conn, arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?)?;
    let file = arg_str(args, "file").ok_or_else(|| anyhow::anyhow!("file required"))?;
    let rels = schema::get_parents(&conn, project.id, file)?;
    let mut seen = std::collections::HashSet::new();
    let parents: Vec<Value> = rels.iter()
        .filter(|r| seen.insert(r.source_file.clone()))
        .map(|r| json!({ "kind": r.kind, "file": r.source_file }))
        .collect();
    Ok(json!(parents))
}

fn tool_get_tree(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let project = schema::get_project(&conn, arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?)?;
    let file = arg_str(args, "file").ok_or_else(|| anyhow::anyhow!("file required"))?;
    let depth = arg_i64(args, "depth").unwrap_or(10) as usize;
    let tree = relationships::traverse_tree(&conn, project.id, file, depth)?;
    let entries: Vec<Value> = tree.iter().map(|(d, path, cycle)| {
        json!({ "depth": d, "file": path, "already_shown": cycle })
    }).collect();
    Ok(json!(entries))
}

fn tool_get_impact(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let project = schema::get_project(&conn, arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?)?;
    let file = arg_str(args, "file").ok_or_else(|| anyhow::anyhow!("file required"))?;
    let depth = arg_i64(args, "depth").unwrap_or(10) as usize;
    let tree = relationships::traverse_impact(&conn, project.id, file, depth)?;
    let entries: Vec<Value> = tree.iter().map(|(d, path, cycle)| {
        json!({ "depth": d, "file": path, "already_shown": cycle })
    }).collect();
    Ok(json!(entries))
}

fn tool_memory_list(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    match (arg_str(args, "project"), arg_str(args, "scope"), arg_str(args, "article")) {
        (None, None, None) => {
            let projects = schema::list_projects(&conn)?;
            let mut result = vec![];
            for p in &projects {
                let sc = schema::count_memory_scopes(&conn, p.id)?;
                let ac = schema::count_memory(&conn, p.id)?;
                if sc > 0 {
                    result.push(json!({ "id": p.id, "name": p.name, "scopes": sc, "articles": ac }));
                }
            }
            Ok(json!(result))
        }
        (Some(proj_input), None, None) => {
            let project = schema::get_project(&conn, proj_input)?;
            let scopes = schema::list_memory_scopes(&conn, project.id)?;
            let result: Vec<Value> = scopes.iter().map(|s| {
                let articles = schema::list_memory_by_scope(&conn, s.id).unwrap_or_default();
                let stale = articles.iter().filter(|a| a.stale).count();
                json!({ "id": s.id, "name": s.name, "articles": articles.len(), "stale": stale })
            }).collect();
            Ok(json!(result))
        }
        (Some(proj_input), Some(scope_input), None) => {
            let project = schema::get_project(&conn, proj_input)?;
            let scope = schema::get_scope(&conn, project.id, scope_input)?;
            let articles = schema::list_memory_by_scope(&conn, scope.id)?;
            let result: Vec<Value> = articles.iter().map(|a| {
                json!({ "id": a.id, "title": a.title, "tags": a.tags, "stale": a.stale, "author": a.author })
            }).collect();
            Ok(json!(result))
        }
        (Some(proj_input), Some(scope_input), Some(article_input)) => {
            let project = schema::get_project(&conn, proj_input)?;
            let scope = schema::get_scope(&conn, project.id, scope_input)?;
            let articles = schema::list_memory_by_scope(&conn, scope.id)?;
            let article = if let Ok(id) = article_input.parse::<i64>() {
                articles.into_iter().find(|a| a.id == id)
            } else {
                articles.into_iter().find(|a| a.title.to_lowercase() == article_input.to_lowercase())
            };
            match article {
                Some(a) => Ok(json!({
                    "id": a.id, "title": a.title, "body": a.body,
                    "tags": a.tags, "author": a.author, "stale": a.stale,
                    "project": project.name, "scope": scope.name,
                    "created_at": a.created_at, "updated_at": a.updated_at
                })),
                None => anyhow::bail!("article '{}' not found", article_input),
            }
        }
        _ => anyhow::bail!("invalid combination: scope requires project, article requires scope"),
    }
}

fn tool_memory_write(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let project = schema::get_project(&conn, arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?)?;
    let scope_name = arg_str(args, "scope").ok_or_else(|| anyhow::anyhow!("scope required"))?;
    let title = arg_str(args, "title").ok_or_else(|| anyhow::anyhow!("title required"))?;
    let body = arg_str(args, "body").ok_or_else(|| anyhow::anyhow!("body required"))?;
    let author = arg_str(args, "author").unwrap_or("claude");
    let tags: Option<Vec<String>> = arg_str(args, "tags")
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let scope_id = schema::upsert_memory_scope(&conn, project.id, scope_name)?;
    let id = schema::insert_memory(&conn, scope_id, title, body, tags.as_deref(), author)?;

    Ok(json!({ "id": id, "project": project.name, "scope": scope_name, "title": title }))
}

fn tool_memory_update(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let id = arg_i64(args, "id").ok_or_else(|| anyhow::anyhow!("id required"))?;
    let title = arg_str(args, "title");
    let body = arg_str(args, "body");
    let tags: Option<Vec<String>> = arg_str(args, "tags")
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    schema::update_memory(&conn, id, title, body, tags.as_deref())?;
    Ok(json!({ "id": id, "updated": true }))
}

fn tool_memory_delete(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let project = schema::get_project(&conn, arg_str(args, "project").ok_or_else(|| anyhow::anyhow!("project required"))?)?;
    let scope = schema::get_scope(&conn, project.id, arg_str(args, "scope").ok_or_else(|| anyhow::anyhow!("scope required"))?)?;

    match arg_str(args, "article") {
        Some(article_input) => {
            let articles = schema::list_memory_by_scope(&conn, scope.id)?;
            let article = if let Ok(id) = article_input.parse::<i64>() {
                articles.into_iter().find(|a| a.id == id)
            } else {
                articles.into_iter().find(|a| a.title.to_lowercase() == article_input.to_lowercase())
            };
            match article {
                Some(a) => {
                    schema::delete_memory(&conn, a.id)?;
                    Ok(json!({ "deleted": "article", "id": a.id, "title": a.title }))
                }
                None => anyhow::bail!("article '{}' not found", article_input),
            }
        }
        None => {
            let count = schema::list_memory_by_scope(&conn, scope.id)?.len();
            schema::delete_memory_scope(&conn, scope.id)?;
            Ok(json!({ "deleted": "scope", "name": scope.name, "articles_removed": count }))
        }
    }
}

fn tool_memory_search(args: &Value) -> Result<Value> {
    let conn = db::open_default()?;
    let query = arg_str(args, "query").ok_or_else(|| anyhow::anyhow!("query required"))?;
    let project_id = match arg_str(args, "project") {
        Some(name) => Some(schema::get_project(&conn, name)?.id),
        None => None,
    };
    let results = schema::search_memory(&conn, project_id, query)?;
    let entries: Vec<Value> = results.iter().map(|(m, scope_name, proj_name)| {
        json!({ "id": m.id, "title": m.title, "project": proj_name, "scope": scope_name, "stale": m.stale })
    }).collect();
    Ok(json!(entries))
}

