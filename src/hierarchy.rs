use anyhow::Result;
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::schema;
use crate::symbol::{FileIndex, SymbolKind};

/// Generate rooms from the project's directory structure.
/// Each top-level directory (under `src/` if it exists, else project root) becomes a room.
/// Files at the root level go into a `_root` room.
pub fn generate_rooms(
    conn: &Connection,
    project_id: i64,
    all_files: &[String],
) -> Result<()> {
    schema::delete_rooms_for_project(conn, project_id)?;
    let mut dir_set: BTreeSet<String> = BTreeSet::new();
    let mut root_files = false;

    for file in all_files {
        let path = Path::new(file);
        let components: Vec<&str> = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        if components.len() <= 1 {
            root_files = true;
            continue;
        }

        // If first component is "src" and there's a subdir, use the subdir.
        // Otherwise use the first directory component as the room name.
        let room_name = if components[0] == "src" && components.len() > 2 {
            components[1].to_string()
        } else {
            components[0].to_string()
        };

        dir_set.insert(room_name);
    }

    if root_files {
        dir_set.insert("_root".to_string());
    }

    for room_name in &dir_set {
        let room_path = if room_name == "_root" {
            None
        } else {
            // Check if files live under src/<room> or <room>
            let src_path = format!("src/{room_name}");
            let has_src = all_files.iter().any(|f| f.starts_with(&src_path));
            if has_src {
                Some(src_path)
            } else {
                Some(room_name.clone())
            }
        };
        schema::upsert_room(conn, project_id, room_name, room_path.as_deref(), None)?;
    }

    Ok(())
}

/// Generate topics from extracted symbols. Each file with symbols becomes a topic
/// in its room. Significant public symbols (structs, classes, traits, enums) also
/// become topics.
pub fn generate_topics(
    conn: &Connection,
    project_id: i64,
    indices: &[FileIndex],
) -> Result<()> {
    schema::delete_topics_for_project(conn, project_id)?;
    let rooms = schema::list_rooms(conn, project_id)?;
    let room_map: BTreeMap<String, i64> = rooms.iter().map(|r| (r.name.clone(), r.id)).collect();

    for fi in indices {
        let room_name = room_for_file(&fi.rel_path.to_string_lossy());
        let room_id = match room_map.get(&room_name) {
            Some(id) => *id,
            None => continue,
        };

        // File-level topic
        let file_name = fi
            .rel_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        schema::upsert_topic(conn, room_id, file_name, "file")?;

        // Significant symbol topics
        for sym in &fi.symbols {
            let is_significant = matches!(
                sym.kind,
                SymbolKind::Struct
                    | SymbolKind::Class
                    | SymbolKind::Trait
                    | SymbolKind::Interface
                    | SymbolKind::Enum
                    | SymbolKind::Impl
            );
            if is_significant {
                let sym_name = parse_symbol_name(&sym.signature);
                if !sym_name.is_empty() {
                    schema::upsert_topic(conn, room_id, &sym_name, "symbol_group")?;
                }
            }
        }
    }

    Ok(())
}

/// Determine which room a file belongs to based on its relative path.
pub fn room_for_file(rel_path: &str) -> String {
    let path = Path::new(rel_path);
    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if components.len() <= 1 {
        return "_root".to_string();
    }

    // Files in src/<subdir>/ use the subdir as room name (look past src/)
    // Files directly in src/ use "src" as the room
    // Files in any other directory use that directory as the room
    if components[0] == "src" && components.len() > 2 {
        components[1].to_string()
    } else {
        components[0].to_string()
    }
}

/// Extract the primary name from a symbol signature.
/// e.g. "pub struct Config { ... }" → "Config"
/// e.g. "fn main() -> Result<()> { ... }" → "main"
/// e.g. "impl Config { ... }" → "Config"
pub fn parse_symbol_name(signature: &str) -> String {
    let tokens: Vec<&str> = signature.split_whitespace().collect();

    for (i, token) in tokens.iter().enumerate() {
        match *token {
            "fn" | "struct" | "enum" | "trait" | "class" | "interface" | "type" | "module"
            | "object" | "def" | "val" | "var" | "const" | "let" | "static" | "impl" => {
                if let Some(next) = tokens.get(i + 1) {
                    // Take only the leading identifier: strip everything from the first non-ident char
                    let name: String = next
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        return name;
                    }
                }
            }
            _ => {}
        }
    }

    // Fallback: first alphanumeric token
    for token in &tokens {
        let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if !cleaned.is_empty()
            && !matches!(
                cleaned,
                "pub" | "async" | "unsafe" | "extern" | "export" | "abstract" | "final"
                    | "private" | "protected" | "public" | "internal" | "override" | "virtual"
            )
        {
            return cleaned.to_string();
        }
    }

    String::new()
}

/// Store symbols from FileIndex into the database.
pub fn store_symbols(
    conn: &Connection,
    project_id: i64,
    fi: &FileIndex,
) -> Result<()> {
    let file_path = fi.rel_path.to_string_lossy();

    schema::delete_symbols_for_file(conn, project_id, &file_path)?;

    fn insert_recursive(
        conn: &Connection,
        project_id: i64,
        file_path: &str,
        symbols: &[crate::symbol::Symbol],
        parent_id: Option<i64>,
    ) -> Result<()> {
        for sym in symbols {
            let kind_str = symbol_kind_str(sym.kind);
            let name = parse_symbol_name(&sym.signature);
            let id = schema::insert_symbol(
                conn,
                project_id,
                kind_str,
                &sym.signature,
                &name,
                file_path,
                sym.line_start as i64,
                sym.line_end as i64,
                parent_id,
            )?;
            if !sym.children.is_empty() {
                insert_recursive(conn, project_id, file_path, &sym.children, Some(id))?;
            }
        }
        Ok(())
    }

    insert_recursive(conn, project_id, &file_path, &fi.symbols, None)
}

fn symbol_kind_str(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Interface => "interface",
        SymbolKind::Class => "class",
        SymbolKind::Constant => "constant",
        SymbolKind::TypeAlias => "type_alias",
        SymbolKind::Module => "module",
        SymbolKind::Impl => "impl",
        SymbolKind::Variant => "variant",
        SymbolKind::Macro => "macro",
        SymbolKind::Heading(_) => "heading",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_symbol_name() {
        assert_eq!(parse_symbol_name("pub struct Config { ... }"), "Config");
        assert_eq!(parse_symbol_name("fn main() -> Result<()> { ... }"), "main");
        assert_eq!(parse_symbol_name("impl Config { ... }"), "Config");
        assert_eq!(parse_symbol_name("pub fn walk(root: &Path) { ... }"), "walk");
        assert_eq!(parse_symbol_name("class UserService { ... }"), "UserService");
        assert_eq!(parse_symbol_name("def process_payment(amount)"), "process_payment");
    }

    #[test]
    fn test_room_for_file() {
        assert_eq!(room_for_file("src/auth/jwt.rs"), "auth");
        assert_eq!(room_for_file("src/main.rs"), "src");
        assert_eq!(room_for_file("lib/utils.py"), "lib");
        assert_eq!(room_for_file("Cargo.toml"), "_root");
        assert_eq!(room_for_file("tests/languages.rs"), "tests");
    }
}
