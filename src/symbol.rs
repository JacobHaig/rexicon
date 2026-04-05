use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Interface,
    Class,
    Constant,
    TypeAlias,
    Module,
    Impl,
    Variant,
    Macro,
    Heading(u8),
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub kind: SymbolKind,
    /// Full signature with bodies replaced by `{ ... }`
    pub signature: String,
    /// Nested items: enum variants, impl/trait methods, class members
    pub children: Vec<Symbol>,
}

#[derive(Debug, Clone)]
pub struct FileIndex {
    pub rel_path: PathBuf,
    pub language: String,
    pub symbols: Vec<Symbol>,
}
