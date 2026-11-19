use pulldown_cmark::{CowStr, Event, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeNode {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
}

/// Builds a tree of directories and files under `dir`, relative to `root`.
pub fn build_tree(dir: &Path, root: &Path) -> Option<TreeNode> {
    let rel = dir.strip_prefix(root).unwrap_or(dir);
    let rel_str = rel.to_string_lossy().to_string();
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.display().to_string());

    if dir.is_dir() {
        let mut children = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut list: Vec<(bool, String, PathBuf)> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let fname = e.file_name().to_string_lossy().to_string();
                    if fname.starts_with('.') || is_ignored_name(&fname) {
                        return None;
                    }
                    let ft = e.file_type().ok()?;
                    let is_dir = ft.is_dir() || (ft.is_symlink() && e.path().is_dir());
                    if !is_dir && !is_markdown_or_asset(&fname) {
                        return None;
                    }
                    Some((is_dir, fname, e.path()))
                })
                .collect();

            // Directories first, then alphabetically
            list.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));

            for (is_dir, _fname, path) in list {
                if is_dir {
                    if let Some(child_node) = build_tree(&path, root) {
                        children.push(child_node);
                    }
                } else {
                    let child_rel = path.strip_prefix(root).unwrap_or(&path);
                    children.push(TreeNode {
                        name: path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        rel_path: child_rel.to_string_lossy().to_string(),
                        is_dir: false,
                        children: Vec::new(),
                    });
                }
            }
        }
        Some(TreeNode {
            name,
            rel_path: rel_str,
            is_dir: true,
            children,
        })
    } else {
        Some(TreeNode {
            name,
            rel_path: rel_str,
            is_dir: false,
            children: Vec::new(),
        })
    }
}

fn is_ignored_name(name: &str) -> bool {
    matches!(
        name,
        "target"
            | "node_modules"
            | ".git"
            | "dist"
            | "build"
            | "__pycache__"
            | ".DS_Store"
            | "venv"
            | ".venv"
    )
}

fn is_markdown_or_asset(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".md")
        || lower.ends_with(".markdown")
        || lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".svg")
        || lower.ends_with(".webp")
}

/// Recursively renders the tree into HTML for the sidebar.
pub fn render_sidebar_html(node: &TreeNode, current_active: &str, is_root: bool) -> String {
    let mut html = String::new();
    if is_root {
        for child in &node.children {
            html.push_str(&render_sidebar_html(child, current_active, false));
        }
        return html;
    }

    if node.is_dir {
        let is_open = current_active.starts_with(&node.rel_path);
        let open_attr = if is_open { " open" } else { "" };
        html.push_str(&format!(
            r#"<details class="tree-folder"{open_attr}>
<summary class="tree-folder-title"><span class="folder-icon">📁</span> <span class="folder-name">{}</span></summary>
<div class="tree-folder-children">
"#,
            escape_html(&node.name)
        ));
        for child in &node.children {
            html.push_str(&render_sidebar_html(child, current_active, false));
        }
        html.push_str("</div></details>\n");
    } else {
        let is_md = node.name.to_lowercase().ends_with(".md")
            || node.name.to_lowercase().ends_with(".markdown");
        let is_active = node.rel_path == current_active;
        let active_cls = if is_active { " active" } else { "" };
        let icon = if is_md { "📄" } else { "🖼️" };
        let link = if is_md {
            format!("/view/{}", url_encode_path(&node.rel_path))
        } else {
            format!("/raw/{}", url_encode_path(&node.rel_path))
        };
        html.push_str(&format!(
            r#"<a href="{link}" class="tree-file{active_cls}" data-name="{}" title="{}">
<span class="file-icon">{icon}</span> <span class="file-name">{}</span>
</a>
"#,
            escape_html(&node.name),
            escape_html(&node.rel_path),
            escape_html(&node.name)
        ));
    }
    html
}

/// Renders markdown source into HTML with pulldown-cmark, rewriting relative image
/// URLs to `/raw/...` and relative `.md` links to `/view/...`, and handling math tags.
pub fn render_markdown_to_html(markdown: &str, current_file_rel: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
        | Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = Parser::new_ext(markdown, options);

    let cur_parent = Path::new(current_file_rel)
        .parent()
        .unwrap_or_else(|| Path::new(""));

    let mut transformed_events = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let rewritten = rewrite_image_url(&dest_url, cur_parent);
                transformed_events.push(Event::Start(Tag::Image {
                    link_type,
                    dest_url: CowStr::Boxed(rewritten.into_boxed_str()),
                    title,
                    id,
                }));
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let rewritten = rewrite_link_url(&dest_url, cur_parent);
                transformed_events.push(Event::Start(Tag::Link {
                    link_type,
                    dest_url: CowStr::Boxed(rewritten.into_boxed_str()),
                    title,
                    id,
                }));
            }
            Event::InlineMath(m) => {
                let html = format!(r#"<span class="math math-inline">\({}\)</span>"#, escape_html(&m));
                transformed_events.push(Event::Html(CowStr::Boxed(html.into_boxed_str())));
            }
            Event::DisplayMath(m) => {
                let html = format!(r#"<div class="math math-display">\[{}\]</div>"#, escape_html(&m));
                transformed_events.push(Event::Html(CowStr::Boxed(html.into_boxed_str())));
            }
            other => transformed_events.push(other),
        }
    }

    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, transformed_events.into_iter());
    html_output
}

fn rewrite_image_url(url: &str, cur_parent: &Path) -> String {
    if url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("data:")
        || url.starts_with('/')
    {
        return url.to_string();
    }
    let resolved = normalize_path(&cur_parent.join(url));
    format!("/raw/{}", url_encode_path(&resolved.to_string_lossy()))
}

fn rewrite_link_url(url: &str, cur_parent: &Path) -> String {
    if url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("data:")
        || url.starts_with('#')
        || url.starts_with('/')
    {
        return url.to_string();
    }

    // Split anchor if any (e.g. "other.md#section")
    let (path_part, anchor_part) = match url.split_once('#') {
        Some((p, a)) => (p, Some(a)),
        None => (url, None),
    };

    if path_part.ends_with(".md") || path_part.ends_with(".markdown") {
        let resolved = normalize_path(&cur_parent.join(path_part));
        let base = format!("/view/{}", url_encode_path(&resolved.to_string_lossy()));
        if let Some(anchor) = anchor_part {
            format!("{base}#{anchor}")
        } else {
            base
        }
    } else {
        url.to_string()
    }
}

/// Normalizes relative paths by resolving `.` and `..` without touching filesystem.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(c) => out.push(c),
            Component::RootDir => out.push("/"),
            Component::Prefix(p) => out.push(p.as_os_str()),
        }
    }
    out
}

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn url_encode_path(path: &str) -> String {
    let mut out = String::new();
    for seg in path.split('/') {
        if !out.is_empty() {
            out.push('/');
        }
        for byte in seg.bytes() {
            match byte {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(byte as char);
                }
                _ => {
                    out.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown_basic() {
        let md = "# Hello World\n\nThis is **bold** and *italic*.";
        let html = render_markdown_to_html(md, "README.md");
        assert!(html.contains("<h1>Hello World</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn test_rewrite_relative_image() {
        let md = "![Diagram](./images/diag.png)";
        let html = render_markdown_to_html(md, "docs/guide.md");
        assert!(html.contains(r#"src="/raw/docs/images/diag.png""#));
    }

    #[test]
    fn test_rewrite_markdown_link() {
        let md = "[Next Chapter](./chapter2.md#summary)";
        let html = render_markdown_to_html(md, "docs/chapter1.md");
        assert!(html.contains(r#"href="/view/docs/chapter2.md#summary""#));
    }

    #[test]
    fn test_external_link_not_rewritten() {
        let md = "[Google](https://google.com)";
        let html = render_markdown_to_html(md, "notes.md");
        assert!(html.contains(r#"href="https://google.com""#));
    }

    #[test]
    fn test_math_rendering() {
        let md = "Inline math: $E=mc^2$\n\nDisplay math:\n\n$$x = \\frac{1}{2}$$";
        let html = render_markdown_to_html(md, "math.md");
        assert!(html.contains(r#"class="math math-inline""#));
        assert!(html.contains(r#"class="math math-display""#));
    }

    #[test]
    fn test_mermaid_code_block() {
        let md = "```mermaid\ngraph TD;\nA-->B;\n```";
        let html = render_markdown_to_html(md, "diag.md");
        assert!(html.contains("language-mermaid"));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(
            normalize_path(Path::new("docs/../images/logo.png")),
            PathBuf::from("images/logo.png")
        );
        assert_eq!(
            normalize_path(Path::new("./foo/./bar/baz.md")),
            PathBuf::from("foo/bar/baz.md")
        );
    }
}

