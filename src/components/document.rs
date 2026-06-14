use std::path::PathBuf;

use iocraft::prelude::*;

use crate::components::blocks_renderer::BlocksRenderer;
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
    pub scale: Option<f32>,
}

#[component]
pub fn Document(props: &DocumentProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let content = props.content.clone();

    // Use parse cache to memoize markdown parsing
    let cache = hooks.use_ref(|| ParseCache::new());
    let doc = if let Some(cached_doc) = cache.read().get(&content) {
        cached_doc
    } else {
        let parsed = parse_markdown(&content);
        cache.read().insert(&content, parsed.clone());
        parsed
    };

    let vh = props.viewport_height;
    let vw = props.viewport_width;
    let scale = props.scale;
    let keyboard_navigation = props.keyboard_navigation.unwrap_or(true);
    let scroll_handle = hooks.use_ref_default::<ScrollViewHandle>();
    let mut pending_g = hooks.use_state(|| false);
    let follow_ref = props.follow_ref;

    hooks.use_terminal_events({
        let mut scroll_handle = scroll_handle;
        let content = content.clone();
        let cursor_offset = props.cursor_offset.clone();
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

            if !keyboard_navigation || kind == KeyEventKind::Release {
                if let Some(follow_ref) = follow_ref {
                    let current_line = follow_ref.get();
                    let total_lines = content.lines().count().max(1);
                    let viewport_height = vh.unwrap_or(0) as i32;
                    let ch = scroll_handle.read().content_height() as i32;

                    if current_line + 1 >= total_lines {
                        scroll_handle.write().scroll_to_bottom();
                    } else if current_line == 0 {
                        scroll_handle.write().scroll_to_top();
                    } else if ch > 0 {
                        let proportion = current_line as f32 / total_lines as f32;
                        let offset = (proportion * ch as f32) as i32 - (viewport_height / 3).max(0);
                        scroll_handle.write().scroll_to(offset.max(0));
                    } else {
                        let offset = current_line as i32 - (viewport_height / 3).max(0);
                        scroll_handle.write().scroll_to(offset.max(0));
                    }
                }
                return;
            }

            let ctrl = modifiers.contains(KeyModifiers::CONTROL);
            let viewport_height = scroll_handle.read().viewport_height() as i32;
            let page = viewport_height.max(1);
            let half_page = (page / 2).max(1);

            let mut update_scroll = |current_off: usize| {
                let line_num = content[..current_off].split('\n').count() - 1;
                let total_lines = content.lines().count().max(1);
                let vh = vh.unwrap_or(0) as i32;
                let ch = scroll_handle.read().content_height() as i32;

                if line_num == 0 {
                    scroll_handle.write().scroll_to_top();
                } else if line_num + 1 >= total_lines {
                    scroll_handle.write().scroll_to_bottom();
                } else if ch > 0 {
                    let proportion = line_num as f32 / total_lines as f32;
                    let offset = (proportion * ch as f32) as i32 - (vh / 2);
                    scroll_handle.write().scroll_to(offset.max(0));
                } else {
                    let offset = line_num as i32 - (vh / 2);
                    scroll_handle.write().scroll_to(offset.max(0));
                }
            };

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
                KeyCode::Char('h') if !ctrl => {
                    if let Some(mut cursor_offset) = cursor_offset.clone() {
                        let current_off = cursor_offset.get();
                        let next_off = content.char_indices()
                            .filter(|&(i, _)| i < current_off)
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        cursor_offset.set(next_off);
                        update_scroll(next_off);
                    }
                    pending_g.set(false);
                }
                KeyCode::Char('l') if !ctrl => {
                    if let Some(mut cursor_offset) = cursor_offset.clone() {
                        let current_off = cursor_offset.get();
                        let next_off = content.char_indices()
                            .find(|&(i, _)| i > current_off)
                            .map(|(i, _)| i)
                            .unwrap_or(content.len());
                        cursor_offset.set(next_off);
                        update_scroll(next_off);
                    }
                    pending_g.set(false);
                }
                KeyCode::Char('j') if !ctrl => {
                    // Move cursor forward by one line
                    if let Some(mut cursor_offset) = cursor_offset.clone() {
                        let current_off = cursor_offset.get();
                        let content_bytes = content.as_bytes();
                        let mut next_off = current_off;
                        
                        // Find the start of the next line
                        while next_off < content_bytes.len() && content_bytes[next_off] != b'\n' {
                            next_off += 1;
                        }
                        if next_off < content_bytes.len() {
                            next_off += 1; // skip \n
                        }
                        cursor_offset.set(next_off.min(content_bytes.len()));
                        update_scroll(next_off.min(content_bytes.len()));
                    }
                    pending_g.set(false);
                }
                KeyCode::Down => {
                    // Same as 'j'
                    if let Some(mut cursor_offset) = cursor_offset.clone() {
                        let current_off = cursor_offset.get();
                        let content_bytes = content.as_bytes();
                        let mut next_off = current_off;
                        
                        while next_off < content_bytes.len() && content_bytes[next_off] != b'\n' {
                            next_off += 1;
                        }
                        if next_off < content_bytes.len() {
                            next_off += 1; // skip \n
                        }
                        cursor_offset.set(next_off.min(content_bytes.len()));
                        update_scroll(next_off.min(content_bytes.len()));
                    }
                    pending_g.set(false);
                }
                KeyCode::Char('k') if !ctrl => {
                    // Move cursor backward by one line
                    if let Some(mut cursor_offset) = cursor_offset.clone() {
                        let current_off = cursor_offset.get();
                        let content_bytes = content.as_bytes();
                        
                        if current_off == 0 {
                            cursor_offset.set(0);
                            update_scroll(0);
                        } else {
                            let mut prev_off = current_off - 1;
                            // If we are at the start of a line, we need to go back further to find the previous \n
                            if prev_off > 0 && content_bytes[prev_off] == b'\n' {
                                prev_off -= 1;
                            }
                            while prev_off > 0 && content_bytes[prev_off] != b'\n' {
                                prev_off -= 1;
                            }
                            if prev_off > 0 && content_bytes[prev_off] == b'\n' {
                                prev_off += 1; // start of the line
                            } else if prev_off == 0 {
                                prev_off = 0;
                            }
                            cursor_offset.set(prev_off);
                            update_scroll(prev_off);
                        }
                    }
                    pending_g.set(false);
                }
                KeyCode::Up => {
                    // Same as 'k'
                    if let Some(mut cursor_offset) = cursor_offset.clone() {
                        let current_off = cursor_offset.get();
                        let content_bytes = content.as_bytes();
                        
                        if current_off == 0 {
                            cursor_offset.set(0);
                            update_scroll(0);
                        } else {
                            let mut prev_off = current_off - 1;
                            // If we are at the start of a line, we need to go back further to find the previous \n
                            if prev_off > 0 && content_bytes[prev_off] == b'\n' {
                                prev_off -= 1;
                            }
                            while prev_off > 0 && content_bytes[prev_off] != b'\n' {
                                prev_off -= 1;
                            }
                            if prev_off > 0 && content_bytes[prev_off] == b'\n' {
                                prev_off += 1; // start of the line
                            } else if prev_off == 0 {
                                prev_off = 0;
                            }
                            cursor_offset.set(prev_off);
                            update_scroll(prev_off);
                        }
                    }
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
                KeyCode::PageDown | KeyCode::Char(' ') => {
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
                            content: content.clone(),
                            file_path: file_path,
                            viewport_height: vh,
                            viewport_width: vw,
                            cursor_offset: props.cursor_offset.clone(),
                            scale
                        )
                    }
                }
            }
        }
    }
}
