use anyhow::Result;
use clap::{Parser, Subcommand};
use globset::{Glob, GlobSetBuilder};
use rayon::prelude::*;
use rexicon::{db, formatter, hierarchy, output, registry, relationships, schema, symbol, treesitter, walker};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "rexicon",
    about = "Local, agent-native project intelligence layer",
    args_conflicts_with_subcommands = true,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    legacy: LegacyArgs,
}

#[derive(clap::Args)]
struct LegacyArgs {
    /// Root directory of the project to index (defaults to current directory)
    #[arg(default_value = ".")]
    target: PathBuf,

    /// Path for the output file (default: <target>/rexicon.txt)
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Include files that would normally be excluded by .gitignore
    #[arg(long)]
    no_ignore: bool,

    /// Only include files/folders matching these patterns (repeatable)
    #[arg(long = "include", value_name = "PATTERN")]
    includes: Vec<String>,

    /// Exclude files/folders matching these patterns (repeatable)
    #[arg(long = "exclude", value_name = "PATTERN")]
    excludes: Vec<String>,

    /// Output format: txt (default box-drawing tree) or plain
    #[arg(long, default_value = "txt")]
    format: Format,
}

#[derive(Subcommand)]
enum Command {
    /// Index a project directory into the database
    Index {
        /// Root directory of the project
        dir: PathBuf,

        /// Project name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,

        /// Include gitignored files
        #[arg(long)]
        no_ignore: bool,

        /// Only include matching paths (repeatable)
        #[arg(long = "include", value_name = "PATTERN")]
        includes: Vec<String>,

        /// Exclude matching paths (repeatable)
        #[arg(long = "exclude", value_name = "PATTERN")]
        excludes: Vec<String>,

        /// Force full re-index (ignore file hashes)
        #[arg(long)]
        force: bool,
    },

    /// List all indexed projects
    List {
        /// Output format
        #[arg(long, default_value = "table")]
        format: OutputFormat,
    },

    /// Navigate the hierarchy: show <project> [room] [topic]
    Show {
        /// Project name
        project: String,

        /// Room name (optional)
        room: Option<String>,

        /// Topic name (optional)
        topic: Option<String>,

        /// Output format
        #[arg(long, default_value = "table")]
        format: OutputFormat,
    },

    /// Search across content, symbols, and memory
    Query {
        /// Search text
        text: String,

        /// Scope to a project
        #[arg(long)]
        project: Option<String>,

        /// Filter by kind: symbol, memory, content
        #[arg(long)]
        kind: Option<String>,

        /// Max results
        #[arg(long, default_value = "10")]
        limit: usize,
    },

    /// Export project data to files
    Export {
        /// Project name
        project: String,

        /// Output format (txt, plain, or md for memory export)
        #[arg(long, default_value = "txt")]
        format: Format,

        /// Output directory or file
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Only export memory entries as markdown
        #[arg(long)]
        memory_only: bool,
    },

    /// Manage agent memory
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },

    /// Show changes since last index
    Diff {
        /// Project name
        project: String,
    },

    /// Query the relationship graph
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },
}

#[derive(Subcommand)]
enum GraphAction {
    /// What does this file depend on (direct)
    #[command(alias = "c")]
    Children {
        /// Project name
        project: String,
        /// File path (relative to project root)
        #[arg(long)]
        file: String,
    },
    /// What depends on this file (direct)
    #[command(alias = "p")]
    Parents {
        /// Project name
        project: String,
        /// File path (relative to project root)
        #[arg(long)]
        file: String,
    },
    /// Full dependency tree downward
    Tree {
        /// Project name
        project: String,
        /// Root file path
        #[arg(long)]
        file: String,
        /// Max depth (default: 10)
        #[arg(long, default_value = "10")]
        depth: usize,
    },
    /// Everything affected if this file changes (reverse tree)
    Impact {
        /// Project name
        project: String,
        /// File path
        #[arg(long)]
        file: String,
        /// Max depth (default: 10)
        #[arg(long, default_value = "10")]
        depth: usize,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Add a memory article to a project scope
    Add {
        /// Project name
        #[arg(short, long)]
        project: String,
        /// Scope name (created if it doesn't exist)
        #[arg(short, long)]
        scope: String,
        /// Article title
        title: String,
        /// Article body
        body: String,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Author name (default: claude)
        #[arg(long, default_value = "claude")]
        author: String,
    },
    /// Browse memory: projects → scopes → articles. Use positional IDs or named flags.
    List {
        /// Positional drill-down: [project] [scope] [article] (by name or ID)
        #[arg(num_args = 0..=3)]
        path: Vec<String>,

        /// Project (name or ID) — shows scopes in that project
        #[arg(short, long)]
        project: Option<String>,
        /// Scope (name or ID) — shows article titles in that scope
        #[arg(short, long)]
        scope: Option<String>,
        /// Article title (name or ID) — shows the full article
        #[arg(short = 'a', long)]
        title: Option<String>,
    },
    /// Update a memory article
    Update {
        /// Article ID
        id: i64,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New body
        #[arg(long)]
        body: Option<String>,
        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
    /// Delete a memory article or entire scope
    Delete {
        /// Positional drill-down: <project> <scope> [article] (by name or ID)
        #[arg(num_args = 2..=3)]
        path: Vec<String>,

        /// Project (name or ID)
        #[arg(short, long)]
        project: Option<String>,
        /// Scope (name or ID)
        #[arg(short, long)]
        scope: Option<String>,
        /// Article (name or ID) — omit to delete entire scope
        #[arg(short = 'a', long)]
        title: Option<String>,
    },
    /// Search across all memory
    Search {
        /// Search query
        query: String,
        /// Scope to a project
        #[arg(short, long)]
        project: Option<String>,
    },
}

#[derive(clap::ValueEnum, Clone, Default)]
enum Format {
    #[default]
    Txt,
    Plain,
}

#[derive(clap::ValueEnum, Clone, Default)]
enum OutputFormat {
    #[default]
    Table,
    Json,
}

fn build_globset(patterns: &[String]) -> Result<globset::GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for raw in patterns {
        let pat = if !raw.contains('*') && !raw.contains('?') && !raw.contains('[') {
            let trimmed = raw.trim_end_matches('/');
            format!("{{{trimmed},{trimmed}/**}}")
        } else {
            raw.clone()
        };
        builder.add(Glob::new(&pat)?);
    }
    Ok(builder.build()?)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Index {
            dir,
            name,
            no_ignore,
            includes,
            excludes,
            force,
        }) => cmd_index(dir, name, no_ignore, &includes, &excludes, force),

        Some(Command::List { format }) => cmd_list(format),

        Some(Command::Show {
            project,
            room,
            topic,
            format,
        }) => cmd_show(&project, room.as_deref(), topic.as_deref(), format),

        Some(Command::Query {
            text,
            project,
            kind,
            limit,
        }) => cmd_query(&text, project.as_deref(), kind.as_deref(), limit),

        Some(Command::Export {
            project,
            format,
            output,
            memory_only,
        }) => {
            if memory_only {
                cmd_export_memory(&project, output)
            } else {
                cmd_export(&project, format, output)
            }
        }

        Some(Command::Memory { action }) => cmd_memory(action),

        Some(Command::Diff { project }) => cmd_diff(&project),

        Some(Command::Graph { action }) => cmd_graph(action),

        None => {
            // Legacy mode: rexicon [dir] [--output ...]
            cmd_legacy(
                cli.legacy.target,
                cli.legacy.output,
                cli.legacy.no_ignore,
                &cli.legacy.includes,
                &cli.legacy.excludes,
                cli.legacy.format,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Legacy mode: walks + extracts + writes rexicon.txt (v1 behavior)
// Also stores into the database for v2 consumers.
// ---------------------------------------------------------------------------

fn cmd_legacy(
    target: PathBuf,
    output_path: Option<PathBuf>,
    no_ignore: bool,
    includes: &[String],
    excludes: &[String],
    format: Format,
) -> Result<()> {
    let root = target.canonicalize()?;
    let output_path = resolve_output_path(&root, output_path.as_ref())?;
    let output_rel = output_path.strip_prefix(&root).ok().map(|p| p.to_owned());

    let languages = registry::built_in_languages();
    let include_set = build_globset(includes)?;
    let exclude_set = build_globset(excludes)?;

    let (all_files, source_files) = walker::walk(
        &root,
        &languages,
        output_rel.as_deref(),
        no_ignore,
        &include_set,
        &exclude_set,
    );

    let mut indices: Vec<symbol::FileIndex> = source_files
        .par_iter()
        .filter_map(|file| match treesitter::extract(file) {
            Ok(index) => Some(index),
            Err(e) => {
                eprintln!("warning: skipping {}: {e}", file.rel_path.display());
                None
            }
        })
        .collect();
    indices.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let project_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let text = match format {
        Format::Txt => formatter::format(&all_files, &indices, project_name),
        Format::Plain => formatter::format_plain(&indices),
    };
    output::write_output(&text, &output_path)?;

    eprintln!(
        "wrote {} ({} files indexed, {} total)",
        output_path.display(),
        indices.len(),
        all_files.len()
    );

    // Also store in database
    if let Ok(conn) = db::open_default() {
        let head = git_head(&root);
        if let Ok(project_id) =
            schema::upsert_project(&conn, project_name, &root.to_string_lossy(), head.as_deref())
        {
            let rel_strings: Vec<String> = all_files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
            let _ = hierarchy::generate_rooms(&conn, project_id, &rel_strings);

            for fi in &indices {
                let _ = hierarchy::store_symbols(&conn, project_id, fi);
                let file_path_str = fi.rel_path.to_string_lossy();
                if let Some(hash) = source_files
                    .iter()
                    .find(|s| s.rel_path == fi.rel_path)
                    .and_then(|s| walker::hash_file(&s.path))
                {
                    let _ = schema::upsert_file_hash(
                        &conn,
                        project_id,
                        &file_path_str,
                        &hash,
                        Some(&fi.language),
                    );
                }
            }
            let _ = hierarchy::generate_topics(&conn, project_id, &indices);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Index command
// ---------------------------------------------------------------------------

fn cmd_index(
    dir: PathBuf,
    name: Option<String>,
    no_ignore: bool,
    includes: &[String],
    excludes: &[String],
    force: bool,
) -> Result<()> {
    let root = dir.canonicalize()?;
    let project_name = name
        .as_deref()
        .or_else(|| root.file_name().and_then(|n| n.to_str()))
        .unwrap_or("project");

    let conn = db::open_default()?;
    let head = git_head(&root);
    let project_id =
        schema::upsert_project(&conn, project_name, &root.to_string_lossy(), head.as_deref())?;

    let languages = registry::built_in_languages();
    let include_set = build_globset(includes)?;
    let exclude_set = build_globset(excludes)?;

    let (all_files, source_files) = walker::walk(
        &root,
        &languages,
        None,
        no_ignore,
        &include_set,
        &exclude_set,
    );

    // Build hash map of existing file hashes for incremental indexing
    let existing_hashes: HashMap<String, String> = if force {
        HashMap::new()
    } else {
        schema::get_file_hashes(&conn, project_id)?
            .into_iter()
            .map(|f| (f.file_path, f.file_hash))
            .collect()
    };

    // Determine which files need re-extraction
    let files_to_extract: Vec<&walker::SourceFile> = source_files
        .iter()
        .filter(|sf| {
            let rel = sf.rel_path.to_string_lossy().into_owned();
            match walker::hash_file(&sf.path) {
                Some(hash) => match existing_hashes.get(&rel) {
                    Some(existing) => existing != &hash,
                    None => true,
                },
                None => true,
            }
        })
        .collect();

    let changed_count = files_to_extract.len();

    // Detect removed files
    let current_files: std::collections::HashSet<String> = source_files
        .iter()
        .map(|sf| sf.rel_path.to_string_lossy().into_owned())
        .collect();
    let removed: Vec<String> = existing_hashes
        .keys()
        .filter(|k| !current_files.contains(k.as_str()))
        .cloned()
        .collect();

    // Clean up removed files
    for path in &removed {
        schema::delete_symbols_for_file(&conn, project_id, path)?;
        schema::delete_file_hash(&conn, project_id, path)?;
    }

    // Extract changed files
    let indices: Vec<symbol::FileIndex> = files_to_extract
        .par_iter()
        .filter_map(|file| match treesitter::extract(file) {
            Ok(index) => Some(index),
            Err(e) => {
                eprintln!("warning: skipping {}: {e}", file.rel_path.display());
                None
            }
        })
        .collect();

    // Flag all memory as stale when files changed
    let stale_count = if !indices.is_empty() {
        schema::flag_stale_memory(&conn, project_id)?
    } else {
        0
    };

    // Store results
    for fi in &indices {
        hierarchy::store_symbols(&conn, project_id, fi)?;
        let file_path_str = fi.rel_path.to_string_lossy();

        // Update file hash
        if let Some(sf) = source_files.iter().find(|s| s.rel_path == fi.rel_path)
            && let Some(hash) = walker::hash_file(&sf.path)
        {
            schema::upsert_file_hash(
                &conn,
                project_id,
                &file_path_str,
                &hash,
                Some(&fi.language),
            )?;
        }
    }

    // Generate hierarchy
    let rel_strings: Vec<String> = all_files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    hierarchy::generate_rooms(&conn, project_id, &rel_strings)?;

    // For topics, we need all indices (not just changed ones)
    // Fetch existing + merge with newly extracted
    let all_source_indices: Vec<symbol::FileIndex> = source_files
        .par_iter()
        .filter_map(|file| treesitter::extract(file).ok())
        .collect();
    hierarchy::generate_topics(&conn, project_id, &all_source_indices)?;

    // Extract relationships (imports, references, config paths)
    let rel_count = relationships::extract_and_store(&conn, project_id, &root, &source_files, &all_files)?;

    let total_symbols = schema::count_symbols(&conn, project_id)?;

    eprintln!(
        "indexed {project_name}: {} files ({changed_count} changed, {} removed), {total_symbols} symbols, {rel_count} relationships{}",
        all_files.len(),
        removed.len(),
        if stale_count > 0 {
            format!(", {stale_count} stale memories flagged")
        } else {
            String::new()
        }
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// List command
// ---------------------------------------------------------------------------

fn cmd_list(format: OutputFormat) -> Result<()> {
    let conn = db::open_default()?;
    let projects = schema::list_projects(&conn)?;

    if projects.is_empty() {
        eprintln!("no projects indexed yet. Run: rexicon index <dir>");
        return Ok(());
    }

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&projects)?;
            println!("{json}");
        }
        OutputFormat::Table => {
            println!(
                "{:<20} {:>6} {:>8} {:>6} LAST INDEXED",
                "PROJECT", "FILES", "SYMBOLS", "MEMORY"
            );
            for p in &projects {
                let files = schema::count_files(&conn, p.id).unwrap_or(0);
                let symbols = schema::count_symbols(&conn, p.id).unwrap_or(0);
                let memory = schema::count_memory(&conn, p.id).unwrap_or(0);
                let indexed = &p.last_indexed[..std::cmp::min(16, p.last_indexed.len())];
                println!(
                    "{:<20} {:>6} {:>8} {:>6} {}",
                    p.name, files, symbols, memory, indexed
                );
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Show command
// ---------------------------------------------------------------------------

fn cmd_show(
    project_name: &str,
    room_name: Option<&str>,
    _scope_name: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let conn = db::open_default()?;
    let project = schema::get_project_by_name(&conn, project_name)?
        .ok_or_else(|| anyhow::anyhow!("project '{}' not found", project_name))?;

    match room_name {
        None => {
            // Project overview
            let rooms = schema::list_rooms(&conn, project.id)?;
            let mem_scope_count = schema::count_memory_scopes(&conn, project.id)?;
            let mem_article_count = schema::count_memory(&conn, project.id)?;

            match format {
                OutputFormat::Json => {
                    let mem_scopes = schema::list_memory_scopes(&conn, project.id)?;
                    let data = serde_json::json!({
                        "project": project,
                        "rooms": rooms,
                        "memory_scopes": mem_scopes,
                    });
                    println!("{}", serde_json::to_string_pretty(&data)?);
                }
                OutputFormat::Table => {
                    let tech = project
                        .tech_stack
                        .as_ref()
                        .map(|t| t.join(", "))
                        .unwrap_or_default();
                    if tech.is_empty() {
                        println!("{}", project.name);
                    } else {
                        println!("{} ({})", project.name, tech);
                    }
                    if let Some(arch) = &project.architecture {
                        println!("{arch}");
                    }
                    println!();

                    if !rooms.is_empty() {
                        println!("Rooms:");
                        for r in &rooms {
                            let topics = schema::list_topics(&conn, r.id)?;
                            let summary = r.summary.as_deref().unwrap_or("");
                            println!(
                                "  {:<20} {:<40} {} topics",
                                r.name,
                                summary,
                                topics.len()
                            );
                        }
                    }

                    if mem_scope_count > 0 {
                        println!(
                            "\nMemory: {} scopes, {} articles (use `rexicon memory list {}` to browse)",
                            mem_scope_count, mem_article_count, project.name
                        );
                    }
                }
            }
        }
        Some(rn) => {
            // Room detail
            let room = schema::get_room_by_name(&conn, project.id, rn)?
                .ok_or_else(|| anyhow::anyhow!("room '{}' not found in project '{}'", rn, project_name))?;
            let topics = schema::list_topics(&conn, room.id)?;

            // Get symbols for files in this room
            let symbols = schema::list_symbols_for_project(&conn, project.id)?;
            let room_symbols: Vec<_> = symbols
                .iter()
                .filter(|s| hierarchy::room_for_file(&s.file_path) == rn)
                .collect();

            match format {
                OutputFormat::Json => {
                    let data = serde_json::json!({
                        "room": room,
                        "topics": topics,
                        "symbols": room_symbols,
                    });
                    println!("{}", serde_json::to_string_pretty(&data)?);
                }
                OutputFormat::Table => {
                    println!("{} / {}", project_name, rn);
                    if let Some(s) = &room.summary {
                        println!("{s}");
                    }
                    if let Some(p) = &room.path {
                        println!("Path: {p}/");
                    }

                    // Files section: list the source files in this room
                    let file_topics: Vec<_> =
                        topics.iter().filter(|t| t.kind == "file").collect();
                    if !file_topics.is_empty() {
                        println!("\nFiles:");
                        for t in &file_topics {
                            println!("  {}", t.name);
                        }
                    }

                    // Symbols grouped by file, top-level only
                    if !room_symbols.is_empty() {
                        // Strip room path prefix from file paths for cleaner display
                        let strip_prefix = room.path.as_deref().map(|p| {
                            let mut s = p.to_string();
                            if !s.ends_with('/') {
                                s.push('/');
                            }
                            s
                        });

                        // Group by file
                        let mut by_file: std::collections::BTreeMap<&str, Vec<&schema::DbSymbol>> =
                            std::collections::BTreeMap::new();
                        for s in &room_symbols {
                            if s.parent_symbol_id.is_none() {
                                by_file.entry(&s.file_path).or_default().push(s);
                            }
                        }

                        println!("\nSymbols:");
                        for (file_path, syms) in &by_file {
                            let display_path = match &strip_prefix {
                                Some(prefix) if file_path.starts_with(prefix.as_str()) => {
                                    &file_path[prefix.len()..]
                                }
                                _ => file_path,
                            };
                            println!("  {}:", display_path);
                            for s in syms {
                                let lines = if s.line_start == s.line_end {
                                    format!("[{}]", s.line_start)
                                } else {
                                    format!("[{}:{}]", s.line_start, s.line_end)
                                };
                                println!("    {:<12} {}", lines, s.signature);
                            }
                        }
                    }

                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Query command
// ---------------------------------------------------------------------------

fn cmd_query(
    text: &str,
    project_name: Option<&str>,
    kind_filter: Option<&str>,
    limit: usize,
) -> Result<()> {
    let conn = db::open_default()?;

    let project_id = match project_name {
        Some(name) => {
            let p = schema::get_project_by_name(&conn, name)?
                .ok_or_else(|| anyhow::anyhow!("project '{}' not found", name))?;
            Some(p.id)
        }
        None => None,
    };

    let search_term = format!("%{text}%");
    let mut results: Vec<(String, String, String, String)> = Vec::new();

    // Search symbols
    if kind_filter.is_none() || kind_filter == Some("symbol") {
        let symbol_results = if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                "SELECT kind, signature, file_path, line_start, line_end
                 FROM symbols WHERE project_id = ?1 AND (signature LIKE ?2 OR name LIKE ?2)
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(rusqlite::params![pid, &search_term, limit as i64], |row| {
                let kind: String = row.get(0)?;
                let sig: String = row.get(1)?;
                let file: String = row.get(2)?;
                let ls: i64 = row.get(3)?;
                let le: i64 = row.get(4)?;
                let lines = if ls == le {
                    format!("[{ls}]")
                } else {
                    format!("[{ls}:{le}]")
                };
                Ok(("symbol".to_string(), kind, sig, format!("{file} {lines}")))
            })?;
            rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
        } else {
            let mut stmt = conn.prepare(
                "SELECT kind, signature, file_path, line_start, line_end
                 FROM symbols WHERE signature LIKE ?1 OR name LIKE ?1
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![&search_term, limit as i64], |row| {
                let kind: String = row.get(0)?;
                let sig: String = row.get(1)?;
                let file: String = row.get(2)?;
                let ls: i64 = row.get(3)?;
                let le: i64 = row.get(4)?;
                let lines = if ls == le {
                    format!("[{ls}]")
                } else {
                    format!("[{ls}:{le}]")
                };
                Ok(("symbol".to_string(), kind, sig, format!("{file} {lines}")))
            })?;
            rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
        };
        results.extend(symbol_results);
    }

    // Search memory
    if kind_filter.is_none() || kind_filter == Some("memory") {
        let mem_results = schema::search_memory(&conn, project_id, text)?;
        for (m, scope_name, proj_name) in &mem_results {
            let stale = if m.stale { " [STALE]" } else { "" };
            results.push((
                "memory".to_string(),
                format!("{proj_name}/{scope_name}"),
                format!("{}{}", m.title, stale),
                String::new(),
            ));
            if results.len() >= limit {
                break;
            }
        }
    }

    results.truncate(limit);

    if results.is_empty() {
        eprintln!("no results for '{text}'");
    } else {
        for (kind, scope, title, loc) in &results {
            let title_trunc = if title.len() > 70 {
                format!("{}...", &title[..67])
            } else {
                title.clone()
            };
            if loc.is_empty() {
                println!("  {:<8} {:<25} {}", kind, scope, title_trunc);
            } else {
                println!("  {:<8} {:<25} {}", kind, loc, title_trunc);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Export command
// ---------------------------------------------------------------------------

fn cmd_export(project_name: &str, format: Format, output_path: Option<PathBuf>) -> Result<()> {
    let conn = db::open_default()?;
    let project = schema::get_project_by_name(&conn, project_name)?
        .ok_or_else(|| anyhow::anyhow!("project '{}' not found", project_name))?;

    let root = PathBuf::from(&project.root_path);
    let languages = registry::built_in_languages();
    let empty_includes = build_globset(&[])?;
    let empty_excludes = build_globset(&[])?;

    let (all_files, source_files) = walker::walk(
        &root,
        &languages,
        None,
        false,
        &empty_includes,
        &empty_excludes,
    );

    let mut indices: Vec<symbol::FileIndex> = source_files
        .par_iter()
        .filter_map(|file| treesitter::extract(file).ok())
        .collect();
    indices.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let text = match format {
        Format::Txt => formatter::format(&all_files, &indices, project_name),
        Format::Plain => formatter::format_plain(&indices),
    };

    let out_path = output_path.unwrap_or_else(|| root.join("rexicon.txt"));
    output::write_output(&text, &out_path)?;
    eprintln!(
        "exported {} ({} files indexed)",
        out_path.display(),
        indices.len()
    );
    Ok(())
}

fn cmd_export_memory(project_name: &str, output_dir: Option<PathBuf>) -> Result<()> {
    let conn = db::open_default()?;
    let project = schema::get_project_by_name(&conn, project_name)?
        .ok_or_else(|| anyhow::anyhow!("project '{}' not found", project_name))?;

    let topics = schema::list_memory_scopes(&conn, project.id)?;
    if topics.is_empty() {
        eprintln!("no memory entries to export for '{project_name}'");
        return Ok(());
    }

    let base = output_dir.unwrap_or_else(|| PathBuf::from(".rexicon-export"));
    let mem_dir = base.join("memory");

    let mut count = 0;
    for t in &topics {
        let safe_name = t.name.replace(' ', "-").to_lowercase();
        let dir = mem_dir.clone();
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join(format!("{safe_name}.md"));

        use std::io::Write;
        let mut f = std::fs::File::create(&file_path)?;
        writeln!(f, "# {}\n", t.name)?;

        let articles = schema::list_memory_by_scope(&conn, t.id)?;
        for a in &articles {
            let tags = a
                .tags
                .as_ref()
                .map(|t| format!("tags: {}", t.join(", ")))
                .unwrap_or_default();
            let stale = if a.stale { " [STALE]" } else { "" };

            writeln!(f, "## {}{}\n", a.title, stale)?;
            writeln!(f, "{}\n", a.body)?;
            if !tags.is_empty() {
                writeln!(f, "_{}_", tags)?;
            }
            writeln!(f, "_author: {}, id: {}_\n", a.author, a.id)?;
            writeln!(f, "---\n")?;
            count += 1;
        }
    }

    eprintln!(
        "exported {count} memory entries to {}",
        base.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Memory command
// ---------------------------------------------------------------------------

fn resolve_project(conn: &rusqlite::Connection, input: &str) -> Result<schema::Project> {
    if let Ok(id) = input.parse::<i64>() {
        let projects = schema::list_projects(conn)?;
        projects
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| anyhow::anyhow!("project #{id} not found"))
    } else {
        schema::get_project_by_name(conn, input)?
            .ok_or_else(|| anyhow::anyhow!("project '{}' not found", input))
    }
}

fn resolve_scope(conn: &rusqlite::Connection, project_id: i64, input: &str) -> Result<schema::MemoryScope> {
    if let Ok(id) = input.parse::<i64>() {
        schema::get_memory_scope_by_id(conn, id)?
            .ok_or_else(|| anyhow::anyhow!("scope #{id} not found"))
    } else {
        schema::get_memory_scope_by_name(conn, project_id, input)?
            .ok_or_else(|| anyhow::anyhow!("scope '{}' not found", input))
    }
}

fn cmd_memory(action: MemoryAction) -> Result<()> {
    let conn = db::open_default()?;

    match action {
        MemoryAction::Add {
            project,
            scope,
            title,
            body,
            tags,
            author,
        } => {
            let proj = resolve_project(&conn, &project)?;
            let topic_id = schema::upsert_memory_scope(&conn, proj.id, &scope)?;
            let tag_list: Option<Vec<String>> = tags.map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .collect()
            });
            let id = schema::insert_memory(
                &conn,
                topic_id,
                &title,
                &body,
                tag_list.as_deref(),
                &author,
            )?;
            println!("memory #{id} created in {project}/{scope}");
        }
        MemoryAction::List {
            path,
            project,
            scope,
            title,
        } => {
            // Merge positional args with named flags. Named flags take priority.
            let project = project.or_else(|| path.first().cloned());
            let scope = scope.or_else(|| path.get(1).cloned());
            let title = title.or_else(|| path.get(2).cloned());

            match (&project, &scope, &title) {
                // Level 1: no args → list projects with memory counts
                (None, None, None) => {
                    let projects = schema::list_projects(&conn)?;
                    let mut found = false;
                    for p in &projects {
                        let scope_count = schema::count_memory_scopes(&conn, p.id)?;
                        let article_count = schema::count_memory(&conn, p.id)?;
                        if scope_count > 0 {
                            found = true;
                            let t_label = if scope_count == 1 { "scope" } else { "scopes" };
                            let a_label = if article_count == 1 { "article" } else { "articles" };
                            println!(
                                "  [{:<3}] {:<25} {} {}, {} {}",
                                p.id, p.name, scope_count, t_label, article_count, a_label
                            );
                        }
                    }
                    if !found {
                        eprintln!("no memory entries yet");
                    }
                }
                // Level 2: project → list topics in that project
                (Some(proj_input), None, None) => {
                    let proj = resolve_project(&conn, proj_input)?;
                    let topics = schema::list_memory_scopes(&conn, proj.id)?;
                    if topics.is_empty() {
                        eprintln!("no memory scopes in '{}'", proj.name);
                    } else {
                        println!("{} — memory scopes:\n", proj.name);
                        for t in &topics {
                            let articles = schema::list_memory_by_scope(&conn, t.id)?;
                            let label = if articles.len() == 1 { "article" } else { "articles" };
                            let stale = articles.iter().filter(|a| a.stale).count();
                            let stale_note = if stale > 0 {
                                format!(" ({stale} stale)")
                            } else {
                                String::new()
                            };
                            println!(
                                "  [{:<3}] {:<30} {} {}{}",
                                t.id, t.name, articles.len(), label, stale_note
                            );
                        }
                    }
                }
                // Level 3: project + scope → list article titles
                (Some(proj_input), Some(scope_input), None) => {
                    let proj = resolve_project(&conn, proj_input)?;
                    let scope = resolve_scope(&conn, proj.id, scope_input)?;
                    let articles = schema::list_memory_by_scope(&conn, scope.id)?;
                    if articles.is_empty() {
                        eprintln!("no articles in '{}/{}'", proj.name, scope.name);
                    } else {
                        println!("{} / {} — articles:\n", proj.name, scope.name);
                        for a in &articles {
                            let stale_tag = if a.stale { " [STALE]" } else { "" };
                            let tags_display = a
                                .tags
                                .as_ref()
                                .map(|t| t.iter().map(|s| format!("#{s}")).collect::<Vec<_>>().join(" "))
                                .unwrap_or_default();
                            println!("  [{:<3}] {}{} {}", a.id, a.title, stale_tag, tags_display);
                        }
                    }
                }
                // Level 4: project + scope + title → show full article
                (Some(proj_input), Some(scope_input), Some(title_input)) => {
                    let proj = resolve_project(&conn, proj_input)?;
                    let scope = resolve_scope(&conn, proj.id, scope_input)?;
                    let articles = schema::list_memory_by_scope(&conn, scope.id)?;

                    let article = if let Ok(id) = title_input.parse::<i64>() {
                        articles.into_iter().find(|a| a.id == id)
                    } else {
                        articles.into_iter().find(|a| a.title.to_lowercase() == title_input.to_lowercase())
                    };

                    match article {
                        Some(a) => {
                            println!("{} / {} / {}\n", proj.name, scope.name, a.title);
                            if a.stale {
                                println!("[STALE] Code has changed since this was written.\n");
                            }
                            println!("{}\n", a.body);
                            if let Some(tags) = &a.tags {
                                println!("tags: {}", tags.join(", "));
                            }
                            println!("author: {}", a.author);
                            println!("created: {}", &a.created_at[..std::cmp::min(16, a.created_at.len())]);
                            println!("updated: {}", &a.updated_at[..std::cmp::min(16, a.updated_at.len())]);
                        }
                        None => eprintln!("article '{}' not found in {}/{}", title_input, proj.name, scope.name),
                    }
                }
                // Partial args that don't make sense
                (None, Some(_), _) => {
                    anyhow::bail!("--scope requires --project");
                }
                (None, None, Some(_)) => {
                    anyhow::bail!("--title requires --project and --scope");
                }
                (Some(_), None, Some(_)) => {
                    anyhow::bail!("--title requires --scope");
                }
            }
        }
        MemoryAction::Update {
            id,
            title,
            body,
            tags,
        } => {
            let tag_list: Option<Vec<String>> = tags.map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .collect()
            });
            schema::update_memory(&conn, id, title.as_deref(), body.as_deref(), tag_list.as_deref())?;
            println!("memory #{id} updated");
        }
        MemoryAction::Delete {
            path,
            project,
            scope,
            title,
        } => {
            let project = project.or_else(|| path.first().cloned());
            let scope = scope.or_else(|| path.get(1).cloned());
            let title = title.or_else(|| path.get(2).cloned());

            let proj_input = project.ok_or_else(|| anyhow::anyhow!("project required: memory delete <project> <scope> [article]"))?;
            let scope_input = scope.ok_or_else(|| anyhow::anyhow!("scope required: memory delete <project> <scope> [article]"))?;

            let proj = resolve_project(&conn, &proj_input)?;
            let scope = resolve_scope(&conn, proj.id, &scope_input)?;

            match title {
                Some(title_input) => {
                    // Delete one article
                    let articles = schema::list_memory_by_scope(&conn, scope.id)?;
                    let article = if let Ok(id) = title_input.parse::<i64>() {
                        articles.into_iter().find(|a| a.id == id)
                    } else {
                        articles.into_iter().find(|a| a.title.to_lowercase() == title_input.to_lowercase())
                    };
                    match article {
                        Some(a) => {
                            schema::delete_memory(&conn, a.id)?;
                            println!("deleted article: {} / {} / {}", proj.name, scope.name, a.title);
                        }
                        None => anyhow::bail!("article '{}' not found in {}/{}", title_input, proj.name, scope.name),
                    }
                }
                None => {
                    // Delete entire scope and all its articles
                    let count = schema::list_memory_by_scope(&conn, scope.id)?.len();
                    schema::delete_memory_scope(&conn, scope.id)?;
                    println!(
                        "deleted scope: {} / {} ({} articles removed)",
                        proj.name, scope.name, count
                    );
                }
            }
        }
        MemoryAction::Search { query, project } => {
            let project_id = match &project {
                Some(name) => {
                    let p = resolve_project(&conn, name)?;
                    Some(p.id)
                }
                None => None,
            };
            let results = schema::search_memory(&conn, project_id, &query)?;
            if results.is_empty() {
                eprintln!("no memory matching '{query}'");
            } else {
                for (m, scope_name, project_name) in &results {
                    let stale_tag = if m.stale { " [STALE]" } else { "" };
                    println!(
                        "  [{:<3}] {}/{} — {}{}",
                        m.id, project_name, scope_name, m.title, stale_tag
                    );
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Graph command
// ---------------------------------------------------------------------------

fn cmd_graph(action: GraphAction) -> Result<()> {
    let conn = db::open_default()?;

    match action {
        GraphAction::Children { project, file } => {
            let proj = resolve_project(&conn, &project)?;
            let rels = schema::get_children(&conn, proj.id, &file)?;
            let mut seen = std::collections::HashSet::new();
            let resolved_rels: Vec<_> = rels
                .iter()
                .filter(|r| r.target_file.is_some())
                .filter(|r| seen.insert(r.target_file.clone()))
                .collect();
            if resolved_rels.is_empty() {
                eprintln!("{file} has no children (dependencies)");
            } else {
                println!("{file} depends on:\n");
                for r in &resolved_rels {
                    let target = r.target_file.as_deref().unwrap();
                    println!("  {:<12} {}", r.kind, target);
                }
            }
        }
        GraphAction::Parents { project, file } => {
            let proj = resolve_project(&conn, &project)?;
            let rels = schema::get_parents(&conn, proj.id, &file)?;
            let mut seen = std::collections::HashSet::new();
            let deduped: Vec<_> = rels
                .iter()
                .filter(|r| seen.insert(r.source_file.clone()))
                .collect();
            if deduped.is_empty() {
                eprintln!("{file} has no parents (nothing depends on it)");
            } else {
                println!("{file} is depended on by:\n");
                for r in &deduped {
                    println!("  {:<12} {}", r.kind, r.source_file);
                }
            }
        }
        GraphAction::Tree {
            project,
            file,
            depth,
        } => {
            let proj = resolve_project(&conn, &project)?;
            let tree = relationships::traverse_tree(&conn, proj.id, &file, depth)?;
            if tree.is_empty() {
                eprintln!("{file} has no dependency tree");
            } else {
                render_tree(&tree);
            }
        }
        GraphAction::Impact {
            project,
            file,
            depth,
        } => {
            let proj = resolve_project(&conn, &project)?;
            let tree = relationships::traverse_impact(&conn, proj.id, &file, depth)?;
            if tree.is_empty() {
                eprintln!("{file} has no impact tree");
            } else {
                println!("Changing {file} affects:\n");
                render_tree(&tree);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Diff command
// ---------------------------------------------------------------------------

fn cmd_diff(project_name: &str) -> Result<()> {
    let conn = db::open_default()?;
    let project = schema::get_project_by_name(&conn, project_name)?
        .ok_or_else(|| anyhow::anyhow!("project '{}' not found", project_name))?;

    let root = PathBuf::from(&project.root_path);
    if !root.exists() {
        anyhow::bail!("project root {} no longer exists", root.display());
    }

    let existing_hashes: HashMap<String, String> = schema::get_file_hashes(&conn, project.id)?
        .into_iter()
        .map(|f| (f.file_path, f.file_hash))
        .collect();

    let languages = registry::built_in_languages();
    let empty = build_globset(&[])?;

    let (_, source_files) = walker::walk(&root, &languages, None, false, &empty, &empty);

    let mut changed = Vec::new();
    let mut added = Vec::new();

    for sf in &source_files {
        let rel = sf.rel_path.to_string_lossy().into_owned();
        if let Some(hash) = walker::hash_file(&sf.path) {
            match existing_hashes.get(&rel) {
                Some(existing) if existing != &hash => changed.push(rel),
                None => added.push(rel),
                _ => {}
            }
        }
    }

    let current_files: std::collections::HashSet<String> = source_files
        .iter()
        .map(|sf| sf.rel_path.to_string_lossy().into_owned())
        .collect();
    let removed: Vec<String> = existing_hashes
        .keys()
        .filter(|k| !current_files.contains(k.as_str()))
        .cloned()
        .collect();

    // Stale memory
    let topics = schema::list_memory_scopes(&conn, project.id)?;
    let mut stale_articles: Vec<(String, schema::Memory)> = Vec::new();
    for t in &topics {
        for a in schema::list_memory_by_scope(&conn, t.id)? {
            if a.stale {
                stale_articles.push((t.name.clone(), a));
            }
        }
    }

    let head = git_head(&root).unwrap_or_default();
    let indexed_commit = project.head_commit.as_deref().unwrap_or("unknown");

    println!(
        "{project_name} — indexed at {indexed_commit}, current HEAD at {}",
        if head.is_empty() { "unknown" } else { &head }
    );

    if changed.is_empty() && added.is_empty() && removed.is_empty() {
        println!("\nNo changes since last index.");
    } else {
        println!("\nChanged files:");
        for f in &changed {
            println!("  M  {f}");
        }
        for f in &added {
            println!("  A  {f}");
        }
        for f in &removed {
            println!("  D  {f}");
        }
    }

    if !stale_articles.is_empty() {
        println!("\nStale memory:");
        for (scope_name, a) in &stale_articles {
            println!("  [{}] {}/{} — {}", a.id, project_name, scope_name, a.title);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn render_tree(entries: &[(usize, String, bool)]) {
    for (idx, (depth, path, is_cycle)) in entries.iter().enumerate() {
        if *depth == 0 {
            println!("{path}");
            continue;
        }
        let marker = if *is_cycle { " ← (already shown)" } else { "" };

        // Determine if this is the last sibling at its depth
        let is_last = entries[idx + 1..]
            .iter()
            .take_while(|(d, _, _)| *d >= *depth)
            .all(|(d, _, _)| *d > *depth);

        let connector = if is_last { "└── " } else { "├── " };

        // Build prefix from parent levels
        let mut prefix = String::new();
        for d in 1..*depth {
            let parent_has_more = entries[idx + 1..]
                .iter()
                .any(|(ed, _, _)| *ed == d);
            if parent_has_more {
                prefix.push_str("│   ");
            } else {
                prefix.push_str("    ");
            }
        }

        println!("{prefix}{connector}{path}{marker}");
    }
}


fn resolve_output_path(root: &std::path::Path, output: Option<&PathBuf>) -> Result<PathBuf> {
    match output {
        Some(p) => {
            let p = if p.is_absolute() {
                p.clone()
            } else {
                std::env::current_dir()?.join(p)
            };
            let parent = p.parent().unwrap_or(&p).canonicalize()?;
            Ok(parent.join(p.file_name().unwrap_or_default()))
        }
        None => Ok(root.join("rexicon.txt")),
    }
}

fn git_head(root: &std::path::Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}
