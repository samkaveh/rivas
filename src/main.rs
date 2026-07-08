use crate::output::kitty;
use anyhow::Result;
use clap::Parser;
use iocraft::prelude::*;
use std::collections::VecDeque;
use std::fs;
use std::io::{IsTerminal, Read, Write, stdin};
use std::path::PathBuf;
mod assets;
mod components;
mod document;
pub mod editor;
mod file_watcher;
mod lib_file_cache;
pub mod output;
pub mod protocol;
mod theme;

use crate::components::document::Document;
use crate::lib_file_cache::FileListCache;
use skim::prelude::{Skim, SkimItemReader, SkimOptionsBuilder};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, PartialEq)]
enum AppAction {
    #[default]
    Quit,
    SearchFile,
}

#[derive(Parser)]
#[command(
    name = "rivas",
    about = "Terminal markdown viewer and editor with pixel perfect rendering"
)]
struct Cli {
    /// Markdown file to view (reads stdin if omitted)
    file: Option<PathBuf>,
    /// Theme: dark, light
    #[arg(short, long, default_value = "dark")]
    theme: String,
    /// Open a side-by-side markdown editor and live preview.
    #[arg(short, long)]
    edit: bool,
    /// Enable watch mode with Unix socket for CLI interaction
    #[arg(long)]
    watch: bool,
    /// Custom socket path (default: /tmp/rivas-{hash}.sock)
    #[arg(long)]
    socket: Option<PathBuf>,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    let (mut content, mut file_path) = match &cli.file {
        // CASE 1: User provided a path
        Some(path) => {
            if !path.exists() {
                // Create the file (and parent directories) if it doesn't exist
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::File::create(path)?;
                (String::new(), Some(path.clone()))
            } else {
                (fs::read_to_string(path)?, Some(path.clone()))
            }
        }
        // CASE 2: No path provided - Check for Pipe vs. New Default File
        None => {
            if !stdin().is_terminal() {
                // Data is being piped in (e.g., `echo "# Hi" | rivas`)
                let mut s = String::new();
                stdin().read_to_string(&mut s)?;
                (s, None)
            } else {
                // No pipe and no arg: Open a default new file
                let default_path = PathBuf::from("untitled.md");
                if !default_path.exists() {
                    fs::File::create(&default_path)?;
                }
                (String::new(), Some(default_path))
            }
        }
    };

    // Terminal capability check
    let caps = output::capabilities::TermCaps::detect()?;
    if !caps.has_kitty {
        anyhow::bail!("Terminal does not support Kitty, use Kitty, WezTerm or Ghostty.")
    }

    let action = Arc::new(Mutex::new(AppAction::Quit));
    let _edit_mode = cli.edit;

    // Watch mode setup
    if cli.watch {
        let socket_path = cli.socket.unwrap_or_else(|| {
            // Generate a deterministic socket path based on file path
            let path_str = file_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.md".to_string());
            let hash = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            let mut hasher = hash;
            path_str.hash(&mut hasher);
            let hash_val = hasher.finish();
            PathBuf::from(format!("/tmp/rivas-{:x}.sock", hash_val))
        });

        eprintln!("Watch mode enabled. Socket: {}", socket_path.display());

        // Create shared editor state
        let editor_state = Arc::new(Mutex::new(rivas::editor::EditorState::new(
            file_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled.md".to_string()),
            &content,
        )));

        // Create file watcher for external changes
        let file_watcher = if let Some(ref path) = file_path {
            match file_watcher::FileWatcher::new(path.clone()) {
                Ok(watcher) => Some(Arc::new(Mutex::new(watcher))),
                Err(e) => {
                    eprintln!("Warning: Could not start file watcher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Create channels for communication
        let (_cmd_tx, _cmd_rx) = std::sync::mpsc::channel::<(String, std::sync::mpsc::Sender<String>)>();

        // Shared message buffer for socket events (displayed in status bar)
        let socket_messages: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

        // Start socket server thread
        let socket_path_clone = socket_path.clone();
        let editor_state_clone = editor_state.clone();
        let socket_messages_clone = socket_messages.clone();
        let _server_thread = std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            use std::os::unix::net::{UnixListener, UnixStream};

            let listener = match UnixListener::bind(&socket_path_clone) {
                Ok(l) => l,
                Err(e) => {
                    if let Ok(mut msgs) = socket_messages_clone.lock() {
                        msgs.push_back(format!("Socket bind error: {}", e));
                    }
                    return;
                }
            };

            if let Ok(mut msgs) = socket_messages_clone.lock() {
                msgs.push_back("Socket server ready".to_string());
            }

            // Set non-blocking for accept
            listener.set_nonblocking(true).ok();

            let mut streams: Vec<UnixStream> = Vec::new();

            loop {
                // Accept new connections
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).ok();
                            if let Ok(mut msgs) = socket_messages_clone.lock() {
                                msgs.push_back("Client connected".to_string());
                            }
                            streams.push(stream);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => {
                            if let Ok(mut msgs) = socket_messages_clone.lock() {
                                msgs.push_back(format!("Accept error: {}", e));
                            }
                            break;
                        }
                    }
                }

                // Process existing connections
                let mut to_remove = Vec::new();
                for (i, stream) in streams.iter_mut().enumerate() {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            // Client disconnected
                            to_remove.push(i);
                        }
                        Ok(_) => {
                            // Parse and handle message
                            let response = handle_message(&line, &editor_state_clone);
                            let mut writer = BufWriter::new(stream);
                            if let Err(e) = writeln!(writer, "{}", response) {
                                if let Ok(mut msgs) = socket_messages_clone.lock() {
                                    msgs.push_back(format!("Write error: {}", e));
                                }
                                to_remove.push(i);
                            }
                            writer.flush().ok();
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // No data available, continue
                        }
                        Err(e) => {
                            if let Ok(mut msgs) = socket_messages_clone.lock() {
                                msgs.push_back(format!("Read error: {}", e));
                            }
                            to_remove.push(i);
                        }
                    }
                }

                // Remove disconnected streams
                for i in to_remove.into_iter().rev() {
                    streams.remove(i);
                }

                // Brief sleep to avoid busy-waiting
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        // Run TUI with socket server
        loop {
            // Check for external file changes
            if let Some(ref watcher) = file_watcher {
                if let Ok(mut w) = watcher.lock() {
                    if let Some(new_content) = w.check_for_changes() {
                        if new_content != content {
                            content = new_content;
                        }
                    }
                }
            }

            *action.lock().unwrap() = AppAction::Quit;

            smol::block_on(
                element!(App(
                    file_path: file_path.clone(),
                    content: content.as_str(),
                    action: action.clone(),
                    socket_messages: socket_messages.clone(),
                ))
                .fullscreen(),
            )?;

            let next_action = action.lock().unwrap().clone();
            match next_action {
                AppAction::Quit => break,
                AppAction::SearchFile => {
                    if let Some(selected) = run_fuzzy_finder() {
                        if let Some(ref path) = file_path {
                            if let Ok(on_disk) = fs::read_to_string(path) {
                                if on_disk != content {
                                    let _ = fs::write(path, &content);
                                }
                            }
                        }
                        if let Ok(new_content) = fs::read_to_string(&selected) {
                            content = new_content;
                            file_path = Some(selected);
                        }
                    }
                }
            }
        }

        // Cleanup socket
        let _ = std::fs::remove_file(&socket_path);
    } else {
        // Normal mode (no watch)
        loop {
            *action.lock().unwrap() = AppAction::Quit;

            smol::block_on(
                element!(App(
                    file_path: file_path.clone(),
                    content: content.as_str(),
                    action: action.clone(),
                ))
                .fullscreen(),
            )?;

            let next_action = action.lock().unwrap().clone();
            match next_action {
                AppAction::Quit => break,
                AppAction::SearchFile => {
                    if let Some(selected) = run_fuzzy_finder() {
                        if let Some(ref path) = file_path {
                            if let Ok(on_disk) = fs::read_to_string(path) {
                                if on_disk != content {
                                    let _ = fs::write(path, &content);
                                }
                            }
                        }
                        if let Ok(new_content) = fs::read_to_string(&selected) {
                            content = new_content;
                            file_path = Some(selected);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::BufWriter;

fn handle_message(
    msg: &str,
    editor_state: &Arc<Mutex<rivas::editor::EditorState>>,
) -> String {
    use rivas::protocol::types::*;

    let msg = msg.trim();
    if msg.is_empty() {
        return serde_json::to_string(&ServerMessage::Error {
            id: "0".to_string(),
            message: "Empty message".to_string(),
            error_code: None,
        })
        .unwrap_or_default();
    }

    let client_msg: ClientMessage = match serde_json::from_str(msg) {
        Ok(m) => m,
        Err(e) => {
            return serde_json::to_string(&ServerMessage::Error {
                id: "0".to_string(),
                message: format!("Invalid message: {}", e),
                error_code: Some("INVALID_COMMAND".to_string()),
            })
            .unwrap_or_default();
        }
    };

    match client_msg {
        ClientMessage::Command(cmd) => {
            let mut state = editor_state.lock().unwrap();
            let mut editor = rivas::Editor::new(&state.filename, &state.buf.to_text());

            // Apply the vim command
            let result = editor.execute_vim(&cmd.command);

            // Update the shared state
            state.buf = rivas::editor::Buffer::new(&editor.content());
            state.row = result.cursor.row;
            state.col = result.cursor.col;
            state.modified = result.modified;

            let response = CommandResponse {
                id: cmd.id,
                success: result.success,
                message: result.message,
                cursor: CursorPosition::new(result.cursor.row, result.cursor.col),
                modified: result.modified,
                error_code: if result.success { None } else { Some("EDITOR_ERROR".to_string()) },
            };

            serde_json::to_string(&ServerMessage::CommandResponse(response)).unwrap_or_default()
        }
        ClientMessage::Query(query) => {
            let state = editor_state.lock().unwrap();
            let response = match query.query {
                QueryType::Cursor => QueryResponse::Cursor {
                    id: query.id,
                    position: CursorPosition::new(state.row, state.col),
                },
                QueryType::Content => QueryResponse::Content {
                    id: query.id,
                    content: state.buf.to_text(),
                },
                QueryType::Modified => QueryResponse::Modified {
                    id: query.id,
                    modified: state.modified,
                },
                QueryType::Mode => QueryResponse::Mode {
                    id: query.id,
                    mode: format!("{:?}", state.mode),
                },
                QueryType::FileName => QueryResponse::FileName {
                    id: query.id,
                    filename: state.filename.clone(),
                },
                QueryType::Status => QueryResponse::Status {
                    id: query.id,
                    cursor: CursorPosition::new(state.row, state.col),
                    modified: state.modified,
                    mode: format!("{:?}", state.mode),
                    filename: state.filename.clone(),
                    line_count: state.buf.line_count(),
                },
                QueryType::Registers => QueryResponse::Registers {
                    id: query.id,
                    registers: state.registers.clone(),
                },
                QueryType::UndoHistory => QueryResponse::UndoHistory {
                    id: query.id,
                    undo_count: state.undo_stack.len(),
                    redo_count: state.redo_stack.len(),
                },
            };

            serde_json::to_string(&ServerMessage::QueryResponse(response)).unwrap_or_default()
        }
    }
}

#[derive(Default, Props)]
struct AppProps<'a> {
    file_path: Option<PathBuf>,
    content: &'a str,
    action: Arc<Mutex<AppAction>>,
    socket_messages: Option<Arc<Mutex<VecDeque<String>>>>,
}

#[component]
fn App<'a>(props: &AppProps<'a>, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let (width, height) = hooks.use_terminal_size();
    let mut system = hooks.use_context_mut::<SystemContext>();
    let should_exit = hooks.use_state(|| false);
    let path = props.file_path.clone().unwrap_or_default();
    let _path_name = path
        .to_str()
        .filter(|name| !name.is_empty())
        .unwrap_or("untitled.md")
        .to_string();
    let content = hooks.use_state(|| props.content.to_string());
    let mouse_captured = hooks.use_state(|| false);
    let cursor_offset = hooks.use_ref(|| 0usize);

    hooks.use_terminal_events({
        let mut should_exit = should_exit;
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
    let current_content = content.read().clone();
    let on_change = hooks.use_async_handler(move |next_content: String| {
        let mut content = content.clone();
        async move {
            content.set(next_content);
        }
    });
    let on_quit = hooks.use_async_handler(move |()| {
        let mut should_exit = should_exit.clone();
        async move {
            should_exit.set(true);
        }
    });

    element! {
        View(flex_direction: FlexDirection::Column, width, height) {
            Document(
                content: current_content,
                file_path: path,
                viewport_height: height.saturating_sub(1) as u32,
                viewport_width: width as u32,
                keyboard_navigation: Some(true),
                follow_ref: None,
                cursor_offset: Some(cursor_offset),
                on_change,
                on_quit,
                socket_messages: props.socket_messages.clone(),
            )
            View(width: 100pct, height: 1, background_color: theme::STATUS_BG, flex_direction: FlexDirection::Row) {
                View(background_color: theme::DARK_GREY) {
                    Text(content: " :q ", color: theme::FG)
                }
                Text(content: " Quit ")
                View(background_color: theme::DARK_GREY) {
                    Text(content: " C-p ", color: theme::FG)
                }
                Text(content: " Find ")
                View(background_color: theme::DARK_GREY) {
                    Text(content: " j/k ", color: theme::FG)
                }
                Text(content: " Scroll ")
                View(background_color: theme::DARK_GREY) {
                    Text(content: " gg/G ", color: theme::FG)
                }
                Text(content: " Top/Bottom ")
                View(flex_grow: 1.0) {}
            }
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
