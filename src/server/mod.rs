pub mod html;
pub mod renderer;

use anyhow::Result;
use renderer::{build_tree, render_markdown_to_html, render_sidebar_html};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

static RUNNING_SERVER: Mutex<Option<ServerInfo>> = Mutex::new(None);

#[derive(Clone, Debug)]
pub struct ServerInfo {
    pub port: u16,
    pub root_dir: PathBuf,
}

pub fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", url]).spawn();
}

/// Starts the browser server in the background (used by editor commands like `:serve` or `:browser`).
pub fn start_background_server(
    root_dir: PathBuf,
    current_file: Option<PathBuf>,
    preferred_port: u16,
) -> Result<String> {
    let mut guard = RUNNING_SERVER.lock().unwrap();
    if let Some(ref info) = *guard {
        let port = info.port;
        let url = if let Some(ref cf) = current_file {
            let cf_canon = fs::canonicalize(cf).unwrap_or_else(|_| cf.clone());
            let rel = cf_canon.strip_prefix(&info.root_dir).unwrap_or(cf);
            format!("http://localhost:{}/view/{}", port, renderer::url_encode_path(&rel.to_string_lossy()))
        } else {
            format!("http://localhost:{}", port)
        };
        open_in_browser(&url);
        return Ok(url);
    }

    let (listener, port) = bind_listener("127.0.0.1", preferred_port)?;
    let root_clone = root_dir.clone();
    let sse_clients = Arc::new(Mutex::new(Vec::<TcpStream>::new()));

    // Spawn SSE watcher
    spawn_watcher(root_dir.clone(), sse_clients.clone());

    // Spawn server thread
    let sse_subscribers = sse_clients.clone();
    let root_for_serve = root_dir.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let root = root_for_serve.clone();
                let sse_subs = sse_subscribers.clone();
                std::thread::spawn(move || {
                    let _ = handle_connection(stream, &root, sse_subs);
                });
            }
        }
    });

    *guard = Some(ServerInfo {
        port,
        root_dir: root_clone.clone(),
    });

    let url = if let Some(ref cf) = current_file {
        let cf_canon = fs::canonicalize(cf).unwrap_or_else(|_| cf.clone());
        let rel = cf_canon.strip_prefix(&root_clone).unwrap_or(cf);
        format!("http://localhost:{}/view/{}", port, renderer::url_encode_path(&rel.to_string_lossy()))
    } else {
        format!("http://localhost:{}", port)
    };

    open_in_browser(&url);
    Ok(url)
}

/// Runs the standalone browser server (blocking, for CLI `--serve` or `rivas serve`).
pub fn run_server(
    root_dir: PathBuf,
    initial_file: Option<PathBuf>,
    preferred_port: u16,
    open_browser: bool,
    _theme_name: &str,
) -> Result<()> {
    let root_dir = if root_dir.is_dir() {
        fs::canonicalize(&root_dir).unwrap_or(root_dir)
    } else if let Some(parent) = root_dir.parent() {
        fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf())
    } else {
        PathBuf::from(".")
    };

    let (listener, port) = bind_listener("127.0.0.1", preferred_port)?;

    let sse_clients = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
    spawn_watcher(root_dir.clone(), sse_clients.clone());

    let base_url = format!("http://localhost:{}", port);
    let target_url = if let Some(ref f) = initial_file {
        if f.is_file() {
            let f_canon = fs::canonicalize(f).unwrap_or_else(|_| f.clone());
            let rel = f_canon.strip_prefix(&root_dir).unwrap_or(f);
            format!("{}/view/{}", base_url, renderer::url_encode_path(&rel.to_string_lossy()))
        } else {
            base_url.clone()
        }
    } else {
        base_url.clone()
    };

    println!("\x1b[1;34m╔══════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;34m║\x1b[0m                       \x1b[1;37mR I V A S   S E R V E\x1b[0m                          \x1b[1;34m║\x1b[0m");
    println!("\x1b[1;34m╠══════════════════════════════════════════════════════════════════════╣\x1b[0m");
    println!("\x1b[1;34m║\x1b[0m  \x1b[1;32mWorkspace\x1b[0m : {:<54}\x1b[1;34m║\x1b[0m", truncate_str(&root_dir.display().to_string(), 54));
    println!("\x1b[1;34m║\x1b[0m  \x1b[1;36mLocal URL\x1b[0m : {:<54}\x1b[1;34m║\x1b[0m", truncate_str(&target_url, 54));
    println!("\x1b[1;34m║\x1b[0m  \x1b[1;33mLive Sync\x1b[0m : \x1b[32menabled\x1b[0m (hot reload on save)                           \x1b[1;34m║\x1b[0m");
    println!("\x1b[1;34m║\x1b[0m                                                                      \x1b[1;34m║\x1b[0m");
    println!("\x1b[1;34m║\x1b[0m  \x1b[90mPress Ctrl+C to stop the server\x1b[0m                                     \x1b[1;34m║\x1b[0m");
    println!("\x1b[1;34m╚══════════════════════════════════════════════════════════════════════╝\x1b[0m");

    if open_browser {
        open_in_browser(&target_url);
    }

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let root = root_dir.clone();
            let sse_subs = sse_clients.clone();
            std::thread::spawn(move || {
                let _ = handle_connection(stream, &root, sse_subs);
            });
        }
    }

    Ok(())
}

/// Joins a URL-supplied relative path onto the workspace root, refusing
/// anything that could escape it: absolute paths (`Path::join` with an
/// absolute path *replaces* the root), `..`, and drive prefixes. Symlinks are
/// resolved and must still point inside the root.
fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    use std::path::Component;
    let mut out = root.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    match fs::canonicalize(&out) {
        Ok(real) => {
            let root_real = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
            real.starts_with(&root_real).then_some(out)
        }
        // Not existing yet: the lexical check above is sufficient.
        Err(_) => Some(out),
    }
}

/// Display name of the workspace (its folder name).
fn workspace_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "workspace".to_string())
}

/// Whether an HTTP `Host` header names this machine's loopback interface.
fn is_loopback_host(host: &str) -> bool {
    let name = host.rsplit_once(':').map_or(host, |(h, _)| h);
    matches!(name, "localhost" | "127.0.0.1" | "[::1]")
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("...{}", &s[s.len().saturating_sub(max_len - 3)..])
    } else {
        s.to_string()
    }
}

fn bind_listener(host: &str, start_port: u16) -> Result<(TcpListener, u16)> {
    for port in start_port..start_port + 20 {
        if let Ok(listener) = TcpListener::bind(format!("{}:{}", host, port)) {
            return Ok((listener, port));
        }
    }
    let listener = TcpListener::bind(format!("{}:{}", host, start_port))?;
    Ok((listener, start_port))
}

fn spawn_watcher(root_dir: PathBuf, subscribers: Arc<Mutex<Vec<TcpStream>>>) {
    std::thread::spawn(move || {
        let mut file_times = HashMap::<PathBuf, SystemTime>::new();
        // Initial scan
        collect_mtimes(&root_dir, &mut file_times);

        loop {
            std::thread::sleep(Duration::from_millis(500));
            let mut fresh_times = HashMap::new();
            collect_mtimes(&root_dir, &mut fresh_times);

            let mut has_changes = false;
            for (path, mtime) in &fresh_times {
                if let Some(old_mtime) = file_times.get(path) {
                    if mtime > old_mtime {
                        has_changes = true;
                        break;
                    }
                } else {
                    has_changes = true;
                    break;
                }
            }

            if has_changes {
                file_times = fresh_times;
                // Broadcast reload message
                let mut subs = subscribers.lock().unwrap();
                let mut retain = Vec::new();
                for mut stream in subs.drain(..) {
                    if stream.write_all(b"data: reload\n\n").is_ok() && stream.flush().is_ok() {
                        retain.push(stream);
                    }
                }
                *subs = retain;
            }
        }
    });
}

fn collect_mtimes(dir: &Path, out: &mut HashMap<PathBuf, SystemTime>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with('.') || fname == "target" || fname == "node_modules" {
                continue;
            }
            if path.is_dir() {
                collect_mtimes(&path, out);
            } else if path.extension().map_or(false, |ext| ext == "md" || ext == "markdown") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        out.insert(path, mtime);
                    }
                }
            }
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    root_dir: &Path,
    sse_subscribers: Arc<Mutex<Vec<TcpStream>>>,
) -> Result<()> {
    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let method = parts[0];
    let full_path = parts[1];

    // ── Live-preview endpoint ──────────────────────────────────────────────
    // POST /api/render/<rel-path> — renders the request body as Markdown
    // (relative links resolved against <rel-path>) without touching disk.
    if method == "POST" {
        let mut headers = HashMap::<String, String>::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 || line == "\r\n" {
                break;
            }
            if let Some((k, v)) = line.trim().split_once(':') {
                headers.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }
        let rel = full_path
            .split('?')
            .next()
            .map(url_decode)
            .and_then(|p| p.strip_prefix("/api/render/").map(str::to_string));
        let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
        let allowed = headers.contains_key("x-rivas-render")
            && headers.get("host").is_some_and(|h| is_loopback_host(h));
        match rel {
            Some(rel) if allowed && len <= 32 * 1024 * 1024 => {
                let mut body = vec![0u8; len];
                reader.read_exact(&mut body)?;
                let html = render_markdown_to_html(&String::from_utf8_lossy(&body), &rel);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\r\n",
                    html.len()
                );
                stream.write_all(header.as_bytes())?;
                stream.write_all(html.as_bytes())?;
            }
            _ => {
                let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\n\r\nForbidden");
            }
        }
        return Ok(());
    }

    // ── Save endpoint ──────────────────────────────────────────────────────
    // PUT /api/save/<rel-path> — writes the request body back to an existing
    // Markdown file in the workspace and broadcasts a reload to SSE clients.
    //
    // This endpoint writes to disk, so it only accepts same-origin requests
    // from the rivas page itself: no CORS headers are ever sent (a
    // cross-origin preflight fails), the request must carry the custom
    // `X-Rivas-Save` header (which forces a preflight for any cross-origin
    // caller), any `Origin` must be this server, and the `Host` must be a
    // loopback name (defeats DNS rebinding).
    if method == "PUT" {
        let mut headers = HashMap::<String, String>::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 || line == "\r\n" {
                break;
            }
            if let Some((k, v)) = line.trim().split_once(':') {
                headers.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }
        let rel = full_path
            .split('?')
            .next()
            .map(url_decode)
            .and_then(|p| p.strip_prefix("/api/save/").map(str::to_string));
        let host_ok = headers
            .get("host")
            .map(|h| is_loopback_host(h))
            .unwrap_or(false);
        let origin_ok = match headers.get("origin") {
            None => true,
            Some(o) => headers.get("host").is_some_and(|h| {
                o == &format!("http://{h}") && is_loopback_host(h)
            }),
        };
        let marked = headers.contains_key("x-rivas-save");
        let target = rel.as_deref().and_then(|r| safe_join(root_dir, r));
        let writable = target.as_ref().is_some_and(|p| {
            p.is_file() && renderer::is_markdown_name(&p.to_string_lossy())
        });
        if rel.is_none() {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found");
            return Ok(());
        }
        if !(host_ok && origin_ok && marked && writable) {
            let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\n\r\nForbidden");
            return Ok(());
        }
        let content_length: usize = headers
            .get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        // Markdown documents are small; refuse absurd bodies outright.
        if content_length > 32 * 1024 * 1024 {
            let _ = stream.write_all(b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n");
            return Ok(());
        }
        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body)?;
        fs::write(target.as_ref().unwrap(), &body)?;
        {
            let mut subs = sse_subscribers.lock().unwrap();
            let mut retain = Vec::new();
            for mut s in subs.drain(..) {
                if s.write_all(b"data: reload\n\n").is_ok() && s.flush().is_ok() {
                    retain.push(s);
                }
            }
            *subs = retain;
        }
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok")?;
        return Ok(());
    }

    if method != "GET" && method != "HEAD" {
        let response = "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        return Ok(());
    }

    // Strip query string
    let path = full_path.split('?').next().unwrap_or(full_path);
    let decoded_path = url_decode(path);

    // Security check: reject parent path traversal
    if decoded_path.contains("..") {
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\n\r\nForbidden";
        let _ = stream.write_all(resp.as_bytes());
        return Ok(());
    }

    // Routing
    if decoded_path == "/api/events" {
        // SSE connection
        let header = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n";
        stream.write_all(header.as_bytes())?;
        stream.write_all(b": connected\n\n")?;
        stream.flush()?;
        let stream_clone = stream.try_clone()?;
        sse_subscribers.lock().unwrap().push(stream_clone);
        return Ok(());
    }

    if decoded_path == "/api/tree" {
        let tree = build_tree(root_dir, root_dir);
        let json = serde_json::to_string(&tree)?;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            json.len(),
            json
        );
        stream.write_all(resp.as_bytes())?;
        return Ok(());
    }

    if decoded_path == "/" {
        // Check if there is a README or default markdown file to display
        if let Some(first_file) = find_default_markdown(root_dir) {
            let rel = first_file.strip_prefix(root_dir).unwrap_or(&first_file);
            let redirect = format!(
                "HTTP/1.1 302 Found\r\nLocation: /view/{}\r\nContent-Length: 0\r\n\r\n",
                renderer::url_encode_path(&rel.to_string_lossy())
            );
            stream.write_all(redirect.as_bytes())?;
            return Ok(());
        }

        // Otherwise show directory dashboard
        return serve_directory_dashboard(&mut stream, root_dir, "");
    }

    if let Some(rel) = decoded_path.strip_prefix("/view/") {
        let Some(file_path) = safe_join(root_dir, rel) else {
            let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\n\r\nForbidden");
            return Ok(());
        };
        if file_path.is_dir() {
            if let Some(readme) = find_default_markdown(&file_path) {
                let rrel = readme.strip_prefix(root_dir).unwrap_or(&readme);
                let redirect = format!(
                    "HTTP/1.1 302 Found\r\nLocation: /view/{}\r\nContent-Length: 0\r\n\r\n",
                    renderer::url_encode_path(&rrel.to_string_lossy())
                );
                stream.write_all(redirect.as_bytes())?;
                return Ok(());
            }
            return serve_directory_dashboard(&mut stream, root_dir, rel);
        }

        if file_path.is_file() {
            return serve_markdown_page(&mut stream, root_dir, &file_path, rel);
        }

        let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
        let _ = stream.write_all(resp.as_bytes());
        return Ok(());
    }

    if let Some(rel) = decoded_path.strip_prefix("/raw/") {
        return match safe_join(root_dir, rel) {
            Some(file_path) => serve_static_file(&mut stream, &file_path),
            None => {
                let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\n\r\nForbidden");
                Ok(())
            }
        };
    }

    // Fallback: check if path directly exists under root
    if let Some(direct_path) = safe_join(root_dir, decoded_path.trim_start_matches('/')) {
        if direct_path.is_file() {
            return serve_static_file(&mut stream, &direct_path);
        }
    }

    let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
    let _ = stream.write_all(resp.as_bytes());
    Ok(())
}

fn serve_markdown_page(
    stream: &mut TcpStream,
    root_dir: &Path,
    file_path: &Path,
    rel_path: &str,
) -> Result<()> {
    let markdown = fs::read_to_string(file_path)?;
    let content_html = render_markdown_to_html(&markdown, rel_path);

    let tree = build_tree(root_dir, root_dir);
    let sidebar_html = tree
        .as_ref()
        .map(|t| render_sidebar_html(t, rel_path, true))
        .unwrap_or_default();

    let title = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Markdown View".to_string());

    let page_html = html::build_page_html(
        &title,
        &workspace_name(root_dir),
        rel_path,
        &sidebar_html,
        &content_html,
        &markdown,
        false,
    );

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-cache\r\n\r\n",
        page_html.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(page_html.as_bytes())?;
    Ok(())
}

fn serve_directory_dashboard(
    stream: &mut TcpStream,
    root_dir: &Path,
    rel_dir: &str,
) -> Result<()> {
    let target_dir = safe_join(root_dir, rel_dir).unwrap_or_else(|| root_dir.to_path_buf());
    let tree = build_tree(root_dir, root_dir);
    let sidebar_html = tree
        .as_ref()
        .map(|t| render_sidebar_html(t, rel_dir, true))
        .unwrap_or_default();

    let mut cards_html = String::new();
    if let Ok(entries) = fs::read_dir(&target_dir) {
        let mut list: Vec<(bool, String, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    return None;
                }
                let ft = e.file_type().ok()?;
                let is_dir = ft.is_dir() || (ft.is_symlink() && e.path().is_dir());
                Some((is_dir, name, e.path()))
            })
            .collect();
        list.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        for (is_dir, name, path) in list {
            let item_rel = path.strip_prefix(root_dir).unwrap_or(&path);
            let item_rel_str = item_rel.to_string_lossy();
            let is_md = name.ends_with(".md") || name.ends_with(".markdown");
            let icon = if is_dir {
                html::icon("folder")
            } else if is_md {
                html::icon("file")
            } else {
                html::icon("image")
            };
            let link = if is_dir || is_md {
                format!("/view/{}", renderer::url_encode_path(&item_rel_str))
            } else {
                format!("/raw/{}", renderer::url_encode_path(&item_rel_str))
            };
            let detail = if is_dir { "Folder".to_string() } else { item_rel_str.to_string() };
            cards_html.push_str(&format!(
                r#"<a href="{link}" class="dir-card{}"><span class="ico">{icon}</span><span class="meta"><span class="name">{}</span><span class="path">{}</span></span></a>
"#,
                if is_dir { " is-dir" } else { "" },
                renderer::escape_html(&name),
                renderer::escape_html(&detail)
            ));
        }
    }

    let folder_name = if rel_dir.is_empty() {
        root_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Workspace".to_string())
    } else {
        rel_dir.to_string()
    };

    let content_html = format!(
        r#"<div class="dir-head"><h1>{}</h1><p>No README here — pick a document to open.</p></div>
<div class="dir-grid">
{}
</div>"#,
        renderer::escape_html(&folder_name),
        cards_html
    );

    let page_html = html::build_page_html(
        &folder_name,
        &workspace_name(root_dir),
        rel_dir,
        &sidebar_html,
        &content_html,
        "",
        true,
    );

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-cache\r\n\r\n",
        page_html.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(page_html.as_bytes())?;
    Ok(())
}

fn serve_static_file(stream: &mut TcpStream, path: &Path) -> Result<()> {
    if !path.is_file() {
        let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
        let _ = stream.write_all(resp.as_bytes());
        return Ok(());
    }

    let content_type = mime_for_path(path);
    let mut file = fs::File::open(path)?;
    let file_len = file.metadata()?.len();

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: public, max-age=3600\r\n\r\n",
        content_type, file_len
    );
    stream.write_all(header.as_bytes())?;

    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        stream.write_all(&buf[..n])?;
    }
    Ok(())
}

fn mime_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json",
        "html" | "htm" => "text/html; charset=utf-8",
        "md" | "markdown" => "text/markdown; charset=utf-8",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

pub fn find_default_markdown(dir: &Path) -> Option<PathBuf> {
    let candidates = [
        "README.md",
        "readme.md",
        "Readme.md",
        "INDEX.md",
        "index.md",
        "Index.md",
    ];
    for name in &candidates {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    // Search top-level .md files
    if let Ok(entries) = fs::read_dir(dir) {
        let mut md_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension().map_or(false, |ext| ext == "md" || ext == "markdown")
            })
            .collect();
        md_files.sort();
        if let Some(first) = md_files.into_iter().next() {
            return Some(first);
        }
    }
    None
}

fn url_decode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                result.push(val);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            result.push(b' ');
        } else {
            result.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("hello%20world"), "hello world");
        assert_eq!(url_decode("path/to/my%2Dfile.md"), "path/to/my-file.md");
        assert_eq!(url_decode("docs/chapter+1.md"), "docs/chapter 1.md");
    }

    #[test]
    fn test_mime_for_path() {
        assert_eq!(mime_for_path(Path::new("image.png")), "image/png");
        assert_eq!(mime_for_path(Path::new("photo.jpg")), "image/jpeg");
        assert_eq!(mime_for_path(Path::new("doc.svg")), "image/svg+xml");
        assert_eq!(mime_for_path(Path::new("style.css")), "text/css; charset=utf-8");
        assert_eq!(mime_for_path(Path::new("app.js")), "application/javascript; charset=utf-8");
        assert_eq!(mime_for_path(Path::new("readme.md")), "text/markdown; charset=utf-8");
    }

    #[test]
    fn test_find_default_markdown() {
        let temp_dir = std::env::temp_dir().join(format!("rivas_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let readme = temp_dir.join("README.md");
        let _ = fs::write(&readme, "# Hello");

        let found = find_default_markdown(&temp_dir);
        assert_eq!(found, Some(readme));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}


#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn safe_join_rejects_escapes() {
        let root = std::env::temp_dir();
        assert!(safe_join(&root, "/etc/passwd").is_none(), "absolute path must not replace root");
        assert!(safe_join(&root, "../etc/passwd").is_none());
        assert!(safe_join(&root, "a/../../b").is_none());
        assert_eq!(safe_join(&root, "docs/a.md"), Some(root.join("docs/a.md")));
        assert_eq!(safe_join(&root, "./a.md"), Some(root.join("a.md")));
    }

    #[test]
    fn loopback_hosts() {
        assert!(is_loopback_host("localhost:3000"));
        assert!(is_loopback_host("127.0.0.1:8080"));
        assert!(is_loopback_host("[::1]:3000"));
        assert!(!is_loopback_host("evil.example:3000"));
        assert!(!is_loopback_host("localhost.evil.example"));
    }
}
