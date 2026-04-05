use crate::registry::{Language, detect_language};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub rel_path: PathBuf,
    pub language: Language,
}

/// Returns every file under `root`, sorted by relative path.
/// Hidden directories (dotfiles) are included — only `.git` is always excluded.
/// When `no_ignore` is false (the default), `.gitignore` rules are respected.
/// Excludes `exclude` (the output file) if it falls inside `root`.
pub fn walk_all(root: &Path, exclude: Option<&Path>, no_ignore: bool) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(!no_ignore)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter_map(|e| e.path().strip_prefix(root).ok().map(|p| p.to_owned()))
        .filter(|rel| !rel.components().any(|c| c.as_os_str() == ".git"))
        .filter(|rel| Some(rel.as_path()) != exclude)
        .collect();
    paths.sort();
    paths
}

/// Returns only files whose extension matches a known language, sorted by
/// relative path. Respects `.gitignore` unless `no_ignore` is true.
pub fn walk(root: &Path, languages: &[Language], no_ignore: bool) -> Vec<SourceFile> {
    let mut files: Vec<SourceFile> = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(!no_ignore)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let path = e.into_path();
            if path.components().any(|c| c.as_os_str() == ".git") {
                return None;
            }
            let language = detect_language(&path, languages)?.clone();
            let rel_path = path.strip_prefix(root).ok()?.to_owned();
            Some(SourceFile {
                path,
                rel_path,
                language,
            })
        })
        .collect();
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    files
}
