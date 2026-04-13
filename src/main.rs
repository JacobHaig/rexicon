use anyhow::Result;
use clap::Parser;
use globset::{Glob, GlobSetBuilder};
use rayon::prelude::*;
use rexicon::{formatter, output, registry, symbol, treesitter, walker};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "rexicon",
    about = "Index a codebase into a single structured text file for LLM navigation"
)]
struct Args {
    /// Root directory of the project to index (defaults to current directory)
    #[arg(default_value = ".")]
    target: PathBuf,

    /// Path for the output file (default: <target>/rexicon.txt)
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Include files that would normally be excluded by .gitignore (e.g. target/, node_modules/)
    #[arg(long)]
    no_ignore: bool,

    /// Only include files/folders matching these patterns (repeatable)
    #[arg(long = "include", value_name = "PATTERN")]
    includes: Vec<String>,

    /// Exclude files/folders matching these patterns (repeatable)
    #[arg(long = "exclude", value_name = "PATTERN")]
    excludes: Vec<String>,

    /// Output format: txt (default box-drawing tree) or plain (path:line  signature, one per symbol)
    #[arg(long, default_value = "txt")]
    format: Format,
}

#[derive(clap::ValueEnum, Clone)]
enum Format {
    Txt,
    Plain,
}

fn build_globset(patterns: &[String]) -> Result<globset::GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for raw in patterns {
        // If the pattern has no glob characters and doesn't start with *,
        // treat it as a prefix — match the path itself or anything inside it.
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
    let args = Args::parse();
    let root = args.target.canonicalize()?;
    let output_path = match args.output {
        Some(p) => {
            // Resolve relative to cwd; the file may not exist yet so we
            // canonicalize the parent and append the filename.
            let p = if p.is_absolute() {
                p
            } else {
                std::env::current_dir()?.join(p)
            };
            let parent = p.parent().unwrap_or(&p).canonicalize()?;
            parent.join(p.file_name().unwrap_or_default())
        }
        None => root.join("rexicon.txt"),
    };

    // Compute the output path relative to root so we can exclude it from the
    // file tree (only relevant when output lives inside the project directory).
    let output_rel = output_path.strip_prefix(&root).ok().map(|p| p.to_owned());

    let languages = registry::built_in_languages();

    let includes = build_globset(&args.includes)?;
    let excludes = build_globset(&args.excludes)?;

    let (all_files, source_files) = walker::walk(
        &root,
        &languages,
        output_rel.as_deref(),
        args.no_ignore,
        &includes,
        &excludes,
    );

    // Extract symbols in parallel; failed files are skipped with a warning.
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

    // Sort for deterministic output.
    indices.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let project_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let text = match args.format {
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
    Ok(())
}
