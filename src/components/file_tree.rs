use crate::theme;
use iocraft::prelude::*;
use std::path::{Path, PathBuf};

/// A node in the sidebar file tree: a file or directory on disk.
#[derive(Clone, Debug)]
pub struct FileNode {
    /// Display name (file/folder name only).
    pub name: String,
    /// Full path of this entry.
    pub path: PathBuf,
    pub is_dir: bool,
    /// Directories only: whether the folder is expanded in the tree.
    pub expanded: bool,
    /// Directories only: lazily-loaded children (empty until expanded once,
    /// or genuinely empty folders).
    pub children: Vec<FileNode>,
}

impl FileNode {
    /// Builds the root node for `dir`, listing one level of entries.
    pub fn from_dir(dir: &Path) -> Option<FileNode> {
        let canonical_dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        Some(FileNode {
            name: canonical_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| canonical_dir.display().to_string()),
            path: canonical_dir.clone(),
            is_dir: true,
            expanded: true,
            children: read_dir_sorted(&canonical_dir)?,
        })
    }

    /// Loads (or refreshes) this directory's children, keeping existing
    /// expansion state of previously-known subdirectories.
    pub fn reload_children(&mut self) {
        if !self.is_dir {
            return;
        }
        let mut fresh = read_dir_sorted(&self.path).unwrap_or_default();
        // Preserve expansion flags of known children.
        for child in &mut fresh {
            if let Some(old) = self.children.iter().find(|c| c.path == child.path) {
                child.expanded = old.expanded;
                child.children = old.children.clone();
            }
        }
        self.children = fresh;
        self.expanded = true;
    }

    /// Depth-first listing of currently *visible* nodes (expanded dirs walk
    /// into their children, collapsed dirs stay as single rows).
    pub fn visible_nodes<'a>(node: &'a FileNode, out: &mut Vec<&'a FileNode>) {
        out.push(node);
        if node.is_dir && node.expanded {
            for child in &node.children {
                Self::visible_nodes(child, out);
            }
        }
    }

    /// Toggles expansion of the directory at `target` (matching by path),
    /// lazily loading children the first time it is expanded.
    ///
    /// Returns `true` if some directory's expansion changed.
    pub fn toggle(node: &mut FileNode, target: &Path) -> bool {
        if node.path == target && node.is_dir {
            if !node.expanded && node.children.is_empty() {
                node.reload_children();
            } else {
                node.expanded = !node.expanded;
            }
            return true;
        }
        if node.is_dir && target.starts_with(&node.path) {
            for child in &mut node.children {
                if FileNode::toggle(child, target) {
                    return true;
                }
            }
        }
        false
    }
}

/// The rows the tree panel shows, shared by rendering and key handling so
/// selection indices always refer to the same list.
///
/// With an empty `query` this is the expanded tree. With a query it is a flat
/// list of files anywhere under the root whose path contains the query
/// (case-insensitive) — including files inside collapsed folders — with
/// Markdown files first and shorter paths before longer ones.
pub fn tree_rows(root: &FileNode, query: &str) -> Vec<FileNode> {
    if query.is_empty() {
        let mut refs = Vec::new();
        FileNode::visible_nodes(root, &mut refs);
        return refs.into_iter().cloned().collect();
    }
    let q = query.to_lowercase();
    let mut hits: Vec<FileNode> = Vec::new();
    collect_matches(&root.path, &root.path, &q, 0, &mut hits);
    hits.sort_by_key(|n| (!is_markdown(&n.name), n.name.len(), n.name.to_lowercase()));
    hits.truncate(200);
    hits
}

fn collect_matches(root: &Path, dir: &Path, q: &str, depth: usize, out: &mut Vec<FileNode>) {
    if depth > 12 || out.len() >= 2000 {
        return;
    }
    let Some(children) = read_dir_sorted(dir) else {
        return;
    };
    for child in children {
        if child.is_dir {
            collect_matches(root, &child.path, q, depth + 1, out);
        } else {
            let rel = child
                .path
                .strip_prefix(root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| child.name.clone());
            if rel.to_lowercase().contains(q) {
                out.push(FileNode { name: rel, ..child });
            }
        }
    }
}

/// Lists `dir` sorted directories-first then alphabetically, skipping hidden
/// entries and common build/VCS noise.
fn read_dir_sorted(dir: &Path) -> Option<Vec<FileNode>> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut nodes: Vec<(bool, String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                return None;
            }
            let ft = e.file_type().ok()?;
            let is_dir = ft.is_dir() || ft.is_symlink() && e.path().is_dir();
            if !is_dir && is_noise(&name) {
                return None;
            }
            if is_dir && is_noise_dir(&name) {
                return None;
            }
            Some((is_dir, name, e.path()))
        })
        .collect();
    nodes.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Some(
        nodes
            .into_iter()
            .map(|(is_dir, name, path)| FileNode {
                name,
                path,
                is_dir,
                expanded: false,
                children: Vec::new(),
            })
            .collect(),
    )
}

fn is_noise(name: &str) -> bool {
    matches!(
        name,
        "node_modules" | "target" | "dist" | "build" | "__pycache__" | ".DS_Store"
    )
}

fn is_noise_dir(name: &str) -> bool {
    matches!(name, "node_modules" | "target" | "build")
}

/// Properties for the [`FileTreePanel`] component.
#[derive(Default, Props)]
pub struct FileTreePanelProps {
    /// Flattened list of currently visible nodes (rendered top to bottom).
    pub visible: Vec<FileNode>,
    /// Index of the highlighted row within `visible`.
    pub selected: usize,
    /// Panel width in columns.
    pub width: u32,
    /// Path of the currently open file (highlighted differently from cursor).
    pub current_file: Option<std::path::PathBuf>,
    /// Current search filter string.
    pub search_query: String,
    /// Whether the tree is in search-input mode.
    pub is_searching: bool,
    /// Panel height in rows (used to keep the selection scrolled into view).
    pub height: u32,
    /// Whether the explorer has keyboard focus (vs. the document).
    pub focused: bool,
}

/// Collapsible left-side file explorer: folder tree with ▸/▾ indicators and a
/// highlighted selected row.
#[component]
pub fn FileTreePanel(props: &FileTreePanelProps) -> impl Into<AnyElement<'static>> {
    use unicode_width::UnicodeWidthStr;

    // Rows available for the list: total minus header (2) and footer (2).
    let list_rows = props.height.saturating_sub(4).max(1) as usize;
    let total = props.visible.len();
    // Keep the selection inside the window, scrolling as little as needed and
    // keeping a few rows of context below it.
    let first = if total <= list_rows {
        0
    } else {
        let margin = 3.min(list_rows / 3);
        let ideal_bottom = (props.selected + margin + 1).min(total);
        ideal_bottom.saturating_sub(list_rows).min(props.selected.saturating_sub(margin))
    };
    let last = (first + list_rows).min(total);
    // Inner width: panel minus right divider and left padding.
    let inner = props.width.saturating_sub(2) as usize;

    let root_name = props.visible.first().map(|n| n.name.clone()).unwrap_or_default();
    let footer: Vec<(String, String)> = if !props.focused {
        vec![("^h".into(), "focus explorer".into())]
    } else if props.is_searching {
        vec![("esc".into(), "cancel".into()), ("↵".into(), "done".into())]
    } else if !props.search_query.is_empty() {
        vec![("esc".into(), "clear".into()), ("↵".into(), "open".into()), ("\\".into(), "hide".into())]
    } else {
        vec![("↵".into(), "open".into()), ("/".into(), "filter".into()), ("esc".into(), "back".into())]
    };

    element! {
        View(
            width: props.width,
            height: 100pct,
            flex_direction: FlexDirection::Column,
            border_style: BorderStyle::Single,
            border_edges: Edges::Right,
            border_color: theme::border(),
            background_color: theme::dark_bg(),
        ) {
            // Header: workspace name, or the live filter while searching.
            View(height: 2, padding_left: 2, flex_direction: FlexDirection::Column) {
                #(if props.is_searching || !props.search_query.is_empty() {
                    element! {
                        View(flex_direction: FlexDirection::Row) {
                            Text(content: "/ ", color: theme::yellow(), weight: Weight::Bold)
                            Text(content: props.search_query.clone(), color: theme::fg())
                            #(props.is_searching.then(|| element! {
                                View(background_color: theme::yellow(), width: 1) { Text(content: " ") }
                            }.into_any()))
                        }
                    }.into_any()
                } else {
                    element! {
                        Text(
                            content: root_name.to_uppercase(),
                            color: if props.focused { theme::blue() } else { theme::muted() },
                            weight: Weight::Bold,
                            wrap: TextWrap::NoWrap,
                        )
                    }.into_any()
                })
            }
            View(flex_grow: 1.0, flex_direction: FlexDirection::Column) {
                #(props.visible.iter().enumerate().skip(first).take(last - first).map(|(i, node)| {
                    let selected = i == props.selected;
                    let is_current = props.current_file.as_ref()
                        .map_or(false, |cf| *cf == node.path);
                    let depth = if props.search_query.is_empty() { depth_of(&props.visible, i) } else { 0 };
                    let indent = "  ".repeat(depth);
                    let icon = if node.is_dir {
                        if node.expanded { "▾ " } else { "▸ " }
                    } else if props.search_query.is_empty() {
                        "  "
                    } else {
                        ""
                    };
                    let is_text = !node.is_dir && is_markdown(&node.name);
                    let color = if is_current {
                        theme::blue()
                    } else if node.is_dir || is_text {
                        theme::fg()
                    } else {
                        theme::comment()
                    };
                    // Truncate long names with an ellipsis instead of wrapping.
                    let budget = inner.saturating_sub(indent.len() + 2 + 1);
                    let name = if node.name.width() > budget && budget > 1 {
                        let mut out = String::new();
                        for ch in node.name.chars() {
                            if out.width() + 2 > budget { break; }
                            out.push(ch);
                        }
                        out.push('…');
                        out
                    } else {
                        node.name.clone()
                    };
                    element! {
                        // The selection bar is bright only while the
                        // explorer has focus, so it's clear where keys go.
                        View(height: 1, flex_direction: FlexDirection::Row, background_color: (selected && props.focused).then(theme::surface)) {
                            Text(content: if selected { "▌" } else { " " }, color: if props.focused { theme::blue() } else { theme::dark_grey() })
                            Text(content: format!(" {indent}"))
                            Text(content: icon, color: theme::muted())
                            Text(
                                content: name,
                                color: color,
                                weight: if node.is_dir || is_current { Weight::Bold } else { Weight::Normal },
                                wrap: TextWrap::NoWrap,
                            )
                        }
                    }.into_any()
                }))
                #((total == 0 && !props.search_query.is_empty()).then(|| element! {
                    View(padding_left: 2) {
                        Text(content: "No matching files", color: theme::comment(), italic: true)
                    }
                }.into_any()))
            }
            // Footer: the three keys that matter here.
            View(height: 2, padding_left: 2, padding_top: 1, flex_direction: FlexDirection::Row) {
                #(footer.into_iter().map(|(key, label)| element! {
                    View(flex_direction: FlexDirection::Row, margin_right: 2) {
                        Text(content: key, color: theme::fg(), weight: Weight::Bold)
                        Text(content: format!(" {label}"), color: theme::comment())
                    }
                }.into_any()))
            }
        }
    }
    .into_any()
}

/// Whether a file is one rivas opens as a document.
fn is_markdown(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown") || lower.ends_with(".mdx") || lower.ends_with(".txt")
}

/// Depth of a visible row = number of ancestor directories above it.
fn depth_of(visible: &[FileNode], idx: usize) -> usize {
    // Depth from path nesting relative to the root entry.
    let root_components = visible[0].path.components().count();
    visible[idx]
        .path
        .components()
        .count()
        .saturating_sub(root_components)
        .saturating_sub(1)
}
