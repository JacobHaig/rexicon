use crate::symbol::{FileIndex, Symbol, SymbolKind};
use crate::walker::SourceFile;
use anyhow::{anyhow, Result};
use tree_sitter::{Node, Parser};

// ---------------------------------------------------------------------------
// Per-language extraction rules
// ---------------------------------------------------------------------------

struct LangRules {
    /// (node_kind, SymbolKind) for direct children of the root node
    top_level: &'static [(&'static str, SymbolKind)],
    /// (container_kind, child_kind, SymbolKind): symbols to extract from
    /// inside a matching container (impl body, class body, enum variants, …)
    nested: &'static [(&'static str, &'static str, SymbolKind)],
    /// Node kinds that represent a declaration body — everything from this
    /// node's start byte onward is replaced with `{ ... }` in the signature.
    body_kinds: &'static [&'static str],
}

// --- Rust ---
const RUST_TOP: &[(&str, SymbolKind)] = &[
    ("function_item", SymbolKind::Function),
    ("struct_item", SymbolKind::Struct),
    ("enum_item", SymbolKind::Enum),
    ("trait_item", SymbolKind::Trait),
    ("type_item", SymbolKind::TypeAlias),
    ("const_item", SymbolKind::Constant),
    ("static_item", SymbolKind::Constant),
    ("impl_item", SymbolKind::Impl),
    ("mod_item", SymbolKind::Module),
    ("macro_definition", SymbolKind::Macro),
];
const RUST_NESTED: &[(&str, &str, SymbolKind)] = &[
    ("impl_item", "function_item", SymbolKind::Method),
    ("trait_item", "function_item", SymbolKind::Method),
    ("enum_item", "enum_variant", SymbolKind::Variant),
    ("mod_item", "function_item", SymbolKind::Function),
    ("mod_item", "struct_item", SymbolKind::Struct),
    ("mod_item", "enum_item", SymbolKind::Enum),
    ("mod_item", "trait_item", SymbolKind::Trait),
    ("mod_item", "impl_item", SymbolKind::Impl),
];
const RUST_BODY: &[&str] = &[
    "block",
    "field_declaration_list",
    "ordered_field_declaration_list",
    "enum_variant_list",
    "declaration_list",
];

// --- Python ---
const PYTHON_TOP: &[(&str, SymbolKind)] = &[
    ("function_definition", SymbolKind::Function),
    ("async_function_definition", SymbolKind::Function),
    ("class_definition", SymbolKind::Class),
    ("decorated_definition", SymbolKind::Function),
];
const PYTHON_NESTED: &[(&str, &str, SymbolKind)] = &[
    ("class_definition", "function_definition", SymbolKind::Method),
    ("class_definition", "async_function_definition", SymbolKind::Method),
    ("class_definition", "decorated_definition", SymbolKind::Method),
];
const PYTHON_BODY: &[&str] = &["block"];

// --- Go ---
const GO_TOP: &[(&str, SymbolKind)] = &[
    ("function_declaration", SymbolKind::Function),
    ("method_declaration", SymbolKind::Method),
    ("type_declaration", SymbolKind::TypeAlias),
    ("const_declaration", SymbolKind::Constant),
    ("var_declaration", SymbolKind::Constant),
];
const GO_NESTED: &[(&str, &str, SymbolKind)] = &[];
const GO_BODY: &[&str] = &["block"];

// --- C / C++ ---
const C_TOP: &[(&str, SymbolKind)] = &[
    ("function_definition", SymbolKind::Function),
    ("declaration", SymbolKind::Constant),
    ("type_definition", SymbolKind::TypeAlias),
    ("struct_specifier", SymbolKind::Struct),
    ("enum_specifier", SymbolKind::Enum),
    ("preproc_def", SymbolKind::Constant),
    ("preproc_function_def", SymbolKind::Macro),
];
const C_NESTED: &[(&str, &str, SymbolKind)] = &[];
const C_BODY: &[&str] = &["compound_statement", "field_declaration_list", "enumerator_list"];

// --- JavaScript ---
const JS_TOP: &[(&str, SymbolKind)] = &[
    ("function_declaration", SymbolKind::Function),
    ("generator_function_declaration", SymbolKind::Function),
    ("class_declaration", SymbolKind::Class),
    ("lexical_declaration", SymbolKind::Constant),
    ("variable_declaration", SymbolKind::Constant),
    ("export_statement", SymbolKind::Function),
];
const JS_NESTED: &[(&str, &str, SymbolKind)] = &[
    ("class_declaration", "method_definition", SymbolKind::Method),
];
const JS_BODY: &[&str] = &["statement_block", "class_body"];

// --- TypeScript ---
const TS_TOP: &[(&str, SymbolKind)] = &[
    ("function_declaration", SymbolKind::Function),
    ("class_declaration", SymbolKind::Class),
    ("abstract_class_declaration", SymbolKind::Class),
    ("interface_declaration", SymbolKind::Interface),
    ("type_alias_declaration", SymbolKind::TypeAlias),
    ("enum_declaration", SymbolKind::Enum),
    ("lexical_declaration", SymbolKind::Constant),
    ("export_statement", SymbolKind::Function),
];
const TS_NESTED: &[(&str, &str, SymbolKind)] = &[
    ("class_declaration", "method_definition", SymbolKind::Method),
    ("abstract_class_declaration", "method_definition", SymbolKind::Method),
    ("interface_declaration", "method_signature", SymbolKind::Method),
    ("interface_declaration", "property_signature", SymbolKind::Constant),
    ("enum_declaration", "enum_member", SymbolKind::Variant),
];
const TS_BODY: &[&str] = &["statement_block", "class_body", "object_type", "enum_body"];

// --- C# ---
const CS_TOP: &[(&str, SymbolKind)] = &[
    ("namespace_declaration", SymbolKind::Module),
    ("class_declaration", SymbolKind::Class),
    ("struct_declaration", SymbolKind::Struct),
    ("interface_declaration", SymbolKind::Interface),
    ("enum_declaration", SymbolKind::Enum),
    ("method_declaration", SymbolKind::Method),
    ("constructor_declaration", SymbolKind::Function),
    ("property_declaration", SymbolKind::Constant),
    ("field_declaration", SymbolKind::Constant),
];
const CS_NESTED: &[(&str, &str, SymbolKind)] = &[
    ("namespace_declaration", "class_declaration", SymbolKind::Class),
    ("namespace_declaration", "struct_declaration", SymbolKind::Struct),
    ("namespace_declaration", "interface_declaration", SymbolKind::Interface),
    ("namespace_declaration", "enum_declaration", SymbolKind::Enum),
    ("class_declaration", "method_declaration", SymbolKind::Method),
    ("class_declaration", "constructor_declaration", SymbolKind::Function),
    ("class_declaration", "property_declaration", SymbolKind::Constant),
    ("class_declaration", "field_declaration", SymbolKind::Constant),
    ("struct_declaration", "method_declaration", SymbolKind::Method),
    ("struct_declaration", "constructor_declaration", SymbolKind::Function),
    ("interface_declaration", "method_declaration", SymbolKind::Method),
    ("interface_declaration", "property_declaration", SymbolKind::Constant),
    ("enum_declaration", "enum_member_declaration", SymbolKind::Variant),
];
const CS_BODY: &[&str] = &[
    "block",
    "declaration_list",
    "enum_member_declaration_list",
    "accessor_list",
];

fn lang_rules(lang_name: &str) -> Option<LangRules> {
    match lang_name {
        "rust" => Some(LangRules {
            top_level: RUST_TOP,
            nested: RUST_NESTED,
            body_kinds: RUST_BODY,
        }),
        "python" => Some(LangRules {
            top_level: PYTHON_TOP,
            nested: PYTHON_NESTED,
            body_kinds: PYTHON_BODY,
        }),
        "go" => Some(LangRules {
            top_level: GO_TOP,
            nested: GO_NESTED,
            body_kinds: GO_BODY,
        }),
        "c" | "cpp" => Some(LangRules {
            top_level: C_TOP,
            nested: C_NESTED,
            body_kinds: C_BODY,
        }),
        "javascript" => Some(LangRules {
            top_level: JS_TOP,
            nested: JS_NESTED,
            body_kinds: JS_BODY,
        }),
        "typescript" => Some(LangRules {
            top_level: TS_TOP,
            nested: TS_NESTED,
            body_kinds: TS_BODY,
        }),
        "c_sharp" => Some(LangRules {
            top_level: CS_TOP,
            nested: CS_NESTED,
            body_kinds: CS_BODY,
        }),
        _ => None,
    }
}

fn ts_language(lang_name: &str) -> Option<tree_sitter::Language> {
    match lang_name {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "c" | "cpp" => Some(tree_sitter_c::LANGUAGE.into()),
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "c_sharp" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
        // markdown is handled separately via line scanning, not tree-sitter
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn extract(file: &SourceFile) -> Result<FileIndex> {
    let source = std::fs::read(&file.path)?;
    let lang_name = file.language.name;

    if lang_name == "markdown" {
        return extract_markdown(file, &source);
    }

    let ts_lang = ts_language(lang_name)
        .ok_or_else(|| anyhow!("No tree-sitter grammar for '{}'", lang_name))?;
    let rules = lang_rules(lang_name)
        .ok_or_else(|| anyhow!("No extraction rules for '{}'", lang_name))?;

    let mut parser = Parser::new();
    parser.set_language(&ts_lang)?;
    let tree = parser
        .parse(&source, None)
        .ok_or_else(|| anyhow!("Failed to parse {}", file.rel_path.display()))?;

    let symbols = collect_top_level(tree.root_node(), &source, &rules);

    Ok(FileIndex {
        rel_path: file.rel_path.clone(),
        language: lang_name.to_string(),
        symbols,
    })
}

// ---------------------------------------------------------------------------
// Symbol collection helpers
// ---------------------------------------------------------------------------

fn collect_top_level(root: Node, source: &[u8], rules: &LangRules) -> Vec<Symbol> {
    let mut result = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if let Some(&(_, kind)) = rules.top_level.iter().find(|(k, _)| *k == child.kind()) {
            let children = collect_nested(child, source, child.kind(), rules);
            let signature = extract_signature(child, source, rules.body_kinds);
            result.push(Symbol { kind, signature, children });
        }
    }
    result
}

/// Searches the subtree of `node` for child symbols defined in the `nested`
/// table under `container_kind`.
fn collect_nested(
    node: Node,
    source: &[u8],
    container_kind: &str,
    rules: &LangRules,
) -> Vec<Symbol> {
    let targets: Vec<(&str, SymbolKind)> = rules
        .nested
        .iter()
        .filter(|(ck, _, _)| *ck == container_kind)
        .map(|(_, child_kind, sym_kind)| (*child_kind, *sym_kind))
        .collect();

    if targets.is_empty() {
        return Vec::new();
    }

    find_in_subtree(node, source, &targets, rules.body_kinds, 0)
}

/// Recursively walks `node`'s children looking for nodes whose kind appears in
/// `targets`. When a target is found it is captured (not recursed into further),
/// so nested bodies of found items are never searched.
fn find_in_subtree(
    node: Node,
    source: &[u8],
    targets: &[(&str, SymbolKind)],
    body_kinds: &[&str],
    depth: u8,
) -> Vec<Symbol> {
    if depth > 8 {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(&(_, kind)) = targets.iter().find(|(k, _)| *k == child.kind()) {
            let signature = extract_signature(child, source, body_kinds);
            result.push(Symbol { kind, signature, children: Vec::new() });
        } else {
            result.extend(find_in_subtree(child, source, targets, body_kinds, depth + 1));
        }
    }
    result
}

/// Returns a signature string for `node`: the text up to (but not including)
/// the first body child, with the body replaced by `{ ... }`. If no body
/// child is found the full node text is returned. Whitespace is normalised to
/// single spaces.
fn extract_signature(node: Node, source: &[u8], body_kinds: &[&str]) -> String {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if body_kinds.contains(&child.kind()) {
            let before = &source[node.start_byte()..child.start_byte()];
            let text = std::str::from_utf8(before).unwrap_or("").trim_end();
            return format!("{} {{ ... }}", normalize(text));
        }
    }
    let text = node.utf8_text(source).unwrap_or("");
    normalize(text)
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Markdown: ATX heading scanner (no tree-sitter — avoids crate version
// conflicts; ATX headings are trivially identifiable without a grammar)
// ---------------------------------------------------------------------------

fn extract_markdown(file: &SourceFile, source: &[u8]) -> Result<FileIndex> {
    let text = std::str::from_utf8(source)
        .map_err(|e| anyhow!("Invalid UTF-8 in {}: {e}", file.rel_path.display()))?;

    let flat = scan_headings(text);
    let symbols = build_heading_tree(flat);

    Ok(FileIndex {
        rel_path: file.rel_path.clone(),
        language: "markdown".to_string(),
        symbols,
    })
}

/// Scans `text` line-by-line for ATX headings (`# …` through `###### …`).
/// Returns a flat list of `(level, heading_text)` in document order.
fn scan_headings(text: &str) -> Vec<(u8, String)> {
    let mut result = Vec::new();
    for line in text.lines() {
        // Count leading `#` characters.
        let hashes = line.chars().take_while(|&c| c == '#').count();
        if hashes == 0 || hashes > 6 {
            continue;
        }
        let rest = &line[hashes..];
        // ATX headings require at least one space after the `#` markers.
        if !rest.starts_with(' ') && !rest.is_empty() {
            continue;
        }
        let heading_text = rest.trim().trim_end_matches('#').trim().to_string();
        result.push((hashes as u8, heading_text));
    }
    result
}

/// Converts a flat, ordered list of `(level, text)` heading pairs into a
/// nested Symbol tree where deeper headings become children of the nearest
/// shallower heading above them.
fn build_heading_tree(headings: Vec<(u8, String)>) -> Vec<Symbol> {
    let mut stack: Vec<Symbol> = Vec::new();
    let mut result: Vec<Symbol> = Vec::new();

    for (level, text) in headings {
        let sym = Symbol {
            kind: SymbolKind::Heading(level),
            signature: format!("{} {}", "#".repeat(level as usize), text),
            children: Vec::new(),
        };

        // Collapse items at the same or deeper level upward to their parent.
        while stack
            .last()
            .map(|s| heading_level(s) >= level)
            .unwrap_or(false)
        {
            let popped = stack.pop().unwrap();
            match stack.last_mut() {
                Some(parent) => parent.children.push(popped),
                None => result.push(popped),
            }
        }

        stack.push(sym);
    }

    // Drain the remainder of the stack.
    while let Some(popped) = stack.pop() {
        match stack.last_mut() {
            Some(parent) => parent.children.push(popped),
            None => result.push(popped),
        }
    }

    result
}

fn heading_level(sym: &Symbol) -> u8 {
    match sym.kind {
        SymbolKind::Heading(l) => l,
        _ => 0,
    }
}
