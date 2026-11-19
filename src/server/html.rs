pub fn build_page_html(
    title: &str,
    current_rel: &str,
    sidebar_html: &str,
    content_html: &str,
    raw_markdown: &str,
    _is_dir_view: bool,
) -> String {
    let escaped_title = crate::server::renderer::escape_html(title);
    let escaped_rel = crate::server::renderer::escape_html(current_rel);
    let escaped_raw = crate::server::renderer::escape_html(raw_markdown);

    format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{escaped_title} - rivas</title>
    <!-- KaTeX for math rendering -->
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">
    <script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/contrib/auto-render.min.js"></script>
    <!-- Mermaid for diagrams -->
    <script src="https://cdn.jsdelivr.net/npm/mermaid@10.9.1/dist/mermaid.min.js"></script>
    <!-- Highlight.js for code syntax highlighting -->
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/styles/tokyo-night-dark.min.css" id="hljs-theme">
    <script src="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/highlight.min.js"></script>
    <style>
        :root {{
            --bg-main: #1a1b26;
            --bg-sidebar: #16161e;
            --bg-panel: #1f2335;
            --bg-hover: #292e42;
            --fg-main: #c0caf5;
            --fg-muted: #787c99;
            --fg-dim: #565f89;
            --accent: #7aa2f7;
            --accent-hover: #89ddff;
            --border: #292e42;
            --code-bg: #1f2335;
            --table-alt: #24283b;
            --quote-border: #7aa2f7;
            --quote-bg: rgba(122, 162, 247, 0.05);
            --badge-bg: #414868;
            --badge-fg: #c0caf5;
            --success: #9ece6a;
            --warning: #e0af68;
            --shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
            --sidebar-width: 290px;
            --font-main: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            --font-mono: "JetBrains Mono", "SF Mono", Menlo, Consolas, Monaco, "Liberation Mono", monospace;
        }}

        [data-theme="light"] {{
            --bg-main: #ffffff;
            --bg-sidebar: #f6f8fa;
            --bg-panel: #f6f8fa;
            --bg-hover: #eaeef2;
            --fg-main: #24292f;
            --fg-muted: #57606a;
            --fg-dim: #8c959f;
            --accent: #0969da;
            --accent-hover: #0550ae;
            --border: #d0d7de;
            --code-bg: #f6f8fa;
            --table-alt: #f6f8fa;
            --quote-border: #0969da;
            --quote-bg: rgba(9, 105, 218, 0.05);
            --badge-bg: #e1e4e8;
            --badge-fg: #24292f;
            --success: #1a7f37;
            --warning: #d97706;
            --shadow: 0 4px 20px rgba(0, 0, 0, 0.08);
        }}

        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}

        body {{
            font-family: var(--font-main);
            background-color: var(--bg-main);
            color: var(--fg-main);
            display: flex;
            height: 100vh;
            overflow: hidden;
            line-height: 1.65;
            font-size: 15px;
        }}

        /* ── Top Progress Bar ── */
        #progress-bar {{
            position: fixed;
            top: 0;
            left: 0;
            height: 3px;
            width: 0%;
            background: linear-gradient(90deg, var(--accent), var(--accent-hover), #bb9af7);
            box-shadow: 0 0 10px var(--accent);
            z-index: 9999;
            transition: width 0.25s ease, opacity 0.3s ease;
            pointer-events: none;
            opacity: 0;
        }}

        #progress-bar.active {{
            opacity: 1;
        }}

        /* ── Sidebar ── */
        #sidebar {{
            width: var(--sidebar-width);
            min-width: var(--sidebar-width);
            background-color: var(--bg-sidebar);
            border-right: 1px solid var(--border);
            display: flex;
            flex-direction: column;
            height: 100vh;
            transition: margin-left 0.2s cubic-bezier(0.4, 0, 0.2, 1);
            user-select: none;
            z-index: 10;
        }}

        #sidebar.collapsed {{
            margin-left: calc(-1 * var(--sidebar-width));
        }}

        .sidebar-header {{
            padding: 14px 16px;
            border-bottom: 1px solid var(--border);
            display: flex;
            align-items: center;
            justify-content: space-between;
        }}

        .brand {{
            display: flex;
            align-items: center;
            gap: 9px;
            font-weight: 700;
            font-size: 1.05rem;
            color: var(--fg-main);
            text-decoration: none;
        }}

        .brand-icon {{
            background: linear-gradient(135deg, #7aa2f7, #bb9af7);
            color: #1a1b26;
            width: 24px;
            height: 24px;
            border-radius: 6px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 13px;
            font-weight: 900;
        }}

        .sidebar-search {{
            padding: 10px 14px;
            border-bottom: 1px solid var(--border);
        }}

        .search-box {{
            width: 100%;
            padding: 7px 11px;
            background: var(--bg-panel);
            border: 1px solid var(--border);
            border-radius: 6px;
            color: var(--fg-main);
            font-size: 13px;
            outline: none;
            transition: border-color 0.15s;
        }}

        .search-box:focus {{
            border-color: var(--accent);
        }}

        .sidebar-content {{
            flex: 1;
            overflow-y: auto;
            padding: 10px 8px;
        }}

        .tree-folder {{
            margin-bottom: 3px;
        }}

        .tree-folder-title {{
            display: flex;
            align-items: center;
            gap: 7px;
            padding: 5px 8px;
            border-radius: 5px;
            cursor: pointer;
            font-size: 13.5px;
            font-weight: 600;
            color: var(--fg-muted);
            list-style: none;
        }}

        .tree-folder-title::-webkit-details-marker {{
            display: none;
        }}

        .tree-folder-title:hover {{
            background: var(--bg-hover);
            color: var(--fg-main);
        }}

        .tree-folder-children {{
            padding-left: 14px;
            margin-left: 6px;
            border-left: 1px solid var(--border);
            margin-top: 2px;
        }}

        .tree-file {{
            display: flex;
            align-items: center;
            gap: 7px;
            padding: 5px 8px;
            border-radius: 5px;
            color: var(--fg-main);
            text-decoration: none;
            font-size: 13px;
            margin-bottom: 1px;
            transition: background 0.1s;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }}

        .tree-file:hover {{
            background: var(--bg-hover);
            color: var(--accent);
        }}

        .tree-file.active {{
            background: var(--bg-panel);
            color: var(--accent);
            font-weight: 600;
            border-left: 2px solid var(--accent);
            border-radius: 0 5px 5px 0;
        }}

        .sidebar-footer {{
            padding: 10px 14px;
            border-top: 1px solid var(--border);
            font-size: 11.5px;
            color: var(--fg-dim);
            display: flex;
            justify-content: space-between;
            align-items: center;
        }}

        .kbd {{
            background: var(--bg-panel);
            border: 1px solid var(--border);
            border-radius: 4px;
            padding: 1px 5px;
            font-family: var(--font-mono);
            font-size: 10.5px;
        }}

        /* ── Main Container ── */
        #main-container {{
            flex: 1;
            display: flex;
            flex-direction: column;
            height: 100vh;
            overflow: hidden;
            background-color: var(--bg-main);
        }}

        /* ── Navbar ── */
        .navbar {{
            height: 52px;
            min-height: 52px;
            border-bottom: 1px solid var(--border);
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 0 20px;
            background-color: var(--bg-main);
            z-index: 5;
        }}

        .nav-left {{
            display: flex;
            align-items: center;
            gap: 12px;
        }}

        .nav-btn {{
            background: var(--bg-panel);
            border: 1px solid var(--border);
            border-radius: 6px;
            color: var(--fg-main);
            cursor: pointer;
            padding: 6px 11px;
            font-size: 13px;
            display: flex;
            align-items: center;
            gap: 6px;
            transition: background 0.15s, border-color 0.15s;
        }}

        .nav-btn:hover {{
            background: var(--bg-hover);
            border-color: var(--accent);
        }}

        .breadcrumb {{
            font-family: var(--font-mono);
            font-size: 13px;
            color: var(--fg-muted);
            display: flex;
            align-items: center;
            gap: 6px;
        }}

        .breadcrumb strong {{
            color: var(--fg-main);
        }}

        .nav-right {{
            display: flex;
            align-items: center;
            gap: 10px;
        }}

        .status-badge {{
            display: flex;
            align-items: center;
            gap: 7px;
            font-size: 12px;
            color: var(--fg-muted);
            padding: 4px 10px;
            border-radius: 12px;
            background: var(--bg-panel);
            border: 1px solid var(--border);
            transition: all 0.2s ease;
        }}

        .status-dot {{
            width: 7px;
            height: 7px;
            border-radius: 50%;
            background-color: var(--success);
            box-shadow: 0 0 8px var(--success);
            transition: all 0.2s ease;
        }}

        .status-dot.loading {{
            background-color: var(--warning);
            box-shadow: 0 0 8px var(--warning);
            animation: pulse-spin 0.8s infinite linear;
        }}

        .status-dot.rendering {{
            background-color: var(--accent);
            box-shadow: 0 0 8px var(--accent);
            animation: pulse 1s infinite ease-in-out;
        }}

        @keyframes pulse {{
            0% {{ opacity: 0.5; transform: scale(0.85); }}
            50% {{ opacity: 1; transform: scale(1.15); }}
            100% {{ opacity: 0.5; transform: scale(0.85); }}
        }}

        @keyframes pulse-spin {{
            0% {{ opacity: 0.4; transform: scale(0.8); }}
            50% {{ opacity: 1; transform: scale(1.2); }}
            100% {{ opacity: 0.4; transform: scale(0.8); }}
        }}

        /* ── Scrollable Content Area ── */
        .content-scroll {{
            flex: 1;
            overflow-y: auto;
            padding: 40px 24px 80px;
            scroll-behavior: smooth;
        }}

        .markdown-wrapper {{
            max-width: 880px;
            margin: 0 auto;
            transition: opacity 0.15s ease;
        }}

        .markdown-wrapper.loading {{
            opacity: 0.5;
        }}

        /* ── Markdown Typography ── */
        .markdown-body {{
            color: var(--fg-main);
            font-size: 15.5px;
            line-height: 1.7;
        }}

        .markdown-body h1,
        .markdown-body h2,
        .markdown-body h3,
        .markdown-body h4,
        .markdown-body h5,
        .markdown-body h6 {{
            color: var(--fg-main);
            font-weight: 700;
            margin-top: 1.8em;
            margin-bottom: 0.6em;
            line-height: 1.3;
            position: relative;
        }}

        .markdown-body h1 {{
            font-size: 2.1rem;
            border-bottom: 1px solid var(--border);
            padding-bottom: 0.3em;
            margin-top: 0;
        }}

        .markdown-body h2 {{
            font-size: 1.6rem;
            border-bottom: 1px solid var(--border);
            padding-bottom: 0.25em;
        }}

        .markdown-body h3 {{ font-size: 1.3rem; }}
        .markdown-body h4 {{ font-size: 1.1rem; }}
        .markdown-body h5 {{ font-size: 0.95rem; }}
        .markdown-body h6 {{ font-size: 0.85rem; color: var(--fg-muted); }}

        .markdown-body p {{
            margin-bottom: 1.1em;
        }}

        .markdown-body a {{
            color: var(--accent);
            text-decoration: none;
            border-bottom: 1px solid transparent;
            transition: border-color 0.15s, color 0.15s;
        }}

        .markdown-body a:hover {{
            color: var(--accent-hover);
            border-bottom-color: var(--accent-hover);
        }}

        .markdown-body blockquote {{
            border-left: 4px solid var(--quote-border);
            background: var(--quote-bg);
            padding: 12px 18px;
            margin: 1.2em 0;
            border-radius: 0 8px 8px 0;
            color: var(--fg-muted);
            font-style: italic;
        }}

        .markdown-body blockquote p:last-child {{
            margin-bottom: 0;
        }}

        .markdown-body ul,
        .markdown-body ol {{
            margin-bottom: 1.1em;
            padding-left: 2em;
        }}

        .markdown-body li {{
            margin-bottom: 0.4em;
        }}

        .markdown-body li input[type="checkbox"] {{
            margin-right: 0.45em;
            accent-color: var(--accent);
        }}

        .markdown-body hr {{
            border: 0;
            height: 1px;
            background: var(--border);
            margin: 2.2em 0;
        }}

        /* Code & Pre */
        .markdown-body code {{
            font-family: var(--font-mono);
            background: var(--code-bg);
            padding: 0.2em 0.4em;
            border-radius: 4px;
            font-size: 88%;
            border: 1px solid var(--border);
        }}

        .markdown-body pre {{
            position: relative;
            background: var(--code-bg);
            border: 1px solid var(--border);
            border-radius: 8px;
            margin: 1.2em 0;
            overflow-x: auto;
            padding: 16px;
        }}

        .markdown-body pre code {{
            background: transparent;
            padding: 0;
            border: none;
            font-size: 13.5px;
            line-height: 1.55;
            display: block;
        }}

        .copy-btn {{
            position: absolute;
            top: 8px;
            right: 8px;
            background: var(--bg-hover);
            border: 1px solid var(--border);
            border-radius: 5px;
            color: var(--fg-muted);
            padding: 4px 8px;
            font-size: 11px;
            cursor: pointer;
            opacity: 0;
            transition: opacity 0.15s, color 0.15s, background 0.15s;
        }}

        .markdown-body pre:hover .copy-btn {{
            opacity: 1;
        }}

        .copy-btn:hover {{
            color: var(--fg-main);
            background: var(--bg-panel);
            border-color: var(--accent);
        }}

        /* Tables */
        .markdown-body table {{
            width: 100%;
            border-collapse: collapse;
            margin: 1.4em 0;
            font-size: 14.5px;
            display: block;
            overflow-x: auto;
        }}

        .markdown-body table th,
        .markdown-body table td {{
            border: 1px solid var(--border);
            padding: 9px 14px;
            text-align: left;
        }}

        .markdown-body table th {{
            background: var(--bg-panel);
            font-weight: 600;
        }}

        .markdown-body table tr:nth-child(even) {{
            background: var(--table-alt);
        }}

        /* Images */
        .markdown-body img {{
            max-width: 100%;
            height: auto;
            border-radius: 8px;
            border: 1px solid var(--border);
            margin: 1em 0;
            box-shadow: var(--shadow);
            display: block;
        }}

        /* Math & KaTeX */
        .math-inline {{
            display: inline-block;
            vertical-align: middle;
        }}

        .math-display {{
            display: block;
            overflow-x: auto;
            margin: 1.2em 0;
            padding: 12px 16px;
            background: var(--bg-panel);
            border-radius: 8px;
            border: 1px solid var(--border);
            text-align: center;
        }}

        /* Mermaid Diagrams */
        .mermaid {{
            display: flex;
            justify-content: center;
            background: var(--bg-panel);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 20px;
            margin: 1.4em 0;
            overflow-x: auto;
        }}

        /* Raw View */
        #raw-view {{
            display: none;
            width: 100%;
            height: 70vh;
            font-family: var(--font-mono);
            font-size: 13.5px;
            line-height: 1.55;
            background: var(--code-bg);
            color: var(--fg-main);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 16px;
            white-space: pre-wrap;
            overflow-y: auto;
            resize: none;
            outline: none;
        }}

        /* Directory Dashboard */
        .dir-dashboard {{
            padding: 20px 0;
        }}

        .dir-header {{
            margin-bottom: 24px;
        }}

        .dir-title {{
            font-size: 1.8rem;
            margin-bottom: 6px;
        }}

        .dir-subtitle {{
            color: var(--fg-muted);
        }}

        .dir-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
            gap: 14px;
            margin-top: 20px;
        }}

        .dir-card {{
            background: var(--bg-panel);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 16px;
            text-decoration: none;
            color: var(--fg-main);
            display: flex;
            align-items: center;
            gap: 12px;
            transition: transform 0.15s, border-color 0.15s, box-shadow 0.15s;
        }}

        .dir-card:hover {{
            transform: translateY(-2px);
            border-color: var(--accent);
            box-shadow: var(--shadow);
        }}

        .dir-card-icon {{
            font-size: 1.8rem;
        }}

        .dir-card-info {{
            overflow: hidden;
        }}

        .dir-card-name {{
            font-weight: 600;
            font-size: 14px;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }}

        .dir-card-path {{
            font-size: 12px;
            color: var(--fg-muted);
        }}

        @media (max-width: 768px) {{
            #sidebar {{
                position: absolute;
                left: 0;
                top: 0;
                box-shadow: 0 0 30px rgba(0,0,0,0.5);
            }}
            .content-scroll {{
                padding: 20px 14px 60px;
            }}
        }}
    </style>
</head>
<body>
    <div id="progress-bar"></div>

    <!-- Sidebar -->
    <aside id="sidebar">
        <div class="sidebar-header">
            <a href="/" class="brand">
                <div class="brand-icon">R</div>
                <span>rivas</span>
            </a>
            <button class="nav-btn" onclick="toggleSidebar()" title="Toggle Sidebar (\)">✕</button>
        </div>
        <div class="sidebar-search">
            <input type="text" id="filter-input" class="search-box" placeholder="Filter files (Ctrl+P / /)...">
        </div>
        <div class="sidebar-content">
            {sidebar_html}
        </div>
        <div class="sidebar-footer">
            <span>Rivas Web Preview</span>
            <span><span class="kbd">\</span> toggle</span>
        </div>
    </aside>

    <!-- Main View -->
    <main id="main-container">
        <!-- Top Navbar -->
        <header class="navbar">
            <div class="nav-left">
                <button class="nav-btn" onclick="toggleSidebar()" title="Toggle Sidebar (\)">
                    <span>☰</span> Files
                </button>
                <div class="breadcrumb">
                    <span>📁</span> <strong>{escaped_rel}</strong>
                </div>
            </div>
            <div class="nav-right">
                <div class="status-badge" id="status-pill">
                    <div class="status-dot" id="status-dot"></div>
                    <span id="status-text">Live</span>
                </div>
                <button class="nav-btn" id="theme-btn" onclick="toggleTheme()" title="Switch Theme (Ctrl+T)">
                    🌙
                </button>
                <button class="nav-btn" id="raw-toggle-btn" onclick="toggleRaw()" title="Toggle Raw Markdown">
                    &lt;/&gt; Raw
                </button>
                <button class="nav-btn" onclick="copyDocument()" title="Copy Markdown">
                    📋 Copy
                </button>
            </div>
        </header>

        <!-- Scrollable Document View -->
        <div class="content-scroll">
            <div class="markdown-wrapper">
                <div id="markdown-body" class="markdown-body">
                    {content_html}
                </div>
                <textarea id="raw-view" readonly>{escaped_raw}</textarea>
            </div>
        </div>
    </main>

    <script>
        // ── Theme Management ──
        function initTheme() {{
            const saved = localStorage.getItem('rivas-theme') || 'dark';
            setTheme(saved);
        }}

        function setTheme(theme) {{
            document.documentElement.setAttribute('data-theme', theme);
            localStorage.setItem('rivas-theme', theme);
            const btn = document.getElementById('theme-btn');
            const hljsTheme = document.getElementById('hljs-theme');
            if (theme === 'light') {{
                if (btn) btn.textContent = '☀️';
                if (hljsTheme) hljsTheme.href = 'https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/styles/github.min.css';
            }} else {{
                if (btn) btn.textContent = '🌙';
                if (hljsTheme) hljsTheme.href = 'https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/styles/tokyo-night-dark.min.css';
            }}
        }}

        function toggleTheme() {{
            const current = document.documentElement.getAttribute('data-theme');
            setTheme(current === 'light' ? 'dark' : 'light');
        }}

        initTheme();

        // ── Sidebar Toggle ──
        function toggleSidebar() {{
            const sb = document.getElementById('sidebar');
            sb.classList.toggle('collapsed');
        }}

        // ── Status Indicator ──
        function setStatus(type, text) {{
            const dot = document.getElementById('status-dot');
            const span = document.getElementById('status-text');
            if (!dot || !span) return;
            dot.className = 'status-dot ' + type;
            span.textContent = text;
        }}

        // ── Progress Bar ──
        let progressTimer = null;
        function startProgressBar() {{
            const bar = document.getElementById('progress-bar');
            if (!bar) return;
            clearTimeout(progressTimer);
            bar.classList.add('active');
            bar.style.width = '20%';
            progressTimer = setTimeout(() => {{
                if (bar.classList.contains('active')) {{
                    bar.style.width = '70%';
                }}
            }}, 120);
        }}

        function finishProgressBar() {{
            const bar = document.getElementById('progress-bar');
            if (!bar) return;
            clearTimeout(progressTimer);
            bar.style.width = '100%';
            setTimeout(() => {{
                bar.classList.remove('active');
                setTimeout(() => {{
                    bar.style.width = '0%';
                }}, 250);
            }}, 150);
        }}

        // ── Raw vs Rendered ──
        let isRaw = false;
        function toggleRaw() {{
            isRaw = !isRaw;
            const md = document.getElementById('markdown-body');
            const raw = document.getElementById('raw-view');
            const btn = document.getElementById('raw-toggle-btn');
            if (isRaw) {{
                md.style.display = 'none';
                raw.style.display = 'block';
                btn.textContent = '👁 Preview';
            }} else {{
                raw.style.display = 'none';
                md.style.display = 'block';
                btn.textContent = '</> Raw';
            }}
        }}

        function copyDocument() {{
            const text = document.getElementById('raw-view').value;
            navigator.clipboard.writeText(text).then(() => {{
                alert('Markdown copied to clipboard!');
            }});
        }}

        // ── Rich Content Rendering (KaTeX, Mermaid, Highlight.js) ──
        async function renderRichContent() {{
            const mdBody = document.getElementById('markdown-body');
            if (!mdBody) return;

            // 1. Math Rendering
            if (typeof renderMathInElement !== 'undefined') {{
                setStatus('rendering', 'Rendering math...');
                try {{
                    renderMathInElement(mdBody, {{
                        delimiters: [
                            {{ left: '$$', right: '$$', display: true }},
                            {{ left: '$', right: '$', display: false }},
                            {{ left: '\\[', right: '\\]', display: true }},
                            {{ left: '\\(', right: '\\)', display: false }}
                        ],
                        throwOnError: false
                    }});
                }} catch(e) {{
                    console.error('KaTeX error:', e);
                }}
            }}

            // 2. Mermaid Rendering
            const mermaidBlocks = mdBody.querySelectorAll('pre code.language-mermaid, pre.mermaid');
            if (mermaidBlocks.length > 0 && typeof mermaid !== 'undefined') {{
                setStatus('rendering', `Rendering diagrams (${{mermaidBlocks.length}})...`);
                const isDark = document.documentElement.getAttribute('data-theme') !== 'light';
                mermaid.initialize({{
                    startOnLoad: false,
                    theme: isDark ? 'dark' : 'default',
                    securityLevel: 'loose'
                }});
                for (const el of mermaidBlocks) {{
                    const pre = el.tagName === 'PRE' ? el : el.parentElement;
                    const div = document.createElement('div');
                    div.className = 'mermaid';
                    div.textContent = el.textContent;
                    pre.replaceWith(div);
                }}
                try {{
                    await mermaid.run();
                }} catch(e) {{
                    console.error('Mermaid render error:', e);
                }}
            }}

            // 3. Highlight.js
            if (typeof hljs !== 'undefined') {{
                mdBody.querySelectorAll('pre code:not(.language-mermaid)').forEach(block => {{
                    hljs.highlightElement(block);
                    const pre = block.parentElement;
                    if (!pre.querySelector('.copy-btn')) {{
                        const btn = document.createElement('button');
                        btn.className = 'copy-btn';
                        btn.textContent = 'Copy';
                        btn.onclick = () => {{
                            navigator.clipboard.writeText(block.textContent).then(() => {{
                                btn.textContent = 'Copied!';
                                setTimeout(() => {{ btn.textContent = 'Copy'; }}, 2000);
                            }});
                        }};
                        pre.appendChild(btn);
                    }}
                }});
            }}

            setStatus('', 'Live');
        }}

        // ── Fast Client-Side Navigation (SPA-style without blank page flash) ──
        async function navigateTo(url, pushState = true) {{
            startProgressBar();
            const filename = url.split('/').pop().split('#')[0] || 'page';
            setStatus('loading', 'Loading: ' + decodeURIComponent(filename) + '...');
            const wrapper = document.querySelector('.markdown-wrapper');
            if (wrapper) wrapper.classList.add('loading');

            try {{
                const response = await fetch(url);
                if (!response.ok) {{
                    window.location.href = url;
                    return;
                }}
                const html = await response.text();
                const parser = new DOMParser();
                const doc = parser.parseFromString(html, 'text/html');

                const newBody = doc.getElementById('markdown-body');
                const newRaw = doc.getElementById('raw-view');
                const newBreadcrumb = doc.querySelector('.breadcrumb strong');
                const currentBreadcrumb = document.querySelector('.breadcrumb strong');

                if (newBody && document.getElementById('markdown-body')) {{
                    document.getElementById('markdown-body').innerHTML = newBody.innerHTML;
                }}
                if (newRaw && document.getElementById('raw-view')) {{
                    document.getElementById('raw-view').value = newRaw.value;
                }}
                if (newBreadcrumb && currentBreadcrumb) {{
                    currentBreadcrumb.textContent = newBreadcrumb.textContent;
                }}
                document.title = doc.title;

                if (pushState) {{
                    history.pushState(null, '', url);
                }}

                // Update active state in sidebar
                const urlPath = new URL(url, window.location.origin).pathname;
                document.querySelectorAll('.tree-file').forEach(el => {{
                    const href = el.getAttribute('href');
                    if (href === urlPath) {{
                        el.classList.add('active');
                    }} else {{
                        el.classList.remove('active');
                    }}
                }});

                // Scroll to top or anchor
                const hash = window.location.hash;
                const scrollArea = document.querySelector('.content-scroll');
                if (hash) {{
                    const target = document.querySelector(hash);
                    if (target) {{
                        target.scrollIntoView();
                    }}
                }} else if (scrollArea) {{
                    scrollArea.scrollTop = 0;
                }}

                await renderRichContent();
            }} catch (err) {{
                console.error('Navigation error:', err);
                window.location.href = url;
            }} finally {{
                if (wrapper) wrapper.classList.remove('loading');
                finishProgressBar();
            }}
        }}

        // Intercept sidebar file links and internal markdown links for instant transitions
        document.addEventListener('click', (e) => {{
            const link = e.target.closest('a');
            if (!link) return;
            const href = link.getAttribute('href');
            if (href && (href.startsWith('/view/') || href === '/')) {{
                e.preventDefault();
                navigateTo(href);
            }}
        }});

        window.addEventListener('popstate', () => {{
            navigateTo(window.location.href, false);
        }});

        // Initial render on page load
        document.addEventListener('DOMContentLoaded', () => {{
            renderRichContent();
        }});

        // ── Filter Files in Sidebar ──
        const filterInput = document.getElementById('filter-input');
        if (filterInput) {{
            filterInput.addEventListener('input', (e) => {{
                const q = e.target.value.toLowerCase();
                document.querySelectorAll('.tree-file').forEach(item => {{
                    const name = item.getAttribute('data-name').toLowerCase();
                    if (name.includes(q)) {{
                        item.style.display = 'flex';
                        let p = item.closest('details');
                        while (p) {{
                            p.open = true;
                            p = p.parentElement?.closest('details');
                        }}
                    }} else {{
                        item.style.display = 'none';
                    }}
                }});
            }});
        }}

        // ── Keyboard Shortcuts ──
        window.addEventListener('keydown', (e) => {{
            if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'p') {{
                e.preventDefault();
                filterInput?.focus();
            }} else if (e.key === '/' && document.activeElement.tagName !== 'INPUT' && document.activeElement.tagName !== 'TEXTAREA') {{
                e.preventDefault();
                filterInput?.focus();
            }} else if (e.key === '\\') {{
                if (document.activeElement.tagName !== 'INPUT' && document.activeElement.tagName !== 'TEXTAREA') {{
                    e.preventDefault();
                    toggleSidebar();
                }}
            }} else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 't') {{
                e.preventDefault();
                toggleTheme();
            }}
        }});

        // ── SSE Live Reload with Scroll Persistence ──
        window.addEventListener('beforeunload', () => {{
            const scrollArea = document.querySelector('.content-scroll');
            if (scrollArea) {{
                sessionStorage.setItem('rivas-scroll-pos', scrollArea.scrollTop);
            }}
        }});

        window.addEventListener('load', () => {{
            const saved = sessionStorage.getItem('rivas-scroll-pos');
            if (saved) {{
                const scrollArea = document.querySelector('.content-scroll');
                if (scrollArea) {{
                    scrollArea.scrollTop = parseInt(saved, 10);
                }}
                sessionStorage.removeItem('rivas-scroll-pos');
            }}
        }});

        const evtSource = new EventSource('/api/events');
        evtSource.onmessage = function(e) {{
            if (e.data === 'reload') {{
                navigateTo(window.location.href, false);
            }}
        }};
    </script>
</body>
</html>
"#
    )
}
