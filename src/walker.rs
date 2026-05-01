use crate::registry::{Language, detect_language};
use globset::GlobSet;
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub rel_path: PathBuf,
    pub language: Language,
}

pub fn hash_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Some(hex)
}

pub fn git_head_short(root: &Path) -> Option<String> {
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

/// Walks `root` in parallel using all available CPU cores, producing both the
/// full file list and the language-matched source file list in a single pass.
///
/// - Hidden directories (dotfiles) are included; `.git` is always excluded.
/// - `.gitignore` rules are respected unless `no_ignore` is true.
/// - `exclude_file` (the output file path) is omitted from both lists.
/// - `includes`: if non-empty, only paths matching at least one pattern are kept.
/// - `excludes`: paths matching any pattern are dropped.
pub fn walk(
    root: &Path,
    languages: &[Language],
    exclude_file: Option<&Path>,
    no_ignore: bool,
    includes: &GlobSet,
    excludes: &GlobSet,
) -> (Vec<PathBuf>, Vec<SourceFile>) {
    let all: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let sources: Arc<Mutex<Vec<SourceFile>>> = Arc::new(Mutex::new(Vec::new()));

    let root = root.to_owned();
    let exclude_file = exclude_file.map(|p| p.to_owned());
    let languages: Arc<Vec<Language>> = Arc::new(languages.to_vec());
    let includes: Arc<GlobSet> = Arc::new(includes.clone());
    let excludes: Arc<GlobSet> = Arc::new(excludes.clone());

    let all2 = Arc::clone(&all);
    let sources2 = Arc::clone(&sources);

    WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(!no_ignore)
        .build_parallel()
        .run(move || {
            let root = root.clone();
            let exclude_file = exclude_file.clone();
            let languages = Arc::clone(&languages);
            let includes = Arc::clone(&includes);
            let excludes = Arc::clone(&excludes);
            let all = Arc::clone(&all2);
            let sources = Arc::clone(&sources2);

            Box::new(move |entry| {
                use ignore::WalkState;
                let e = match entry {
                    Ok(e) => e,
                    Err(_) => return WalkState::Continue,
                };
                if !e.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    return WalkState::Continue;
                }
                let path = e.into_path();
                let rel = match path.strip_prefix(&root) {
                    Ok(r) => r.to_owned(),
                    Err(_) => return WalkState::Continue,
                };

                // Always skip .git internals and the output file.
                if rel.components().any(|c| c.as_os_str() == ".git") {
                    return WalkState::Continue;
                }
                if exclude_file.as_deref() == Some(rel.as_path()) {
                    return WalkState::Continue;
                }

                // Apply --include / --exclude filters.
                if !includes.is_empty() && !includes.is_match(&rel) {
                    return WalkState::Continue;
                }
                if excludes.is_match(&rel) {
                    return WalkState::Continue;
                }

                let lang = detect_language(&path, &languages).cloned();

                all.lock().unwrap().push(rel.clone());
                if let Some(language) = lang {
                    sources.lock().unwrap().push(SourceFile {
                        path,
                        rel_path: rel,
                        language,
                    });
                }

                WalkState::Continue
            })
        });

    let mut all = Arc::try_unwrap(all).unwrap().into_inner().unwrap();
    let mut sources = Arc::try_unwrap(sources).unwrap().into_inner().unwrap();

    all.sort();
    sources.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    (all, sources)
}
