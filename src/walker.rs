use crate::registry::{Language, detect_language};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub rel_path: PathBuf,
    pub language: Language,
}

/// Returns every non-hidden, non-gitignored file under `root`, sorted by
/// relative path. Excludes `exclude` (the output file) if it falls inside `root`.
pub fn walk_all(root: &Path, exclude: Option<&Path>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = WalkBuilder::new(root)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter_map(|e| e.path().strip_prefix(root).ok().map(|p| p.to_owned()))
        .filter(|rel| Some(rel.as_path()) != exclude)
        .collect();
    paths.sort();
    paths
}

/// Returns only files whose extension matches a known language, sorted by
/// relative path.
pub fn walk(root: &Path, languages: &[Language]) -> Vec<SourceFile> {
    let mut files: Vec<SourceFile> = WalkBuilder::new(root)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let path = e.into_path();
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
