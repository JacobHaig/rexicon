use std::path::Path;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Language {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub lsp_command: Option<&'static str>,
}

pub fn built_in_languages() -> Vec<Language> {
    vec![
        Language {
            name: "rust",
            extensions: &["rs"],
            lsp_command: Some("rust-analyzer"),
        },
        Language {
            name: "python",
            extensions: &["py", "pyi"],
            lsp_command: Some("pylsp"),
        },
        Language {
            name: "go",
            extensions: &["go"],
            lsp_command: Some("gopls"),
        },
        Language {
            name: "c",
            extensions: &["c", "h"],
            lsp_command: Some("clangd"),
        },
        Language {
            name: "cpp",
            extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
            lsp_command: Some("clangd"),
        },
        Language {
            name: "javascript",
            extensions: &["js", "jsx", "mjs", "cjs"],
            lsp_command: Some("typescript-language-server"),
        },
        Language {
            name: "typescript",
            extensions: &["ts", "tsx", "mts", "cts"],
            lsp_command: Some("typescript-language-server"),
        },
        Language {
            name: "c_sharp",
            extensions: &["cs"],
            lsp_command: Some("OmniSharp"),
        },
        Language {
            name: "zig",
            extensions: &["zig"],
            lsp_command: Some("zls"),
        },
        Language {
            name: "markdown",
            extensions: &["md", "mdx"],
            lsp_command: Some("marksman"),
        },
    ]
}

pub fn detect_language<'a>(path: &Path, languages: &'a [Language]) -> Option<&'a Language> {
    let ext = path.extension()?.to_str()?;
    languages.iter().find(|lang| lang.extensions.contains(&ext))
}
