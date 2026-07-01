use std::path::PathBuf;

use iocraft::prelude::*;

use crate::components::blocks_renderer::BlocksRenderer;
use crate::components::editor::{Buffer, EditorState, Mode, handle_key};
use crate::document::cache::ParseCache;
use crate::document::parser::parse_markdown;

#[derive(Default, Props)]
pub struct DocumentProps {
    pub content: String,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub keyboard_navigation: Option<bool>,
    pub follow_ref: Option<Ref<usize>>,
    pub cursor_offset: Option<Ref<usize>>,
    pub on_change: Handler<String>,
    pub on_quit: Handler<()>,
}

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
    drop(state_guard);

    // To trigger re-renders on edit
    let tick = hooks.use_state(|| 0u64);

    // Keep editor state in sync with content prop if it changes externally
    hooks.use_effect(
        {
            let mut editor_state = editor_state.clone();
            let content = content_prop.clone();
            move || {
                if let Some(s) = editor_state.write().as_mut() {
                    if s.buf.to_text() != content {
                        s.buf = Buffer::new(&content);
                    }
                }
            }
        },
        content_prop.clone(),
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
    let prev_cursor_row = hooks.use_ref(|| 0usize);
    let _follow_ref = props.follow_ref;
    let on_change = props.on_change.clone();
    let on_quit = props.on_quit.clone();

    hooks.use_terminal_events({
        let mut scroll_handle = scroll_handle;
        let content = current_content.clone();
        let cursor_offset = props.cursor_offset.clone();
        let mut editor_state = editor_state.clone();
        let mut tick = tick.clone();
        let on_change = on_change.clone();
        let on_quit = on_quit.clone();
        let mut prev_cursor_row = prev_cursor_row.clone();
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

            // Handle editing
            let mut quit = false;
            let mut changed = false;
            let mut cursor_row = None;
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

                // Update global cursor offset
                if let Some(mut off_ref) = cursor_offset.clone() {
                    off_ref.set(s.absolute_byte_offset());
                }
                cursor_row = Some(s.row);
            }

            if quit {
                on_quit(());
                return;
            }

            if changed {
                tick.set(tick.get().wrapping_add(1));
            }

            // Scroll and Navigation logic
            let viewport_height = scroll_handle.read().viewport_height() as i32;
            let page = viewport_height.max(1);
            let half_page = (page / 2).max(1);

            // Incremental scroll: instead of proportional centering (which breaks
            // because rendered content height doesn't map linearly to line numbers),
            // just scroll by small increments based on cursor direction.
            let mut update_scroll = |current_row: usize| {
                let state_guard = editor_state.read();
                let total_logical_lines = state_guard
                    .as_ref()
                    .map(|s| s.buf.line_count())
                    .unwrap_or(1);
                drop(state_guard);

                let vh = vh.unwrap_or(0) as i32;
                let ch = scroll_handle.read().content_height() as i32;

                if ch <= vh {
                    return;
                }

                // Boundary handling: snap to top/bottom at document edges
                if current_row == 0 {
                    scroll_handle.write().scroll_to(0);
                    return;
                }
                if current_row >= total_logical_lines.saturating_sub(1) {
                    scroll_handle.write().scroll_to(ch - vh);
                    return;
                }
            };

            if let Some(row) = cursor_row {
                update_scroll(row);
                prev_cursor_row.set(row);
            }

            match code {
                KeyCode::Char('g') if !ctrl && pending_g.get() => {
                    scroll_handle.write().scroll_to_top();
                    pending_g.set(false);
                }
                KeyCode::Char('g') if !ctrl => {
                    pending_g.set(true);
                }
                KeyCode::Char('G') if !ctrl => {
                    scroll_handle.write().scroll_to_bottom();
                    pending_g.set(false);
                }
                KeyCode::End => {
                    scroll_handle.write().scroll_to_bottom();
                    pending_g.set(false);
                }
                KeyCode::Char('d') if ctrl => {
                    scroll_handle.write().scroll_by(half_page);
                    pending_g.set(false);
                }
                KeyCode::Char('u') if ctrl => {
                    scroll_handle.write().scroll_by(-half_page);
                    pending_g.set(false);
                }
                KeyCode::Char('f') if ctrl => {
                    scroll_handle.write().scroll_by(page);
                    pending_g.set(false);
                }
                KeyCode::PageDown => {
                    scroll_handle.write().scroll_by(page);
                    pending_g.set(false);
                }
                KeyCode::Char('b') if ctrl => {
                    scroll_handle.write().scroll_by(-page);
                    pending_g.set(false);
                }
                KeyCode::PageUp => {
                    scroll_handle.write().scroll_by(-page);
                    pending_g.set(false);
                }
                KeyCode::Home => {
                    scroll_handle.write().scroll_to_top();
                    pending_g.set(false);
                }
                _ => {
                    pending_g.set(false);
                }
            }
        }
    });

    let file_path = props.file_path.clone();

    element! {
    View(width: vw.unwrap_or(100), height: vh.unwrap_or(100), flex_direction: FlexDirection::Column, background_color: crate::theme::BG) {
        View(flex_grow: 1.0, border_style: BorderStyle::Single, border_color: crate::theme::BORDER){
                ScrollView(
                    handle: Some(scroll_handle),
                    keyboard_scroll: Some(false),
                    scrollbar_thumb_color: Some(crate::theme::FG),
                    scrollbar_track_color: Some(crate::theme::DARK_BG),
                ) {
                    View(flex_direction: FlexDirection::Column, padding_left: 2, padding_right: 2, padding_top: 1, padding_bottom: 1) {
                        BlocksRenderer(
                            blocks: doc.blocks,
                            content: current_content,
                            file_path: file_path,
                            viewport_height: vh,
                            viewport_width: vw,
                            cursor_offset: props.cursor_offset.clone(),
                            editor_state: Some(editor_state.clone()),
                            scroll_handle: Some(scroll_handle.clone()),
                        )
                    }
                }
            }
            View(width: 100pct, height: 1, background_color: crate::theme::STATUS_BG, flex_direction: FlexDirection::Row) {
                View(background_color: current_mode.color(), padding_left: 1, padding_right: 1) {
                    Text(content: format!(" {} ", current_mode.label()), color: crate::theme::DARK_BG, weight: Weight::Bold)
                }
                View(flex_grow: 1.0, padding_left: 1) {
                    #(if current_mode == Mode::Command {
                        Some(element! {
                            Text(content: format!(":{}", current_cmd), color: crate::theme::FG)
                        })
                    } else if let Mode::Search { .. } = current_mode {
                        Some(element! {
                            Text(content: current_cmd.clone(), color: crate::theme::FG)
                        })
                    } else {
                        Some(element! {
                            Text(content: current_msg.clone(), color: crate::theme::FG)
                        })
                    }.into_iter())
                }
            }
        }
    }
}
