//! HTML shell for the browser preview. Styles and behavior live in
//! `assets/page.css` and `assets/page.js` (embedded at compile time) so they
//! can be read and edited as ordinary CSS/JS.

use crate::server::renderer::{escape_html, url_encode_path};

const PAGE_CSS: &str = include_str!("assets/page.css");
const PAGE_JS: &str = include_str!("assets/page.js");

/// Inline SVG icons (Lucide-style strokes, MIT). One consistent visual
/// language instead of emoji, which render differently on every platform.
pub fn icon(name: &str) -> &'static str {
    match name {
        "menu" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h16M4 12h16M4 18h16"/></svg>"#,
        "x" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12"/></svg>"#,
        "search" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>"#,
        "sun" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>"#,
        "moon" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/></svg>"#,
        "code" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="m16 18 6-6-6-6M8 6l-6 6 6 6"/></svg>"#,
        "edit" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/></svg>"#,
        "copy" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>"#,
        "check" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg>"#,
        "save" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/></svg>"#,
        "eye" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></svg>"#,
        "folder" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>"#,
        "file" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6M8 13h8M8 17h5"/></svg>"#,
        "image" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-5-5L5 21"/></svg>"#,
        "chevron" => r#"<svg class="icon chev" viewBox="0 0 24 24" aria-hidden="true"><path d="m9 18 6-6-6-6"/></svg>"#,
        "info" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/></svg>"#,
        "keyboard" => r#"<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/></svg>"#,
        _ => "",
    }
}

/// Breadcrumb trail for a workspace-relative path: every folder links to its
/// view; the last segment is the current page.
fn breadcrumbs(root_name: &str, rel: &str) -> String {
    let mut out = format!(r#"<a href="/">{}</a>"#, escape_html(root_name));
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    let mut acc = String::new();
    for (i, part) in parts.iter().enumerate() {
        out.push_str(r#"<span class="sep">/</span>"#);
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(part);
        if i + 1 == parts.len() {
            out.push_str(&format!(r#"<span class="current" aria-current="page">{}</span>"#, escape_html(part)));
        } else {
            out.push_str(&format!(r#"<a href="/view/{}">{}</a>"#, url_encode_path(&acc), escape_html(part)));
        }
    }
    out
}

/// Builds the complete preview page.
pub fn build_page_html(
    title: &str,
    root_name: &str,
    current_rel: &str,
    sidebar_html: &str,
    content_html: &str,
    raw_markdown: &str,
    is_dir_view: bool,
) -> String {
    let escaped_title = escape_html(title);
    let escaped_rel = escape_html(current_rel);
    let escaped_raw = escape_html(raw_markdown);
    let crumbs = breadcrumbs(root_name, current_rel);
    let kind = if is_dir_view { "dir" } else { "file" };
    let brand = escape_html(root_name);
    let i = icon;

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{escaped_title} · rivas</title>
<script>
  // Apply the theme before first paint (saved choice, else the OS setting).
  (function () {{
    var t = null;
    try {{ t = localStorage.getItem('rivas-theme'); }} catch (e) {{}}
    if (!t) t = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    document.documentElement.dataset.theme = t;
  }})();
</script>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/contrib/auto-render.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/mermaid@10.9.1/dist/mermaid.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/highlight.min.js"></script>
<style>{PAGE_CSS}</style>
</head>
<body data-rel="{escaped_rel}" data-kind="{kind}">
<div id="progress" aria-hidden="true"></div>

<aside class="sidebar" id="sidebar" aria-label="Files">
  <div class="sidebar-head">
    <a href="/" class="brand"><span class="brand-mark">R</span><span>{brand}</span></a>
    <button class="btn icon-only mobile-only" data-action="sidebar" aria-label="Close files">{x}</button>
  </div>
  <div class="search">
    {search}
    <input id="filter" type="search" placeholder="Filter files" aria-label="Filter files" autocomplete="off" spellcheck="false">
    <kbd class="hide-narrow">/</kbd>
  </div>
  <nav class="tree" id="tree">
    {sidebar_html}
    <p class="tree-empty" hidden>No matching files</p>
  </nav>
  <div class="sidebar-foot">
    <span>rivas</span>
    <button class="linklike" data-action="shortcuts">{keyboard}Shortcuts <kbd>?</kbd></button>
  </div>
</aside>
<div id="backdrop"></div>

<div class="main">
  <header class="topbar">
    <button class="btn icon-only" data-action="sidebar" aria-label="Toggle files" title="Toggle files (\)">{menu}</button>
    <nav class="crumbs" id="crumbs" aria-label="Breadcrumb">{crumbs}</nav>
    <div class="actions">
      <span class="live hide-narrow" id="live" title="Live reload"><span class="dot"></span><span class="label">Live</span></span>
      <button class="btn icon-only" id="theme-btn" data-action="theme" title="Toggle theme (t)"><span class="only-light">{moon}</span><span class="only-dark">{sun}</span></button>
      <button class="btn icon-only hide-narrow" data-action="copy" aria-label="Copy Markdown" title="Copy Markdown (c)">{copy}</button>
      <button class="btn" id="source-btn" data-action="source" aria-pressed="false" title="View source (r)">{code}<span class="label">Source</span></button>
      <button class="btn outline" id="edit-btn" data-action="edit" aria-pressed="false" title="Edit (e)">{edit}<span class="label">Edit</span></button>
    </div>
  </header>

  <div class="workspace">
    <section class="editor" aria-label="Editor">
      <div class="editor-bar">
        <span class="title" id="editor-title">{escaped_rel}</span>
        <span class="save-state" id="save-state">Saved</span>
        <button class="btn icon-only mobile-only" data-action="preview-tab" aria-label="Show preview">{eye}</button>
        <button class="btn primary" data-action="save" title="Save (⌘S / Ctrl+S)">{save}<span class="label">Save</span></button>
      </div>
      <textarea id="editor-input" spellcheck="false" aria-label="Markdown source"></textarea>
    </section>
    <div class="scroller" id="scroller">
      <div class="page">
        <article class="markdown-body" id="markdown-body">
{content_html}
        </article>
        <pre class="source" id="source"><code id="source-code" class="language-markdown">{escaped_raw}</code></pre>
        <nav class="toc" id="toc" aria-label="On this page" hidden></nav>
      </div>
    </div>
  </div>
</div>

<div id="toast" role="status" aria-live="polite"></div>
<dialog id="shortcuts" aria-label="Keyboard shortcuts">
  <div class="dialog-head">Keyboard shortcuts <button class="btn icon-only" data-action="close-dialog" aria-label="Close">{x}</button></div>
  <dl class="shortcut-list">
    <dt><kbd>/</kbd> <kbd>⌘K</kbd></dt><dd>Filter files</dd>
    <dt><kbd>\</kbd></dt><dd>Show or hide the file list</dd>
    <dt><kbd>e</kbd></dt><dd>Edit with live preview</dd>
    <dt><kbd>⌘S</kbd></dt><dd>Save while editing</dd>
    <dt><kbd>r</kbd></dt><dd>View Markdown source</dd>
    <dt><kbd>c</kbd></dt><dd>Copy Markdown</dd>
    <dt><kbd>t</kbd></dt><dd>Toggle light / dark</dd>
    <dt><kbd>?</kbd></dt><dd>This list</dd>
  </dl>
</dialog>

<template id="icon-copy">{copy}</template>
<template id="icon-check">{check}</template>
<template id="icon-info">{info}</template>
<script>{PAGE_JS}</script>
</body>
</html>
"##,
        x = i("x"),
        search = i("search"),
        keyboard = i("keyboard"),
        menu = i("menu"),
        moon = i("moon"),
        sun = i("sun"),
        copy = i("copy"),
        code = i("code"),
        edit = i("edit"),
        eye = i("eye"),
        save = i("save"),
        check = i("check"),
        info = i("info"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breadcrumbs_link_every_folder() {
        let html = breadcrumbs("ws", "docs/guide/intro.md");
        assert!(html.contains(r#"<a href="/">ws</a>"#));
        assert!(html.contains(r#"<a href="/view/docs">docs</a>"#));
        assert!(html.contains(r#"<a href="/view/docs/guide">guide</a>"#));
        assert!(html.contains(r#"aria-current="page">intro.md</span>"#));
    }

    #[test]
    fn page_embeds_assets_and_escapes() {
        let html = build_page_html("<t>", "ws", "a.md", "", "<p>x</p>", "<raw>", false);
        assert!(html.contains("&lt;t&gt; · rivas"));
        assert!(html.contains("&lt;raw&gt;"));
        assert!(html.contains(".markdown-body"));
        assert!(html.contains("data-kind=\"file\""));
    }
}
