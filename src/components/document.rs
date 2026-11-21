use std::path::PathBuf;

use iocraft::prelude::*;

use crate::components::blocks_renderer::BlocksRenderer;
use crate::components::editor::{Buffer, EditorState, Mode, handle_key};
use crate::components::help::HelpOverlay;
use crate::components::leader::{self, LeaderAction, LeaderMenu, Step};
use crate::components::file_tree::{FileNode, FileTreePanel, tree_rows};
use crate::components::scroll::{
    ScrollDelta, ScrollIntent, Viewport, intent_from_key, scroll_delta,
};
use crate::debug;
use crate::document::cache::ParseCache;
use crate::document::parser::parse_markdown;
use crate::theme;

/// Properties for the [`Document`] component.
#[derive(Default, Props)]
pub struct DocumentProps {
    /// The Markdown source text to render.
    pub content: String,
    /// File path for resolving relative image/link URLs.
    pub file_path: PathBuf,
    /// Base directory for the workspace / file tree.
    pub base_dir: Option<PathBuf>,
    /// Whether to open the file tree sidebar initially.
    pub initial_show_tree: Option<bool>,
    /// Height of the visible viewport in lines (for scrolling).
    pub viewport_height: Option<u32>,
    /// Width of the visible viewport in columns (for line wrapping).
    pub viewport_width: Option<u32>,
    /// `true` to enable keyboard navigation (scrolling, editing).
    pub keyboard_navigation: Option<bool>,
    /// Reference to scroll a specific element into view (e.g., a link target).
    pub follow_ref: Option<Ref<usize>>,
    /// Reference to the cursor byte offset for editor cursor rendering.
    pub cursor_offset: Option<Ref<usize>>,
    /// `true` to enable debug overlay rendering.
    pub debug: bool,
    /// `true` to render inline debug annotations.
    pub debug_annotations: bool,
    /// Callback fired when the buffer content changes (for auto-save, etc.).
    pub on_change: Handler<String>,
    /// Callback fired when the user requests quit (`:q`, `ZZ`, Ctrl-C).
    pub on_quit: Handler<()>,
    /// Callback fired when a file is selected in the sidebar file tree.
    pub on_open_file: Option<Handler<PathBuf>>,
    /// Callback for leader actions the app handles (find file, next theme).
    pub on_app_action: Option<Handler<LeaderAction>>,
    /// Transient app-level notice for the status line: `(ok, text)`.
    pub notice: Option<(bool, String)>,
    /// Number of images/diagrams/formulas still rendering in the background.
    pub pending_graphics: usize,
}

/// Width of the file tree side panel, when open.
const TREE_WIDTH: u32 = 32;
/// Maximum width of the text column. Long lines are hard to track: the
/// column is capped and centered on wide terminals (code, tables and
/// diagrams use the same column so everything shares one left edge).
const MAX_COLUMN: u32 = 96;
/// Minimum blank columns on each side of the text column (the left side also
/// hosts the edit marker).
const MIN_SIDE: u32 = 3;

/// Top-level document component combining the editor, block renderer, and status bar.
///
/// Manages the editor state, handles keyboard input, scroll navigation, and
/// renders the parsed Markdown with an optional cursor overlay in edit mode.
#[component]
pub fn Document(props: &DocumentProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let content_prop = props.content.clone();

    let editor_state = hooks.use_ref(|| {
        Some(EditorState::new(
            props.file_path.to_string_lossy().to_string(),
            &content_prop,
        ))
    });

    // Use the editor_state for rendering status and passing to blocks_renderer
    let state_guard = editor_state.read();
    let current_mode = state_guard
        .as_ref()
        .map(|s| s.mode.clone())
        .unwrap_or(Mode::Normal);
    let current_msg = state_guard
        .as_ref()
        .map(|s| s.message.clone())
        .unwrap_or_default();
    let current_cmd = state_guard
        .as_ref()
        .map(|s| s.cmd_buf.clone())
        .unwrap_or_default();
    let cursor_row_col = state_guard.as_ref().and_then(|s| Some((s.row, s.col)));
    let is_modified = state_guard.as_ref().map(|s| s.modified).unwrap_or(false);
    let line_count = state_guard.as_ref().map(|s| s.buf.line_count()).unwrap_or(1);
    let cursor_line_preview = state_guard.as_ref().and_then(|s| {
        let (row, col) = (s.row, s.col);
        let line = s.buf.line(row);
        let byte = s.buf.byte_offset(row, col);
        let before = &line[..byte.min(line.len())];
        let (cursor_ch, after) = if col < s.buf.char_count(row) {
            let c = line[byte..].chars().next()?;
            let char_len = c.len_utf8();
            let after_byte = (byte + char_len).min(line.len());
            (c.to_string(), line[after_byte..].to_string())
        } else {
            (" ".to_string(), String::new())
        };
        Some((before.to_string(), cursor_ch, after))
    });
    drop(state_guard);

    // To trigger re-renders on edit
    let tick = hooks.use_state(|| 0u64);

    // Keep editor state in sync with content prop if it changes externally
    hooks.use_effect(
        {
            let mut editor_state = editor_state.clone();
            let content = content_prop.clone();
            let path_str = props.file_path.to_string_lossy().to_string();
            move || {
                if let Some(s) = editor_state.write().as_mut() {
                    s.filename = path_str.clone();
                    if s.buf.to_text() != content {
                        s.buf = Buffer::new(&content);
                    }
                }
            }
        },
        (content_prop.clone(), props.file_path.clone()),
    );

    // Use parse cache to memoize markdown parsing
    let cache = hooks.use_ref(|| ParseCache::new());
    let current_content = editor_state
        .read()
        .as_ref()
        .map(|s| s.buf.to_text())
        .unwrap_or_default();
    let doc = if let Some(cached_doc) = cache.read().get(&current_content) {
        cached_doc
    } else {
        let parsed = parse_markdown(&current_content);
        cache.read().insert(&current_content, parsed.clone());
        parsed
    };

    let vh = props.viewport_height;
    let vw = props.viewport_width;
    let _keyboard_navigation = props.keyboard_navigation.unwrap_or(true);
    let scroll_handle = hooks.use_ref_default::<ScrollViewHandle>();
    let mut pending_g = hooks.use_state(|| false);
    // `stick_to_bottom` is set when the user presses `G`/`End`. Because the
    // document's *measured* content height can still grow after that press
    // (async Kitty image/graphic loads increase `ScrollView`'s measured
    // height), a single `scroll_to_bottom()` can land short of the true end.
    // We re-pin to the bottom on every frame until the viewport is actually at
    // the measured bottom, then clear the intent.
    let mut stick_to_bottom = hooks.use_state(|| false);
    let _follow_ref = props.follow_ref;
    let on_change = props.on_change.clone();
    let on_quit = props.on_quit.clone();

    // ── File tree sidebar state ────────────────────────────────────────────
    let show_tree = hooks.use_state(|| props.initial_show_tree.unwrap_or(false));
    let base_workspace = props
        .base_dir
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let tree_root = hooks.use_ref(|| {
        if props.initial_show_tree.unwrap_or(false) {
            FileNode::from_dir(&base_workspace)
        } else {
            None
        }
    });
    // Start the selection on the open file when the tree opens at launch.
    let mut tree_selected = hooks.use_state(|| {
        let file = std::fs::canonicalize(&props.file_path).unwrap_or_else(|_| props.file_path.clone());
        tree_root
            .read()
            .as_ref()
            .and_then(|root| tree_rows(root, "").iter().position(|n| n.path == file))
            .unwrap_or(0)
    });
    // Keyboard reference overlay (F1 / `:help`).
    let show_help = hooks.use_state(|| false);
    // Whether the explorer (rather than the document) receives keys.
    let tree_focus = hooks.use_state(|| props.initial_show_tree.unwrap_or(false));
    // Keys typed after Space while the leader menu is open.
    let leader = hooks.use_state(|| None::<Vec<char>>);
    // Search filter string typed with `/` while tree is open.
    let tree_search = hooks.use_state(|| String::new());
    // Whether the tree is in search-input mode (after pressing `/`).
    let tree_searching = hooks.use_state(|| false);
    let on_open_file = props.on_open_file.clone();

    // Flattened visible rows for rendering + selection math (recomputed each
    // frame; cheap relative to parsing/rendering).
    // Workspace search results are cached per query: walking the disk on
    // every repaint would make typing in the filter sluggish.
    let search_cache = hooks.use_ref(|| (String::new(), Vec::<FileNode>::new()));
    let search_query = tree_search.read().clone();
    let visible_tree: Vec<FileNode> = if show_tree.get() {
        let guard = tree_root.read();
        match guard.as_ref() {
            Some(root) => cached_tree_rows(root, &search_query, search_cache.clone()),
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    // Clamp selection into range after collapses/expansions.
    {
        let max = visible_tree.len().saturating_sub(1);
        if tree_selected.get() > max {
            tree_selected.set(max);
        }
    }

    // Re-pin to the bottom while `stick_to_bottom` is set. Runs every render so
    // that once the (async, image-loaded) measured content height grows, the
    // viewport catches up to the true end instead of stopping short. When the
    // viewport is already at the measured bottom the intent is cleared.
    hooks.use_effect(
        {
            let mut scroll_handle = scroll_handle.clone();
            let mut stick_to_bottom = stick_to_bottom.clone();
            move || {
                if !stick_to_bottom.get() {
                    return;
                }
                // Use the measured content height. With virtualization disabled,
                // `ScrollView` always measures the full document, so this is the
                // true total and `scroll_to_bottom` lands exactly at the end even
                // as async graphic loads grow the content.
                let content_h = scroll_handle.read().content_height() as i32;
                let vph = scroll_handle.read().viewport_height() as i32;
                let off = scroll_handle.read().scroll_offset();
                let target = (content_h - vph).max(0);
                let at_bottom = off >= target;
                debug::log_event(&debug::DebugEvent::StickBottom {
                    ts: debug::elapsed_ms(),
                    active: stick_to_bottom.get(),
                    content_h,
                    off,
                    target,
                    repin: !at_bottom,
                });
                if at_bottom {
                    stick_to_bottom.set(false);
                } else {
                    scroll_handle.write().scroll_to_bottom();
                }
            }
        },
        (scroll_handle.read().scroll_offset(),),
    );

    hooks.use_terminal_events({
        let mut scroll_handle = scroll_handle;
        let _content = current_content.clone();
        let cursor_offset = props.cursor_offset.clone();
        let mut editor_state = editor_state.clone();
        let mut tick = tick.clone();
        let on_change = on_change.clone();
        let on_quit = on_quit.clone();
        let mut show_tree = show_tree.clone();
        let mut tree_root = tree_root.clone();
        let mut tree_selected = tree_selected.clone();
        let mut tree_search = tree_search.clone();
        let mut tree_searching = tree_searching.clone();
        let mut show_help = show_help.clone();
        let mut tree_focus = tree_focus.clone();
        let mut leader = leader.clone();
        let on_app_action = props.on_app_action.clone();
        // Tree paths are canonical; match the open file the same way.
        let current_file = std::fs::canonicalize(&props.file_path).unwrap_or_else(|_| props.file_path.clone());
        let search_cache = search_cache.clone();
        let on_open_file = on_open_file.clone();
        // Tree root = the workspace or directory rivas was launched from.
        let tree_base_dir = props
            .base_dir
            .clone()
            .or_else(|| std::env::current_dir().ok());
        move |event| {
            let TerminalEvent::Key(KeyEvent {
                code,
                modifiers,
                kind,
                ..
            }) = event
            else {
                return;
            };

            if kind == KeyEventKind::Release {
                return;
            }

            let ctrl = modifiers.contains(KeyModifiers::CONTROL);

            // ── Help overlay ───────────────────────────────────────────────
            // While open it swallows every key; Esc / q / F1 / Enter close it.
            if show_help.get() {
                if matches!(code, KeyCode::Esc | KeyCode::F(1) | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?')) {
                    show_help.set(false);
                    tick.set(tick.get().wrapping_add(1));
                }
                return;
            }
            if code == KeyCode::F(1) {
                show_help.set(true);
                tick.set(tick.get().wrapping_add(1));
                return;
            }

            // An action chosen from the leader menu or a direct shortcut; run
            // by the single dispatcher below.
            let mut action: Option<LeaderAction> = None;

            // ── Leader menu (Space) ────────────────────────────────────────
            let leader_path = leader.read().clone();
            if let Some(mut path) = leader_path {
                match code {
                    KeyCode::Backspace if !path.is_empty() => {
                        path.pop();
                        leader.set(Some(path));
                    }
                    KeyCode::Char(c) if !ctrl => match leader::step(&path, c) {
                        Step::Descend => {
                            path.push(c);
                            leader.set(Some(path));
                        }
                        Step::Run(a) => {
                            leader.set(None);
                            action = Some(a);
                        }
                        Step::Unbound => leader.set(None),
                    },
                    _ => leader.set(None),
                }
                tick.set(tick.get().wrapping_add(1));
                if action.is_none() {
                    return;
                }
            }

            let editor_idle = editor_state.read().as_ref().map_or(true, |s| {
                s.mode == Mode::Normal
                    && s.operator.is_none()
                    && s.pending.is_none()
                    && s.count_buf.is_empty()
            }) && !pending_g.get();
            let tree_open = show_tree.get();
            let tree_has_focus = tree_open && tree_focus.get();

            if action.is_none() && !ctrl && !tree_searching.get() {
                if code == KeyCode::Char(' ') && (editor_idle || tree_has_focus) {
                    leader.set(Some(Vec::new()));
                    tick.set(tick.get().wrapping_add(1));
                    return;
                }
                // `\` shows the explorer, focuses it, or hides it.
                if code == KeyCode::Char('\\') && (editor_idle || tree_has_focus) {
                    action = Some(LeaderAction::Explorer);
                }
            }

            // ── Pane focus: explorer ⇄ document ────────────────────────────
            // Ctrl-h / Ctrl-l move left / right like window navigation in
            // Vim; Tab flips between the two. Many terminals send Ctrl-h as
            // Backspace, so Backspace in an idle document also moves left.
            if action.is_none() && tree_open && !tree_searching.get() {
                let to_tree = !tree_has_focus
                    && ((ctrl && code == KeyCode::Char('h'))
                        || (editor_idle && matches!(code, KeyCode::Tab | KeyCode::Backspace)));
                let to_doc = tree_has_focus
                    && ((ctrl && code == KeyCode::Char('l')) || code == KeyCode::Tab);
                if to_tree || to_doc {
                    tree_focus.set(to_tree);
                    tick.set(tick.get().wrapping_add(1));
                    return;
                }
            }

            // ── Action dispatcher ──────────────────────────────────────────
            if let Some(action) = action {
                let editor_cmd = |cmd: &str, editor_state: &mut Ref<Option<EditorState>>| -> bool {
                    let mut quit = false;
                    if let Some(s) = editor_state.write().as_mut() {
                        s.cmd_buf = cmd.to_string();
                        quit = s.execute_command();
                        s.needs_rerender = false;
                    }
                    quit
                };
                let mut quit = false;
                match action {
                    LeaderAction::Explorer => {
                        if !show_tree.get() {
                            if tree_root.read().is_none() {
                                let dir = tree_base_dir
                                    .clone()
                                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                                *tree_root.write() = FileNode::from_dir(&dir);
                            }
                            // Start on the open file when it's visible.
                            let rows = tree_root
                                .read()
                                .as_ref()
                                .map(|root| tree_rows(root, ""))
                                .unwrap_or_default();
                            let here = rows.iter().position(|n| n.path == current_file).unwrap_or(0);
                            tree_selected.set(here);
                            show_tree.set(true);
                            tree_focus.set(true);
                        } else if tree_focus.get() {
                            show_tree.set(false);
                            tree_focus.set(false);
                            tree_search.set(String::new());
                            tree_searching.set(false);
                        } else {
                            tree_focus.set(true);
                        }
                    }
                    LeaderAction::FindFile | LeaderAction::NextTheme => {
                        if let Some(handler) = &on_app_action {
                            handler(action);
                        }
                    }
                    LeaderAction::Search => {
                        tree_focus.set(false);
                        if let Some(s) = editor_state.write().as_mut() {
                            s.mode = Mode::Normal;
                            handle_key(s, KeyCode::Char('/'), false);
                        }
                    }
                    LeaderAction::Save => quit = editor_cmd("w", &mut editor_state),
                    LeaderAction::Quit => quit = editor_cmd("q", &mut editor_state),
                    LeaderAction::SaveQuit => quit = editor_cmd("wq", &mut editor_state),
                    LeaderAction::ForceQuit => quit = editor_cmd("q!", &mut editor_state),
                    LeaderAction::ToggleMath => quit = editor_cmd("math", &mut editor_state),
                    LeaderAction::Serve => quit = editor_cmd("serve", &mut editor_state),
                    LeaderAction::GrowGraphics => adjust_graphics(0.25),
                    LeaderAction::ShrinkGraphics => adjust_graphics(-0.25),
                    LeaderAction::Keymaps => show_help.set(true),
                }
                tick.set(tick.get().wrapping_add(1));
                if quit {
                    on_quit(());
                }
                return;
            }

            // ── File tree (when it has focus) ──────────────────────────────
            // The explorer owns the keyboard while focused; only the command
            // line (`:`) and Ctrl shortcuts reach the document.
            if tree_has_focus && !(code == KeyCode::Char(':') && !tree_searching.get()) && !(ctrl && !tree_searching.get()) {
                // ── Search mode: type characters to filter ─────────────────
                if tree_searching.get() {
                    match code {
                        KeyCode::Esc => {
                            tree_searching.set(false);
                            tree_search.set(String::new());
                        }
                        KeyCode::Enter => tree_searching.set(false),
                        KeyCode::Backspace => {
                            let mut s = tree_search.read().clone();
                            s.pop();
                            tree_search.set(s);
                            tree_selected.set(0);
                        }
                        KeyCode::Char(c) => {
                            let mut s = tree_search.read().clone();
                            s.push(c);
                            tree_search.set(s);
                            tree_selected.set(0);
                        }
                        _ => {}
                    }
                    tick.set(tick.get().wrapping_add(1));
                    return;
                }

                // Freshly recompute the visible rows so selection/expansion
                // always operate on the current tree, not a stale snapshot.
                let search_q = tree_search.read().clone();
                let visible: Vec<(PathBuf, bool, bool)> = {
                    // (path, is_dir, expanded)
                    let guard = tree_root.read();
                    match guard.as_ref() {
                        Some(root) => cached_tree_rows(root, &search_q, search_cache.clone())
                            .into_iter()
                            .map(|n| (n.path, n.is_dir, n.expanded))
                            .collect(),
                        None => Vec::new(),
                    }
                };
                let max = visible.len().saturating_sub(1);
                let mut toggle_selected = || {
                    if let Some((target, _, _)) = visible.get(tree_selected.get()) {
                        let target = target.clone();
                        if let Some(root) = tree_root.write().as_mut() {
                            FileNode::toggle(root, &target);
                        }
                    }
                };
                let page = (vh.unwrap_or(24) as usize / 2).max(1);
                let mut open = |path: &PathBuf| {
                    if let Some(handler) = &on_open_file {
                        handler(path.clone());
                    }
                    // Like opening a file from an explorer in an editor: the
                    // document takes focus; the explorer stays open.
                    tree_focus.set(false);
                };
                match code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        tree_selected.set(tree_selected.get().saturating_sub(1).min(max));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        tree_selected.set((tree_selected.get() + 1).min(max));
                    }
                    KeyCode::Char('g') | KeyCode::Home => tree_selected.set(0),
                    KeyCode::Char('G') | KeyCode::End => tree_selected.set(max),
                    KeyCode::PageDown => tree_selected.set((tree_selected.get() + page).min(max)),
                    KeyCode::PageUp => tree_selected.set(tree_selected.get().saturating_sub(page)),
                    // Enter: toggle dirs, open files.
                    KeyCode::Enter => {
                        if let Some(node) = visible.get(tree_selected.get()).cloned() {
                            if node.1 {
                                toggle_selected();
                            } else {
                                open(&node.0);
                            }
                        }
                    }
                    // Right / l: expand a folder, or open a file.
                    KeyCode::Right | KeyCode::Char('l') => {
                        let collapsed_dir = visible
                            .get(tree_selected.get())
                            .map_or(false, |(_, is_dir, expanded)| *is_dir && !*expanded);
                        if collapsed_dir {
                            toggle_selected();
                        } else if let Some((path, false, _)) = visible.get(tree_selected.get()) {
                            open(path);
                        }
                    }
                    // Left / h: collapse expanded dir, or jump to parent.
                    KeyCode::Left | KeyCode::Char('h') => {
                        let expanded_dir = visible
                            .get(tree_selected.get())
                            .map_or(false, |(_, is_dir, expanded)| *is_dir && *expanded);
                        if expanded_dir {
                            toggle_selected();
                        } else if let Some((path, _, _)) = visible.get(tree_selected.get()) {
                            if let Some(parent) = path.parent() {
                                if let Some(i) = visible.iter().position(|n| n.0 == *parent) {
                                    tree_selected.set(i);
                                }
                            }
                        }
                    }
                    KeyCode::Char('/') => {
                        tree_searching.set(true);
                        tree_search.set(String::new());
                        tree_selected.set(0);
                    }
                    // Esc clears the filter, then hands focus back.
                    KeyCode::Esc => {
                        if !tree_search.read().is_empty() {
                            tree_search.set(String::new());
                            tree_selected.set(0);
                        } else {
                            tree_focus.set(false);
                        }
                    }
                    KeyCode::Char('q') => {
                        show_tree.set(false);
                        tree_focus.set(false);
                        tree_search.set(String::new());
                    }
                    _ => {}
                }
                tick.set(tick.get().wrapping_add(1));
                return;
            }

            // Handle editing
            let mut quit = false;
            let mut changed = false;
            let mut rerender = false;
            if let Some(s) = editor_state.write().as_mut() {
                let before = s.buf.to_text();
                s.view_height = vh.unwrap_or(20) as usize;
                s.view_width = vw.unwrap_or(80) as usize;

                if handle_key(s, code, ctrl) {
                    quit = true;
                }
                let after = s.buf.to_text();
                if before != after {
                    changed = true;
                    on_change(after);
                }
                if s.needs_rerender {
                    rerender = true;
                    s.needs_rerender = false;
                }
                if s.help_requested {
                    s.help_requested = false;
                    show_help.set(true);
                    rerender = true;
                }

                // Update global cursor offset
                if let Some(mut off_ref) = cursor_offset.clone() {
                    off_ref.set(s.absolute_byte_offset());
                }
            }

            if quit {
                on_quit(());
                return;
            }

            if changed || rerender {
                tick.set(tick.get().wrapping_add(1));
            }

            // Scroll and Navigation logic (Normal and Visual modes only)
            let current_mode = editor_state
                .read()
                .as_ref()
                .map(|s| s.mode.clone())
                .unwrap_or(Mode::Normal);
            let is_editing_mode = matches!(
                current_mode,
                Mode::Insert | Mode::Command | Mode::Search { .. }
            );
            let (intent, new_pending_g) =
                intent_from_key(code, ctrl, pending_g.get(), is_editing_mode);
            pending_g.set(new_pending_g);

            // Live graphics size adjustment (Normal/Visual modes): resize every
            // image / diagram / formula up or down and see the result at once.
            if !is_editing_mode {
                let delta = match code {
                    KeyCode::Char('[') => Some(-0.25),
                    KeyCode::Char(']') => Some(0.25),
                    _ => None,
                };
                if let Some(delta) = delta {
                    adjust_graphics(delta);
                    tick.set(tick.get().wrapping_add(1));
                }
            }

            let old_scroll = scroll_handle.read().scroll_offset();

            if let ScrollIntent::None = intent { /* no-op */
            } else {
                let viewport_height = scroll_handle.read().viewport_height() as i32;
                let delta = scroll_delta(intent, viewport_height);
                match delta {
                    ScrollDelta::Absolute(offset) => {
                        scroll_handle.write().scroll_to(offset);
                    }
                    ScrollDelta::Relative(d) => {
                        scroll_handle.write().scroll_by(d);
                    }
                    ScrollDelta::ToEnd => {
                        scroll_handle.write().scroll_to_bottom();
                        stick_to_bottom.set(true);
                    }
                    ScrollDelta::None => {}
                }
            }

            let new_scroll = scroll_handle.read().scroll_offset();
            if old_scroll != new_scroll {
                debug::log_event(&debug::DebugEvent::Scroll {
                    ts: debug::elapsed_ms(),
                    old: old_scroll,
                    new: new_scroll,
                });
            }
        }
    });

    // When a different file is opened (file_path changes), reset scroll to
    // the top — otherwise the new document opens at the old scroll offset,
    // which looks like "nothing happened". Keyed on file_path only, so
    // ordinary edit echoes of `content` never trigger it.
    hooks.use_effect(
        {
            let mut scroll_handle = scroll_handle.clone();
            move || {
                scroll_handle.write().scroll_to(0);
            }
        },
        props.file_path.clone(),
    );

    let file_path = props.file_path.clone();
    // When the sidebar is open the content viewport narrows by its width so
    // text wrapping and graphics sizing adapt to the remaining columns.
    let tree_open = show_tree.get();
    let pane_w = vw
        .unwrap_or(100)
        .saturating_sub(if tree_open { TREE_WIDTH } else { 0 });
    // Text column: capped and centered; the scrollbar takes the last column.
    let usable = pane_w.saturating_sub(1);
    let col_w = usable.saturating_sub(2 * MIN_SIDE).min(MAX_COLUMN).max(16);
    let side = usable.saturating_sub(col_w);
    let pad_left = side / 2;
    let pad_right = side - pad_left;
    // Downstream components derive their usable width as
    // `viewport.width - CONTENT_H_INSET`; hand them exactly the column.
    let column_vw = Some(col_w + theme::CONTENT_H_INSET);
    let content_vh = vh.map(|h| h.saturating_sub(1));
    // Current file for highlighting in the tree.
    let current_file_for_tree = if tree_open {
        std::fs::canonicalize(&props.file_path).unwrap_or_else(|_| props.file_path.clone())
    } else {
        props.file_path.clone()
    };
    let tree_search_text = tree_search.read().clone();
    let tree_is_searching = tree_searching.get();

    // ── Status line ──────────────────────────────────────────────────────
    let display_name = display_path(&props.file_path, props.base_dir.as_deref());
    let (row, col) = cursor_row_col.unwrap_or((0, 0));
    let percent = if line_count <= 1 { 100 } else { (row * 100) / (line_count - 1) };
    let position = format!("{}:{}", row + 1, col + 1);
    let message: Option<(Color, String)> = if current_mode == Mode::Command {
        None
    } else if let Some((ok, text)) = props.notice.clone() {
        Some((if ok { theme::green() } else { theme::red() }, text))
    } else if !current_msg.is_empty() {
        let is_err = current_msg.starts_with("E") && current_msg.contains(':');
        Some((if is_err { theme::red() } else { theme::muted() }, current_msg.clone()))
    } else {
        None
    };
    let _ = &cursor_line_preview;
    let pending = props.pending_graphics;
    let explorer_focused = tree_open && tree_focus.get();
    let (mode_label, mode_color) = if explorer_focused && current_mode == Mode::Normal {
        ("EXPLORER", theme::cyan())
    } else {
        (current_mode.label(), current_mode.color())
    };
    let leader_path = leader.read().clone();
    let command_line = match current_mode {
        Mode::Command => Some(format!(":{}", current_cmd)),
        Mode::Search { .. } => Some(current_cmd.clone()),
        _ => None,
    };

    element! {
    View(width: vw.unwrap_or(100), height: vh.unwrap_or(100), flex_direction: FlexDirection::Column, background_color: theme::bg()) {
        View(flex_grow: 1.0, flex_direction: FlexDirection::Row) {
            #(tree_open.then(|| {
                element! {
                    FileTreePanel(
                        visible: visible_tree,
                        selected: tree_selected.get(),
                        width: TREE_WIDTH,
                        current_file: Some(current_file_for_tree.clone()),
                        search_query: tree_search_text.clone(),
                        is_searching: tree_is_searching,
                        focused: explorer_focused,
                        height: content_vh.unwrap_or(24),
                    )
                }
                .into_any()
            }))
            View(flex_grow: 1.0) {
                ScrollView(
                    handle: Some(scroll_handle),
                    keyboard_scroll: Some(false),
                    scrollbar_thumb_color: Some(theme::dark_grey()),
                    scrollbar_track_color: Some(theme::bg()),
                ) {
                    View(width: 100pct, flex_direction: FlexDirection::Column, padding_left: pad_left, padding_right: pad_right, padding_top: 1, padding_bottom: 2) {
                    BlocksRenderer(
                        blocks: doc.blocks,
                        content: current_content,
                        file_path: file_path,
                        viewport: Some(Viewport::new(content_vh, column_vw, Some(scroll_handle.read().scroll_offset()))),
                        cursor_offset: props.cursor_offset.clone(),
                        editor_state: Some(editor_state.clone()),
                        scroll_handle: Some(scroll_handle.clone()),
                        debug: props.debug,
                        debug_annotations: props.debug_annotations,
                    )
                    }
                }
            }
        }
        // Single status line: mode · file · message ……… progress · position · help
        View(width: 100pct, height: 1, background_color: theme::status_bg(), flex_direction: FlexDirection::Row) {
            View(background_color: mode_color, padding_left: 1, padding_right: 1) {
                Text(content: mode_label, color: theme::dark_bg(), weight: Weight::Bold)
            }
            #(if let Some(cmd) = command_line {
                // Command / search entry owns the line, with a visible caret.
                Some(element! {
                    View(flex_grow: 1.0, padding_left: 1, flex_direction: FlexDirection::Row) {
                        Text(content: cmd, color: theme::fg(), wrap: TextWrap::NoWrap)
                        View(background_color: theme::fg(), width: 1) {
                            Text(content: " ")
                        }
                    }
                }.into_any())
            } else {
                Some(element! {
                    View(flex_grow: 1.0, flex_direction: FlexDirection::Row, overflow: Overflow::Hidden) {
                        View(padding_left: 1, flex_shrink: 0.0) {
                            Text(content: display_name.clone(), color: theme::fg(), wrap: TextWrap::NoWrap)
                        }
                        #(is_modified.then(|| element! {
                            View(padding_left: 1, flex_shrink: 0.0) {
                                Text(content: "●", color: theme::orange())
                            }
                        }.into_any()))
                        #(message.clone().map(|(color, text)| element! {
                            View(padding_left: 2, flex_shrink: 1.0, overflow: Overflow::Hidden) {
                                Text(content: text, color: color, wrap: TextWrap::NoWrap)
                            }
                        }.into_any()))
                    }
                }.into_any())
            })
            #((pending > 0).then(|| element! {
                View(padding_right: 2, flex_shrink: 0.0) {
                    Text(content: format!("◌ rendering {pending}"), color: theme::blue())
                }
            }.into_any()))
            View(padding_right: 2, flex_shrink: 0.0) {
                Text(content: format!("{position}  {percent}%"), color: theme::muted(), wrap: TextWrap::NoWrap)
            }
            View(padding_right: 1, flex_shrink: 0.0, flex_direction: FlexDirection::Row) {
                Text(content: "Space", color: theme::fg(), weight: Weight::Bold)
                Text(content: " menu", color: theme::comment())
            }
        }
        // Which-key style menu, docked just above the status line.
        #(leader_path.map(|path| {
            let w = vw.unwrap_or(100);
            let h = leader::menu_height(&path, w);
            element! {
                View(position: Position::Absolute, left: 0, top: vh.unwrap_or(30).saturating_sub(1 + h) as i32, width: w, height: h) {
                    LeaderMenu(path: path, width: w)
                }
            }.into_any()
        }))
        #(show_help.get().then(|| element! {
            HelpOverlay(width: vw.unwrap_or(100), height: vh.unwrap_or(30))
        }.into_any()))
    }
    }
}

/// Grows (`delta > 0`) or shrinks every image, diagram and formula.
fn adjust_graphics(delta: f32) {
    let next = crate::output::capabilities::graphics_scale() + delta;
    if crate::output::capabilities::set_graphics_scale(next) {
        crate::output::graphics_manager::refresh_graphics();
        debug::log_event(&debug::DebugEvent::GraphicsScale {
            ts: debug::elapsed_ms(),
            scale: crate::output::capabilities::graphics_scale(),
        });
    }
}

/// [`tree_rows`] with the workspace-search result memoized per query.
fn cached_tree_rows(
    root: &FileNode,
    query: &str,
    mut cache: Ref<(String, Vec<FileNode>)>,
) -> Vec<FileNode> {
    if query.is_empty() {
        return tree_rows(root, "");
    }
    if cache.read().0 == query {
        return cache.read().1.clone();
    }
    let rows = tree_rows(root, query);
    cache.set((query.to_string(), rows.clone()));
    rows
}

/// The file name shown in the status line: relative to the workspace (or the
/// current directory), falling back to a `~`-shortened absolute path.
fn display_path(file: &std::path::Path, base: Option<&std::path::Path>) -> String {
    let full = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let candidates = base
        .filter(|b| !b.as_os_str().is_empty())
        .map(|b| b.to_path_buf())
        .into_iter()
        .chain(std::env::current_dir().ok());
    for dir in candidates {
        let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        if let Ok(rel) = full.strip_prefix(&dir) {
            if !rel.as_os_str().is_empty() {
                return rel.display().to_string();
            }
        }
    }
    let text = full.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && text.starts_with(&home) => format!("~{}", &text[home.len()..]),
        _ => text,
    }
}
