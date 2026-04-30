use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::schema;
use crate::walker::SourceFile;

#[derive(Debug)]
struct RawRef {
    target: String,
    kind: &'static str,
    line: u32,
}

pub fn index_relationships(
    conn: &Connection,
    project_id: i64,
    root: &Path,
    source_files: &[SourceFile],
    rel_files: &[PathBuf],
) -> Result<u64> {
    let file_set: HashSet<String> = rel_files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    let already_extracted: HashSet<String> = source_files
        .iter()
        .map(|sf| sf.rel_path.to_string_lossy().into_owned())
        .collect();

    let mut count = 0u64;
    for sf in source_files {
        let rel_path = sf.rel_path.to_string_lossy().into_owned();
        let source = match std::fs::read(&sf.path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let refs = parse_imports(sf.language.name, &source, &rel_path);
        count += save_relationships(conn, project_id, &rel_path, &refs, &file_set)?;
    }

    // Also extract from non-source files (markdown, config)
    for rel_path in rel_files {
        let rel = rel_path.to_string_lossy().into_owned();
        let ext = rel_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let filename = rel_path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let is_config = matches!(ext, "toml" | "json" | "yaml" | "yml")
            || matches!(filename, "Dockerfile" | "Makefile" | "makefile" | "GNUmakefile");
        let is_markdown = matches!(ext, "md" | "mdx");

        if is_markdown || is_config {
            if already_extracted.contains(&rel) && !is_config {
                continue;
            }

            let abs_path = root.join(rel_path);
            let source = match std::fs::read(&abs_path) {
                Ok(b) => b,
                Err(_) => continue,
            };

            let refs = match ext {
                "md" | "mdx" => parse_markdown_links(&source),
                "toml" => parse_toml_paths(&source, &rel),
                "json" => parse_json_paths(&source, &rel),
                "yaml" | "yml" => parse_yaml_paths(&source, &rel),
                _ if filename.starts_with("Dockerfile") => parse_dockerfile_paths(&source),
                _ if filename.contains("akefile") => parse_makefile_includes(&source),
                _ => vec![],
            };

            if !refs.is_empty() {
                count += save_relationships(conn, project_id, &rel, &refs, &file_set)?;
            }
        }
    }

    Ok(count)
}

fn save_relationships(
    conn: &Connection,
    project_id: i64,
    rel_path: &str,
    refs: &[RawRef],
    file_set: &HashSet<String>,
) -> Result<u64> {
    schema::delete_relationships_for_file(conn, project_id, rel_path)?;
    let mut count = 0u64;
    for r in refs {
        let resolved = match_target_file(&r.target, r.kind, rel_path, file_set);
        schema::insert_relationship(
            conn,
            project_id,
            rel_path,
            &r.target,
            resolved.as_deref(),
            r.kind,
            Some(r.line as i64),
            None,
        )?;
        count += 1;
    }
    Ok(count)
}

fn parse_imports(lang: &str, source: &[u8], rel_path: &str) -> Vec<RawRef> {
    let text = match std::str::from_utf8(source) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let mut refs = match lang {
        "rust" => parse_rust_imports(text),
        "python" => parse_python_imports(text),
        "javascript" | "typescript" => parse_js_ts_imports(text),
        "go" => parse_go_imports(text),
        "java" => parse_java_imports(text),
        "c" | "cpp" => parse_c_includes(text),
        "ruby" => parse_ruby_requires(text),
        "php" => parse_php_imports(text),
        "c_sharp" => parse_csharp_usings(text),
        "swift" => parse_swift_imports(text),
        "scala" => parse_scala_imports(text),
        "lua" => parse_lua_requires(text),
        "zig" => parse_zig_imports(text),
        "shell" => parse_shell_sources(text),
        "markdown" => parse_markdown_links(source),
        _ => vec![],
    };
    // Extract backtick code references from any file with inline code spans
    refs.extend(parse_backtick_references(text, rel_path));
    refs
}

// ---------------------------------------------------------------------------
// Language-specific import extractors
// ---------------------------------------------------------------------------

fn parse_rust_imports(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("use ") {
            let path = rest.trim_end_matches(';').trim();
            if path.contains("::") {
                let first_seg = path.split("::").next().unwrap_or("");
                if matches!(first_seg, "std" | "core" | "alloc") {
                    continue;
                }
                // Expand grouped imports: use foo::{a, b, c} → foo::a, foo::b, foo::c
                if let Some(brace_start) = path.find('{') {
                    let prefix = &path[..brace_start];
                    let inner = path[brace_start + 1..]
                        .trim_end_matches('}')
                        .trim();
                    for item in inner.split(',') {
                        let item = item.trim();
                        if !item.is_empty() {
                            refs.push(RawRef {
                                target: format!("{prefix}{item}"),
                                kind: "imports",
                                line: (i + 1) as u32,
                            });
                        }
                    }
                } else {
                    refs.push(RawRef {
                        target: path.to_string(),
                        kind: "imports",
                        line: (i + 1) as u32,
                    });
                }
            }
        } else {
            // Match both `mod foo;` and `pub mod foo;` and `pub(crate) mod foo;`
            let after_mod = if let Some(rest) = trimmed.strip_prefix("mod ") {
                Some(rest)
            } else if let Some(rest) = trimmed.strip_prefix("pub mod ") {
                Some(rest)
            } else if trimmed.starts_with("pub(") && trimmed.contains("mod ") {
                trimmed.split("mod ").nth(1)
            } else {
                None
            };
            if let Some(rest) = after_mod {
                let mod_name = rest.trim_end_matches(';').trim();
                if !mod_name.contains('{') && !mod_name.is_empty() {
                    refs.push(RawRef {
                        target: format!("mod {mod_name}"),
                        kind: "imports",
                        line: (i + 1) as u32,
                    });
                }
            }
        }
    }
    refs
}

fn parse_python_imports(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("from ") {
            if let Some(module) = rest.split_whitespace().next()
                && (!module.starts_with('.') || module.len() > 1)
            {
                refs.push(RawRef {
                    target: module.to_string(),
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix("import ") {
            for module in rest.split(',') {
                let module = module.split_whitespace().next().unwrap_or("");
                if !module.is_empty() {
                    refs.push(RawRef {
                        target: module.to_string(),
                        kind: "imports",
                        line: (i + 1) as u32,
                    });
                }
            }
        }
    }
    refs
}

fn parse_js_ts_imports(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if (trimmed.starts_with("import ") || trimmed.starts_with("export "))
            && let Some(path) = parse_quoted_after_keyword(trimmed, "from")
        {
            refs.push(RawRef {
                target: path,
                kind: "imports",
                line: (i + 1) as u32,
            });
        }
        // const x = require('...')
        if let Some(start) = trimmed.find("require(") {
            let rest = &trimmed[start + 8..];
            if let Some(path) = parse_first_quoted(rest) {
                refs.push(RawRef {
                    target: path,
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

fn parse_go_imports(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    let mut in_import_block = false;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == "import (" {
            in_import_block = true;
            continue;
        }
        if in_import_block && trimmed == ")" {
            in_import_block = false;
            continue;
        }
        if in_import_block
            && let Some(path) = parse_first_quoted(trimmed)
        {
            refs.push(RawRef {
                target: path,
                kind: "imports",
                line: (i + 1) as u32,
            });
        } else if let Some(rest) = trimmed.strip_prefix("import ")
            && let Some(path) = parse_first_quoted(rest)
        {
            refs.push(RawRef {
                target: path,
                kind: "imports",
                line: (i + 1) as u32,
            });
        }
    }
    refs
}

fn parse_java_imports(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("import ") {
            let path = rest.trim_start_matches("static ").trim_end_matches(';').trim();
            if !path.is_empty() {
                refs.push(RawRef {
                    target: path.to_string(),
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

fn parse_c_includes(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#include") {
            let rest = rest.trim();
            if rest.starts_with('"')
                && let Some(path) = parse_first_quoted(rest)
            {
                refs.push(RawRef {
                    target: path,
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

fn parse_ruby_requires(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        for keyword in &["require_relative", "require"] {
            if let Some(rest) = trimmed.strip_prefix(keyword) {
                let rest = rest.trim();
                if let Some(path) = parse_first_quoted(rest) {
                    refs.push(RawRef {
                        target: path,
                        kind: "imports",
                        line: (i + 1) as u32,
                    });
                    break;
                }
            }
        }
    }
    refs
}

fn parse_php_imports(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("use ") {
            let path = rest.trim_end_matches(';').trim();
            if !path.is_empty() && !path.starts_with('(') {
                refs.push(RawRef {
                    target: path.to_string(),
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        }
        for keyword in &["require", "include", "require_once", "include_once"] {
            if trimmed.contains(keyword)
                && let Some(path) = parse_first_quoted(trimmed)
            {
                refs.push(RawRef {
                    target: path,
                    kind: "imports",
                    line: (i + 1) as u32,
                });
                break;
            }
        }
    }
    refs
}

fn parse_csharp_usings(text: &str) -> Vec<RawRef> {
    parse_simple_imports(text, "using ")
}

fn parse_simple_imports(text: &str, keyword: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(keyword) {
            let val = rest.trim_end_matches(';').trim();
            if !val.is_empty() && !val.starts_with('(') && !val.contains('=') {
                refs.push(RawRef {
                    target: val.to_string(),
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

fn parse_swift_imports(text: &str) -> Vec<RawRef> {
    parse_simple_imports(text, "import ")
}

fn parse_scala_imports(text: &str) -> Vec<RawRef> {
    parse_simple_imports(text, "import ")
}

fn parse_lua_requires(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(start) = trimmed.find("require") {
            let rest = &trimmed[start + 7..];
            let rest = rest.trim().trim_start_matches('(').trim();
            if let Some(path) = parse_first_quoted(rest) {
                refs.push(RawRef {
                    target: path,
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

fn parse_zig_imports(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(start) = trimmed.find("@import(") {
            let rest = &trimmed[start + 8..];
            if let Some(path) = parse_first_quoted(rest) {
                refs.push(RawRef {
                    target: path,
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

fn parse_shell_sources(text: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        for keyword in &["source ", ". "] {
            if let Some(rest) = trimmed.strip_prefix(keyword) {
                let path = rest.split_whitespace().next().unwrap_or("").trim();
                let path = path.trim_matches('"').trim_matches('\'');
                if !path.is_empty() && (path.contains('/') || path.contains('.')) {
                    refs.push(RawRef {
                        target: path.to_string(),
                        kind: "imports",
                        line: (i + 1) as u32,
                    });
                }
                break;
            }
        }
    }
    refs
}

// ---------------------------------------------------------------------------
// Backtick code references (any source file)
// ---------------------------------------------------------------------------

fn parse_backtick_references(text: &str, _rel_path: &str) -> Vec<RawRef> {
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        // Skip comment lines — backtick refs in comments are documentation, not real dependencies
        if trimmed.starts_with("///")
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with('*')
            || trimmed.starts_with("```")
        {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            let after = &rest[start + 1..];
            if let Some(end) = after.find('`') {
                let code = &after[..end];
                // Match file-path-like references: contains / or ends with known extension
                if (code.contains('/') || code.contains('.'))
                    && !code.contains(' ')
                    && !code.starts_with("http")
                    && code.len() > 2
                    && code.len() < 100
                {
                    let has_ext = code.split('.').next_back().is_some_and(|ext| {
                        matches!(
                            ext,
                            "rs" | "py" | "ts" | "js" | "go" | "java" | "rb" | "php"
                                | "c" | "h" | "cpp" | "cs" | "swift" | "scala" | "lua"
                                | "zig" | "sh" | "toml" | "json" | "yaml" | "yml" | "md"
                        )
                    });
                    if has_ext || code.contains('/') {
                        refs.push(RawRef {
                            target: code.to_string(),
                            kind: "references",
                            line: (i + 1) as u32,
                        });
                    }
                }
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    refs
}

// ---------------------------------------------------------------------------
// Markdown references
// ---------------------------------------------------------------------------

fn parse_markdown_links(source: &[u8]) -> Vec<RawRef> {
    let text = match std::str::from_utf8(source) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            if let Some(end) = after.find(')') {
                let target = &after[..end];
                let target = target.trim();
                if !target.is_empty() {
                    let kind = if target.starts_with("http://") || target.starts_with("https://") {
                        "links_to"
                    } else {
                        "references"
                    };
                    refs.push(RawRef {
                        target: target.to_string(),
                        kind,
                        line: (i + 1) as u32,
                    });
                }
                rest = &after[end..];
            } else {
                break;
            }
        }
    }
    refs
}

// ---------------------------------------------------------------------------
// Config file references
// ---------------------------------------------------------------------------

fn parse_toml_paths(source: &[u8], _rel_path: &str) -> Vec<RawRef> {
    let text = match std::str::from_utf8(source) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("path") {
            let rest = rest.trim().strip_prefix('=').unwrap_or("").trim();
            if let Some(path) = parse_first_quoted(rest) {
                refs.push(RawRef {
                    target: path,
                    kind: "config_path",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

fn parse_json_paths(source: &[u8], rel_path: &str) -> Vec<RawRef> {
    let text = match std::str::from_utf8(source) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    if !rel_path.ends_with("package.json") && !rel_path.ends_with("tsconfig.json") {
        return vec![];
    }
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        for key in &["\"main\"", "\"module\"", "\"types\"", "\"typings\"", "\"bin\""] {
            let value_part = trimmed.split_once(':').map(|x| x.1).unwrap_or("");
            if trimmed.starts_with(key)
                && let Some(path) = parse_first_quoted(value_part)
                && (path.ends_with(".js")
                    || path.ends_with(".ts")
                    || path.ends_with(".mjs")
                    || path.ends_with(".cjs"))
            {
                refs.push(RawRef {
                    target: path,
                    kind: "config_path",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

fn parse_yaml_paths(source: &[u8], rel_path: &str) -> Vec<RawRef> {
    let text = match std::str::from_utf8(source) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let is_ci = rel_path.contains(".github/workflows") || rel_path.contains(".gitlab-ci");
    let is_docker_compose = rel_path.contains("docker-compose") || rel_path.contains("compose.y");
    if !is_ci && !is_docker_compose {
        return vec![];
    }
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        // CI: working-directory, paths, file references
        for key in &["working-directory:", "path:", "file:"] {
            if let Some(rest) = trimmed.strip_prefix(key) {
                let val = rest.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() && !val.starts_with('$') && (val.contains('/') || val.contains('.')) {
                    refs.push(RawRef {
                        target: val.to_string(),
                        kind: "config_path",
                        line: (i + 1) as u32,
                    });
                }
            }
        }
        // docker-compose: build context, volumes
        if is_docker_compose {
            if let Some(rest) = trimmed.strip_prefix("build:") {
                let val = rest.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() && val != "." {
                    refs.push(RawRef {
                        target: val.to_string(),
                        kind: "config_path",
                        line: (i + 1) as u32,
                    });
                }
            }
            if trimmed.starts_with("- ") && trimmed.contains(':') && trimmed.contains('/') {
                let vol = trimmed.strip_prefix("- ").unwrap_or(trimmed);
                if let Some((host, _)) = vol.split_once(':') {
                    let host = host.trim().trim_matches('"').trim_matches('\'');
                    if !host.starts_with('$') && (host.starts_with('.') || host.starts_with('/')) {
                        refs.push(RawRef {
                            target: host.to_string(),
                            kind: "config_path",
                            line: (i + 1) as u32,
                        });
                    }
                }
            }
        }
    }
    refs
}

fn parse_dockerfile_paths(source: &[u8]) -> Vec<RawRef> {
    let text = match std::str::from_utf8(source) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        for keyword in &["COPY", "ADD"] {
            if let Some(rest) = trimmed.strip_prefix(keyword) {
                let rest = rest.trim();
                // Skip --from=... flags
                let rest = if rest.starts_with("--") {
                    rest.split_whitespace().skip(1).collect::<Vec<_>>().join(" ")
                } else {
                    rest.to_string()
                };
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() >= 2 {
                    for src in &parts[..parts.len() - 1] {
                        let src = src.trim();
                        if src != "." && !src.starts_with("--") && !src.starts_with("http") {
                            refs.push(RawRef {
                                target: src.to_string(),
                                kind: "config_path",
                                line: (i + 1) as u32,
                            });
                        }
                    }
                }
            }
        }
    }
    refs
}

fn parse_makefile_includes(source: &[u8]) -> Vec<RawRef> {
    let text = match std::str::from_utf8(source) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let mut refs = vec![];
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        // include directives
        if let Some(rest) = trimmed.strip_prefix("include ").or_else(|| trimmed.strip_prefix("-include ")) {
            let path = rest.trim();
            if !path.starts_with('$') {
                refs.push(RawRef {
                    target: path.to_string(),
                    kind: "imports",
                    line: (i + 1) as u32,
                });
            }
        }
    }
    refs
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

fn match_target_file(
    target: &str,
    kind: &str,
    source_file: &str,
    file_set: &HashSet<String>,
) -> Option<String> {
    match kind {
        "links_to" => None,
        "imports" => match_import_to_file(target, source_file, file_set),
        "references" | "config_path" => match_relative_path(target, source_file, file_set),
        _ => None,
    }
}

fn match_import_to_file(target: &str, source_file: &str, file_set: &HashSet<String>) -> Option<String> {
    // Rust: use crate::foo::bar → src/foo/bar.rs or src/foo.rs
    if let Some(rest) = target.strip_prefix("crate::") {
        let rest = rest.split('{').next().unwrap_or(rest).trim_end_matches(',').trim();
        let parts: Vec<&str> = rest.split("::").collect();
        return match_rust_module_path(&parts, file_set);
    }
    // Rust: use <crate_name>::{...} — try stripping the crate name and resolving as crate::
    if target.contains("::") && !target.starts_with("super::") {
        let first_sep = target.find("::").unwrap_or(0);
        let rest = &target[first_sep + 2..];
        let rest = rest.split('{').next().unwrap_or(rest).trim_end_matches(',').trim();
        if !rest.is_empty() {
            let parts: Vec<&str> = rest.split("::").collect();
            if let Some(found) = match_rust_module_path(&parts, file_set) {
                return Some(found);
            }
        }
    }
    if let Some(rest) = target.strip_prefix("super::") {
        let parent = Path::new(source_file).parent()?;
        let grandparent = parent.parent()?;
        let parts: Vec<&str> = rest.split("::").collect();
        let base = grandparent.to_string_lossy();
        for i in (1..=parts.len()).rev() {
            let path = format!("{}/{}.rs", base, parts[..i].join("/"));
            if file_set.contains(&path) {
                return Some(path);
            }
        }
        return None;
    }
    if let Some(rest) = target.strip_prefix("mod ") {
        let parent = Path::new(source_file).parent()?;
        let as_file = format!("{}/{rest}.rs", parent.to_string_lossy());
        if file_set.contains(&as_file) {
            return Some(as_file);
        }
        let as_mod = format!("{}/{rest}/mod.rs", parent.to_string_lossy());
        if file_set.contains(&as_mod) {
            return Some(as_mod);
        }
        return None;
    }

    // JS/TS: relative paths ./foo or ../foo
    if target.starts_with("./") || target.starts_with("../") {
        return match_relative_path(target, source_file, file_set);
    }

    // Python: dots to slashes
    if target.contains('.') && !target.contains('/') && !target.starts_with("http") {
        let as_path = target.replace('.', "/");
        let candidates = [
            format!("{as_path}.py"),
            format!("{as_path}/__init__.py"),
            format!("src/{as_path}.py"),
        ];
        for c in &candidates {
            if file_set.contains(c) {
                return Some(c.clone());
            }
        }
    }

    // Java: dots to slashes (only if looks like a Java package — starts with lowercase or has uppercase class name)
    if target.contains('.') && !target.contains('/') && !target.starts_with("http") {
        let parts: Vec<&str> = target.rsplitn(2, '.').collect();
        if parts.len() == 2 {
            let path = parts[1].replace('.', "/");
            let candidates = [
                format!("{path}/{}.java", parts[0]),
                format!("src/{path}/{}.java", parts[0]),
                format!("src/main/java/{path}/{}.java", parts[0]),
            ];
            for c in &candidates {
                if file_set.contains(c) {
                    return Some(c.clone());
                }
            }
        }
    }

    None
}

fn match_relative_path(
    target: &str,
    source_file: &str,
    file_set: &HashSet<String>,
) -> Option<String> {
    let target = target.split('#').next().unwrap_or(target);
    let target = target.split('?').next().unwrap_or(target);

    let source_dir = Path::new(source_file).parent().unwrap_or(Path::new(""));
    let resolved = source_dir.join(target);
    let normalized = normalize_path(&resolved);
    let norm_str = normalized.to_string_lossy().into_owned();

    if file_set.contains(&norm_str) {
        return Some(norm_str);
    }

    // Try common extensions for JS/TS imports
    for ext in &["", ".ts", ".tsx", ".js", ".jsx", ".mjs", "/index.ts", "/index.js"] {
        let with_ext = format!("{norm_str}{ext}");
        if file_set.contains(&with_ext) {
            return Some(with_ext);
        }
    }

    // Try .rs for Rust
    let with_rs = format!("{norm_str}.rs");
    if file_set.contains(&with_rs) {
        return Some(with_rs);
    }

    // Try .rb for Ruby
    let with_rb = format!("{norm_str}.rb");
    if file_set.contains(&with_rb) {
        return Some(with_rb);
    }

    None
}

fn match_rust_module_path(parts: &[&str], file_set: &HashSet<String>) -> Option<String> {
    for i in (1..=parts.len()).rev() {
        let path = format!("src/{}.rs", parts[..i].join("/"));
        if file_set.contains(&path) {
            return Some(path);
        }
        let mod_path = format!("src/{}/mod.rs", parts[..i].join("/"));
        if file_set.contains(&mod_path) {
            return Some(mod_path);
        }
    }
    None
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {}
            other => result.push(other),
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_quoted_after_keyword(text: &str, keyword: &str) -> Option<String> {
    let idx = text.find(keyword)?;
    let rest = &text[idx + keyword.len()..];
    parse_first_quoted(rest)
}

fn parse_first_quoted(text: &str) -> Option<String> {
    for quote in ['"', '\''] {
        if let Some(start) = text.find(quote) {
            let after = &text[start + 1..];
            if let Some(end) = after.find(quote) {
                let val = &after[..end];
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Graph traversal
// ---------------------------------------------------------------------------

enum Direction {
    Children,
    Parents,
}

pub fn traverse_tree(
    conn: &Connection,
    project_id: i64,
    root_file: &str,
    max_depth: usize,
) -> Result<Vec<(usize, String, bool)>> {
    traverse(conn, project_id, root_file, max_depth, Direction::Children)
}

pub fn traverse_impact(
    conn: &Connection,
    project_id: i64,
    root_file: &str,
    max_depth: usize,
) -> Result<Vec<(usize, String, bool)>> {
    traverse(conn, project_id, root_file, max_depth, Direction::Parents)
}

fn traverse(
    conn: &Connection,
    project_id: i64,
    root_file: &str,
    max_depth: usize,
    direction: Direction,
) -> Result<Vec<(usize, String, bool)>> {
    let mut result = vec![];
    let mut visited = HashSet::new();
    traverse_recursive(conn, project_id, root_file, 0, max_depth, &direction, &mut visited, &mut result)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn traverse_recursive(
    conn: &Connection,
    project_id: i64,
    file: &str,
    depth: usize,
    max_depth: usize,
    direction: &Direction,
    visited: &mut HashSet<String>,
    result: &mut Vec<(usize, String, bool)>,
) -> Result<()> {
    if visited.contains(file) {
        result.push((depth, file.to_string(), true));
        return Ok(());
    }
    visited.insert(file.to_string());
    result.push((depth, file.to_string(), false));

    if depth >= max_depth {
        return Ok(());
    }

    let neighbors: Vec<String> = match direction {
        Direction::Children => schema::get_children(conn, project_id, file)?
            .iter()
            .filter_map(|r| r.target_file.clone())
            .collect(),
        Direction::Parents => schema::get_parents(conn, project_id, file)?
            .iter()
            .map(|r| r.source_file.clone())
            .collect(),
    };

    let mut seen: HashSet<String> = HashSet::new();
    for neighbor in &neighbors {
        if seen.insert(neighbor.clone()) {
            traverse_recursive(conn, project_id, neighbor, depth + 1, max_depth, direction, visited, result)?;
        }
    }
    Ok(())
}
