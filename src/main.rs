use crate::output::graphics_manager;
use crate::output::kitty;
use anyhow::Result;
use clap::Parser;
use iocraft::prelude::*;
use std::fs;
use std::io::{IsTerminal, Read, Write, stdin};
use std::path::PathBuf;
mod assets;
mod components;
mod debug;
mod document;
mod lib_file_cache;
mod output;
mod server;
mod theme;

use crate::components::document::Document;
use crate::components::leader::LeaderAction;
use crate::lib_file_cache::FileListCache;
use skim::prelude::{Skim, SkimItemReader, SkimOptionsBuilder};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, PartialEq)]
enum AppAction {
    #[default]
    Quit,
    SearchFile,
}

/// Switches to `new_path`, first auto-saving the current buffer to *its own*
/// file (never the file being opened) when modified on disk.
fn switch_file(old_path: &PathBuf, new_path: &PathBuf, content: &str) -> anyhow::Result<String> {
    if old_path != new_path && old_path.exists() && old_path.is_file() {
        if let Ok(on_disk) = fs::read_to_string(old_path) {
            if on_disk != content {
                let _ = fs::write(old_path, content);
            }
        }
    }
    Ok(fs::read_to_string(new_path)?)
}

#[derive(Parser)]
#[command(
    name = "rivas",
    version,
    about = "Terminal markdown viewer and editor with pixel perfect rendering"
)]
struct Cli {
    /// Markdown file to view (reads stdin if omitted)
    file: Option<PathBuf>,
    /// Color theme. One of: tokyo-night (default), dracula, nord,
    /// gruvbox-dark, catppuccin-mocha, solarized-light. Cycle at runtime
    /// with Ctrl+T.
    #[arg(short, long, default_value = "tokyo-night")]
    theme: String,
    /// Enable debug mode: JSONL log to rivas-debug.jsonl + visual overlay annotations
    #[arg(long)]
    debug: bool,
    /// Debug JSON logging only — writes rivas-debug.jsonl without visual overlays
    #[arg(long)]
    debug_json_only: bool,
    /// Show debug layout annotations (visual overlay boxes). Can combine with --debug or --debug-json-only
    #[arg(long)]
    debug_annotations: bool,
    /// Math rendering mode. `unicode` (default) shows formulas as Unicode
    /// glyphs that work in any terminal. `image` rasterizes them via the Kitty
    /// graphics protocol for pixel-perfect layout (needs Kitty/WezTerm/Ghostty).
    #[arg(long, value_enum, default_value_t = crate::assets::math::MathMode::Image)]
    math: crate::assets::math::MathMode,
    /// Force Kitty graphics protocol even if the terminal is not recognized.
    /// Use this if your terminal supports Kitty but is not auto-detected.
    #[arg(long)]
    force_kitty: bool,
    /// Override the pixel width of a terminal cell. Graphics are shown at their
    /// natural pixel size mapped onto this cell, so this directly grows/shrinks
    /// images, diagrams and formulas. Use it to correct a terminal whose
    /// `TIOCGWINSZ` pixel size is wrong (e.g. a secondary display). If only one
    /// axis is given, the other defaults to its standard value (16).
    #[arg(long)]
    cell_width: Option<u16>,
    /// Override the pixel height of a terminal cell (see `--cell-width`).
    /// If only one axis is given, the other defaults to 8.
    #[arg(long)]
    cell_height: Option<u16>,
    /// Serve in the browser with live reload and full browser rendering
    #[arg(long)]
    serve: bool,
    /// Port for the browser server (default: 3000)
    #[arg(long, default_value_t = 3000)]
    port: u16,
    /// Do not automatically open the browser when serving
    #[arg(long)]
    no_open: bool,
}

/// Whether debug JSON logging is enabled (from --debug or --debug-json-only)
fn debug_json_enabled(cli: &Cli) -> bool {
    cli.debug || cli.debug_json_only
}

/// Whether visual debug annotations should be shown
fn debug_annotations_enabled(cli: &Cli) -> bool {
    cli.debug_annotations || cli.debug
}

fn find_initial_file_in_dir(dir: &std::path::Path) -> (Option<PathBuf>, String) {
    if let Some(path) = server::find_default_markdown(dir) {
        if let Ok(text) = fs::read_to_string(&path) {
            return (Some(path), text);
        }
    }
    (None, String::new())
}

fn main() -> Result<()> {
    env_logger::init();

    // Support `rivas serve [path] [options]` by converting first argument "serve" to "--serve"
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let mut rewritten_args = Vec::new();
    for (i, arg) in args.iter().enumerate() {
        if i == 1 && arg == "serve" {
            rewritten_args.push(std::ffi::OsString::from("--serve"));
        } else {
            rewritten_args.push(arg.clone());
        }
    }
    let cli = Cli::parse_from(rewritten_args);

    if cli.serve {
        let input_path = cli.file.unwrap_or_else(|| PathBuf::from("."));
        let (root_dir, initial_file) = if input_path.is_file() {
            let parent = input_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf();
            (parent, Some(input_path))
        } else {
            let init = server::find_default_markdown(&input_path);
            (input_path, init)
        };
        return server::run_server(root_dir, initial_file, cli.port, !cli.no_open, &cli.theme);
    }

    if !theme::set_by_name(&cli.theme) {
        anyhow::bail!(
            "Unknown theme '{}'. Available themes: {}",
            cli.theme,
            theme::names().collect::<Vec<_>>().join(", ")
        );
    }
    debug::init(debug_json_enabled(&cli), debug_annotations_enabled(&cli));
    crate::assets::math::set_math_mode(cli.math);

    let (mut content, mut file_path, base_dir, initial_show_tree) = match &cli.file {
        // CASE 1: User provided a path
        Some(path) => {
            if path.is_dir() {
                let canon = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
                let (file, text) = find_initial_file_in_dir(&canon);
                (text, file, canon, true)
            } else if path.exists() {
                let text = fs::read_to_string(path)?;
                let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
                let canon_dir = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
                (text, Some(path.clone()), canon_dir, false)
            } else {
                // Create the file (and parent directories) if it doesn't exist
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent)?;
                    }
                }
                fs::File::create(path)?;
                let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
                let canon_dir = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
                (String::new(), Some(path.clone()), canon_dir, false)
            }
        }
        // CASE 2: No path provided - Check for Pipe vs. Current Directory
        None => {
            if !stdin().is_terminal() {
                // Data is being piped in (e.g., `echo "# Hi" | rivas`)
                let mut s = String::new();
                stdin().read_to_string(&mut s)?;
                let cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                (s, None, cur, false)
            } else {
                // No pipe and no arg: Open current directory and show file tree!
                let cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let (file, text) = find_initial_file_in_dir(&cur);
                (text, file, cur, true)
            }
        }
    };

    // Terminal capability check
    match (cli.cell_width, cli.cell_height) {
        (Some(w), Some(h)) => output::capabilities::override_cell_size(w, h),
        (Some(w), None) => output::capabilities::override_cell_size(w, 16),
        (None, Some(h)) => output::capabilities::override_cell_size(8, h),
        (None, None) => {}
    }
    let _caps = output::capabilities::TermCaps::detect()?;
    debug::log_event(&debug::DebugEvent::TermCaps {
        ts: debug::elapsed_ms(),
        cell_w: _caps.cell_w_px,
        cell_h: _caps.cell_h_px,
        overridden: cli.cell_width.is_some() || cli.cell_height.is_some(),
    });
    if cli.force_kitty {
        output::capabilities::force_kitty();
    }
    if !output::capabilities::has_kitty() {
        if cli.math == crate::assets::math::MathMode::Image {
            log::warn!(
                "Terminal does not support Kitty graphics; falling back to Unicode math mode."
            );
            crate::assets::math::set_math_mode(crate::assets::math::MathMode::Unicode);
        } else {
            log::warn!(
                "Terminal does not support Kitty graphics; images and diagrams will show as text."
            );
        }
    }

    let current_file_path = Arc::new(Mutex::new(file_path.clone()));
    let current_content = Arc::new(Mutex::new(content.clone()));
    let action = Arc::new(Mutex::new(AppAction::Quit));

    loop {
        *action.lock().unwrap() = AppAction::Quit;

        smol::block_on(
            element!(App(
                file_path: file_path.clone(),
                base_dir: base_dir.clone(),
                initial_show_tree: initial_show_tree,
                content: content.as_str(),
                action: action.clone(),
                debug: debug_json_enabled(&cli),
                debug_annotations: debug_annotations_enabled(&cli),
                current_file_path: current_file_path.clone(),
                current_content: current_content.clone(),
            ))
            .fullscreen(),
        )?;

        let next_action = action.lock().unwrap().clone();
        match next_action {
            AppAction::Quit => break,
            AppAction::SearchFile => {
                file_path = current_file_path.lock().unwrap().clone();
                content = current_content.lock().unwrap().clone();
                if let Some(selected) = run_fuzzy_finder() {
                    // Auto-save current file if it exists and content is modified
                    if let Some(ref path) = file_path {
                        if path.is_file() {
                            if let Ok(on_disk) = fs::read_to_string(path) {
                                if on_disk != content {
                                    let _ = fs::write(path, &content);
                                }
                            }
                        }
                    }
                    if let Ok(new_content) = fs::read_to_string(&selected) {
                        content = new_content.clone();
                        file_path = Some(selected.clone());
                        *current_file_path.lock().unwrap() = Some(selected);
                        *current_content.lock().unwrap() = new_content;
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Default, Props)]
struct AppProps<'a> {
    file_path: Option<PathBuf>,
    base_dir: PathBuf,
    initial_show_tree: bool,
    content: &'a str,
    action: Arc<Mutex<AppAction>>,
    debug: bool,
    debug_annotations: bool,
    current_file_path: Arc<Mutex<Option<PathBuf>>>,
    current_content: Arc<Mutex<String>>,
}

#[component]
fn App<'a>(props: &AppProps<'a>, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let (width, height) = hooks.use_terminal_size();
    let mut system = hooks.use_context_mut::<SystemContext>();
    let should_exit = hooks.use_state(|| false);
    let content = hooks.use_state(|| props.content.to_string());
    let current_path = hooks.use_state(|| props.file_path.clone());
    let path = {
        let p = current_path.read().clone();
        p.unwrap_or_else(|| props.base_dir.clone())
    };
    let mouse_captured = hooks.use_state(|| false);
    let cursor_offset = hooks.use_ref(|| 0usize);
    // Feedback for file-open actions (success or failure) shown in the bar.
    let status_msg = hooks.use_state(|| None::<(bool, String)>);
    // Notices are transient: clear each one a few seconds after it appears.
    let notice_text = status_msg.read().as_ref().map(|(_, t)| t.clone());
    hooks.use_future({
        let mut status_msg = status_msg.clone();
        async move {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(250)).await;
                let current = status_msg.read().as_ref().map(|(_, t)| t.clone());
                if current.is_some() {
                    let seen = current.clone();
                    smol::Timer::after(std::time::Duration::from_millis(3000)).await;
                    if status_msg.read().as_ref().map(|(_, t)| t.clone()) == seen {
                        status_msg.set(None);
                    }
                }
            }
        }
    });
    let _ = notice_text;
    // Tick counter used to drive re-renders while graphics are loading.
    // The future polls pending_graphics_count() every 100 ms and bumps the
    // counter whenever rendering is in progress so the status badge updates.
    let mut render_tick = hooks.use_state(|| 0u8);
    hooks.use_future(async move {
        loop {
            smol::Timer::after(std::time::Duration::from_millis(100)).await;
            if graphics_manager::pending_graphics_count() > 0 {
                render_tick.set(render_tick.get().wrapping_add(1));
            }
        }
    });

    hooks.use_terminal_events({
        let mut should_exit = should_exit;
        let mut status_msg = status_msg.clone();
        let action = props.action.clone();
        move |event| match event {
            TerminalEvent::Key(KeyEvent {
                code,
                modifiers,
                kind,
                ..
            }) if kind != KeyEventKind::Release => {
                let ctrl = modifiers.contains(KeyModifiers::CONTROL);
                if ctrl && code == KeyCode::Char('p') {
                    *action.lock().unwrap() = AppAction::SearchFile;
                    should_exit.set(true);
                } else if ctrl && code == KeyCode::Char('t') {
                    theme::cycle();
                    status_msg.set(Some((true, format!("Theme: {}", theme::current().name))));
                }
            }
            _ => {}
        }
    });

    if should_exit.get() {
        if kitty::is_supported() {
            let mut stdout = std::io::stdout().lock();
            kitty::delete_all(&mut stdout);
            let _ = stdout.flush();
        }
        system.exit();
    }

    system.set_mouse_capture(mouse_captured.get());
    // Flush any pending kitty graphic commands buffered by the background
    // graphics thread.  Must happen here (main render thread) so the writes
    // are atomic w.r.t. iocraft/crossterm output.
    graphics_manager::flush_output();
    let current_content = content.read().clone();
    let on_change = {
        let content = content.clone();
        let shared = props.current_content.clone();
        hooks.use_async_handler(move |next_content: String| {
            let mut content = content.clone();
            let shared = shared.clone();
            async move {
                *shared.lock().unwrap() = next_content.clone();
                content.set(next_content);
            }
        })
    };
    let on_quit = hooks.use_async_handler(move |()| {
        let mut should_exit = should_exit.clone();
        async move {
            should_exit.set(true);
        }
    });
    // Sidebar file tree: open the selected file (auto-saving the current one).
    let on_open_file = {
        let content = content.clone();
        let current_path = current_path.clone();
        let status_msg = status_msg.clone();
        let shared_path = props.current_file_path.clone();
        let shared_content = props.current_content.clone();
        hooks.use_async_handler(move |path: PathBuf| {
            let mut content = content.clone();
            let mut current_path = current_path.clone();
            let mut status_msg = status_msg.clone();
            let shared_path = shared_path.clone();
            let shared_content = shared_content.clone();
            async move {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                status_msg.set(Some((true, format!("Loading: {name}..."))));
                let old_path = current_path.read().clone().unwrap_or_default();
                let buffer = content.read().clone();
                match switch_file(&old_path, &path, &buffer) {
                    Ok(text) => {
                        *shared_path.lock().unwrap() = Some(path.clone());
                        *shared_content.lock().unwrap() = text.clone();
                        content.set(text);
                        current_path.set(Some(path.clone()));
                        if old_path == path {
                            status_msg.set(Some((true, format!("Opened: {name}"))));
                        } else {
                            status_msg.set(Some((true, format!("Saved & opened: {name}"))));
                        }
                    }
                    Err(_) => {
                        status_msg.set(Some((false, format!("✗ Not a text file: {name}"))));
                    }
                }
            }
        })
    };

    // Leader-menu actions that live at the app level.
    let on_app_action = {
        let should_exit = should_exit.clone();
        let status_msg = status_msg.clone();
        let action = props.action.clone();
        hooks.use_async_handler(move |a: LeaderAction| {
            let mut should_exit = should_exit.clone();
            let mut status_msg = status_msg.clone();
            let action = action.clone();
            async move {
                match a {
                    LeaderAction::FindFile => {
                        *action.lock().unwrap() = AppAction::SearchFile;
                        should_exit.set(true);
                    }
                    LeaderAction::NextTheme => {
                        theme::cycle();
                        status_msg.set(Some((true, format!("Theme: {}", theme::current().name))));
                    }
                    _ => {}
                }
            }
        })
    };

    // `render_tick` is read so the status line re-renders while graphics load.
    let _t = render_tick.get();
    let pending_graphics = graphics_manager::pending_graphics_count();

    element! {
        View(flex_direction: FlexDirection::Column, width, height) {
            Document(
                content: current_content,
                file_path: path,
                base_dir: Some(props.base_dir.clone()),
                initial_show_tree: Some(props.initial_show_tree),
                viewport_height: height as u32,
                viewport_width: width as u32,
                keyboard_navigation: Some(true),
                follow_ref: None,
                cursor_offset: Some(cursor_offset),
                debug: props.debug,
                debug_annotations: props.debug_annotations,
                on_change,
                on_quit,
                on_open_file,
                on_app_action,
                notice: status_msg.read().clone(),
                pending_graphics,
            )
        }
    }
}

// Global file cache with 5 second TTL (revalidates on each Ctrl+P)
lazy_static::lazy_static! {
    static ref FILE_CACHE: FileListCache = FileListCache::new();
}

fn visit_dirs(dir: &std::path::Path, files: &mut Vec<String>) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && (name.starts_with('.')
                        || name == "target"
                        || name == "node_modules"
                        || name == "build"
                        || name == "dist"
                        || name == "venv"
                        || name == "env")
                {
                    continue;
                }
                visit_dirs(&path, files)?;
            } else if let Some(path_str) = path.to_str() {
                let relative_path = path_str.strip_prefix("./").unwrap_or(path_str).to_string();
                files.push(relative_path);
            }
        }
    }
    Ok(())
}

/// Get local files with caching (5 second TTL)
fn get_local_files() -> Vec<String> {
    // Check cache first (5 second TTL)
    if let Some(cached) = FILE_CACHE.get(5) {
        return cached;
    }

    // Cache miss - rescan filesystem
    let mut files = Vec::new();
    let _ = visit_dirs(std::path::Path::new("."), &mut files);

    // Store in cache for next call
    FILE_CACHE.set(files.clone());

    files
}

fn run_fuzzy_finder() -> Option<PathBuf> {
    let files = get_local_files();
    let input = files.join("\n");

    let options = SkimOptionsBuilder::default()
        .height("40%".to_string())
        .multi(false)
        .prompt("Find File> ".to_string())
        .build()
        .unwrap();

    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(std::io::Cursor::new(input));

    let output = Skim::run_with(options, Some(items)).ok()?;

    if output.is_abort {
        return None;
    }

    let selected = output.selected_items.first()?;
    Some(PathBuf::from(selected.output().to_string()))
}
