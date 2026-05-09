use std::path::PathBuf;

use iocraft::prelude::*;

use crate::components::blocks_renderer::BlocksRenderer;
use crate::document::parser::parse_markdown;

#[derive(Default, Props)]
pub struct DocumentProps {
    pub content: String,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub keyboard_navigation: Option<bool>,
    pub follow_ref: Option<Ref<usize>>,
}

#[component]
pub fn Document(props: &DocumentProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let content = props.content.clone();
    let doc = parse_markdown(&content);

    let vh = props.viewport_height;
    let vw = props.viewport_width;
    let keyboard_navigation = props.keyboard_navigation.unwrap_or(true);
    let scroll_handle = hooks.use_ref_default::<ScrollViewHandle>();
    let mut pending_g = hooks.use_state(|| false);
    let follow_ref = props.follow_ref;

    hooks.use_terminal_events({
        let mut scroll_handle = scroll_handle;
        let content = content.clone();
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
                KeyCode::Char('j') if !ctrl => {
                    scroll_handle.write().scroll_by(1);
                    pending_g.set(false);
                }
                KeyCode::Down => {
                    scroll_handle.write().scroll_by(1);
                    pending_g.set(false);
                }
                KeyCode::Char('k') if !ctrl => {
                    scroll_handle.write().scroll_by(-1);
                    pending_g.set(false);
                }
                KeyCode::Up => {
                    scroll_handle.write().scroll_by(-1);
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
    View(width: vw.unwrap_or(100), height: vh.unwrap_or(100), flex_direction: FlexDirection::Column, background_color: Color::AnsiValue(234)) {
        View(flex_grow: 1.0, border_style: BorderStyle::Single, border_color: Color::AnsiValue(238)){
                ScrollView(
                    handle: Some(scroll_handle),
                    keyboard_scroll: Some(false),
                    scrollbar_thumb_color: Some(Color::AnsiValue(250)),
                    scrollbar_track_color: Some(Color::AnsiValue(238)),
                ) {
                    View(flex_direction:FlexDirection::Column, padding: 1){
                        BlocksRenderer(
                            blocks: doc.blocks,
                            file_path: file_path,
                            viewport_height: vh,
                            viewport_width: vw
                        )
                    }
                }
            }
        }
    }
}
