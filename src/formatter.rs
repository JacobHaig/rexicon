use crate::symbol::{FileIndex, Symbol};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

// ---------------------------------------------------------------------------
// Internal tree representation
// ---------------------------------------------------------------------------

enum TreeNode {
    Dir {
        name: String,
        children: Vec<TreeNode>,
    },
    File {
        name: String,
        /// Empty string for files whose language we did not parse.
        language: String,
        symbols: Vec<Symbol>,
    },
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Builds and renders the complete rexicon.txt content as a single String.
pub fn format(all_files: &[PathBuf], indices: &[FileIndex], project_name: &str) -> String {
    let index: HashMap<PathBuf, &FileIndex> =
        indices.iter().map(|fi| (fi.rel_path.clone(), fi)).collect();

    let mut roots: Vec<TreeNode> = Vec::new();
    for path in all_files {
        insert_path(&mut roots, path, &index);
    }
    sort_nodes(&mut roots);

    let mut out = String::new();
    out.push_str(&format!("# rexicon — {}\n\n", project_name));
    out.push_str(&format!("{}/\n", project_name));
    render_nodes(&roots, "", &mut out);
    out
}

// ---------------------------------------------------------------------------
// Tree construction
// ---------------------------------------------------------------------------

fn insert_path(nodes: &mut Vec<TreeNode>, path: &Path, index: &HashMap<PathBuf, &FileIndex>) {
    let components: Vec<Component> = path.components().collect();
    insert_components(nodes, &components, path, index);
}

fn insert_components(
    nodes: &mut Vec<TreeNode>,
    components: &[Component],
    full_path: &Path,
    index: &HashMap<PathBuf, &FileIndex>,
) {
    if components.is_empty() {
        return;
    }

    let name = components[0].as_os_str().to_string_lossy().into_owned();

    if components.len() == 1 {
        // Leaf — this is a file node.
        let fi = index.get(full_path);
        let language = fi.map(|f| f.language.as_str()).unwrap_or("").to_string();
        let symbols = fi.map(|f| f.symbols.clone()).unwrap_or_default();
        nodes.push(TreeNode::File { name, language, symbols });
    } else {
        // Find an existing Dir with this name or create one.
        let pos = nodes.iter().position(|n| {
            if let TreeNode::Dir { name: dname, .. } = n {
                dname == &name
            } else {
                false
            }
        });

        if let Some(i) = pos {
            if let TreeNode::Dir { children, .. } = &mut nodes[i] {
                insert_components(children, &components[1..], full_path, index);
            }
        } else {
            let mut children = Vec::new();
            insert_components(&mut children, &components[1..], full_path, index);
            nodes.push(TreeNode::Dir { name, children });
        }
    }
}

/// Sorts nodes alphabetically by name at every level of the tree.
fn sort_nodes(nodes: &mut Vec<TreeNode>) {
    nodes.sort_by(|a, b| node_name(a).cmp(node_name(b)));
    for node in nodes.iter_mut() {
        if let TreeNode::Dir { children, .. } = node {
            sort_nodes(children);
        }
    }
}

fn node_name(n: &TreeNode) -> &str {
    match n {
        TreeNode::Dir { name, .. } => name,
        TreeNode::File { name, .. } => name,
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_nodes(nodes: &[TreeNode], prefix: &str, out: &mut String) {
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

        match node {
            TreeNode::Dir { name, children } => {
                out.push_str(&format!("{}{}{}/\n", prefix, connector, name));
                render_nodes(children, &child_prefix, out);
            }
            TreeNode::File { name, language, symbols } => {
                if language.is_empty() {
                    out.push_str(&format!("{}{}{}\n", prefix, connector, name));
                } else {
                    out.push_str(&format!(
                        "{}{}{}  [{}]\n",
                        prefix, connector, name, language
                    ));
                }
                if !symbols.is_empty() {
                    render_symbols(symbols, &child_prefix, out);
                }
            }
        }
    }
}

fn render_symbols(symbols: &[Symbol], prefix: &str, out: &mut String) {
    for (i, sym) in symbols.iter().enumerate() {
        let is_last = i == symbols.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

        let line_tag = if sym.line_start == sym.line_end {
            format!("[{}]", sym.line_start)
        } else {
            format!("[{}:{}]", sym.line_start, sym.line_end)
        };
        out.push_str(&format!(
            "{}{}{}  {}\n",
            prefix, connector, sym.signature, line_tag
        ));

        if !sym.children.is_empty() {
            render_symbols(&sym.children, &child_prefix, out);
        }
    }
}
