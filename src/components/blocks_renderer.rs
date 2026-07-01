use crate::components::code_block::CodeBlock;
use crate::components::editor::{EditorState, Mode};
use crate::components::heading::Heading;
use crate::components::html_block::HtmlBlock;
use crate::components::image::Image;
use crate::components::list_block::ListBlock;
use crate::components::math_block::MathBlock;
use crate::components::mermaid_block::MermaidBlock;
use crate::components::paragraph::Paragraph;
use crate::components::quote_block::QuoteBlock;
use crate::components::table_block::TableBlock;
use crate::components::thematic_break::ThematicBreak;
use crate::document::model::Block;
use crate::theme;
use iocraft::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// "… rest of the truncated text"
fn tail_to_width(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut width = 0usize;
    for ch in s.chars().rev() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + w > max {
            out.insert(0, '…');
            break;
        }
        out.insert(0, ch);
        width += w;
    }
    out
}

// "first part of the truncated text …"
fn head_to_width(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut width = 0usize;
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + w > max {
            out.push('…');
            break;
        }
        out.push(ch);
        width += w;
    }
    out
}

#[derive(Default, Props)]
struct ScrollIntoViewContainerProps {
    pub scroll_handle: Option<Ref<ScrollViewHandle>>,
    pub viewport_height: Option<u32>,
    pub cursor_moved: bool,
    pub child: Option<Arc<dyn Fn() -> AnyElement<'static> + Send + Sync + 'static>>,
}

#[component]
fn ScrollIntoViewContainer(
    props: &ScrollIntoViewContainerProps,
    mut hooks: Hooks,
) -> impl Into<AnyElement<'static>> {
    let rect = hooks.use_component_rect();
    let mut needs_scroll = hooks.use_state(|| false);

    if props.cursor_moved {
        needs_scroll.set(true);
    }

    hooks.use_effect(
        {
            let mut needs_scroll = needs_scroll.clone();
            let rect = rect.clone();
            let scroll_handle = props.scroll_handle.clone();
            move || {
                if needs_scroll.get() {
                    if let Some(r) = rect {
                        if let Some(scroll_ref) = &scroll_handle {
                            let mut scroll_ref = scroll_ref.clone();
                            let block_top = r.top;
                            let block_bottom = r.bottom;

                            let viewport_h = scroll_ref.read().viewport_height() as i32;
                            let viewport_top = 1; // offset for top border
                            let viewport_bottom = viewport_top + viewport_h;

                            if block_top < viewport_top {
                                let diff = viewport_top - block_top;
                                scroll_ref.write().scroll_by(-diff);
                            } else if block_bottom > viewport_bottom {
                                let diff = block_bottom - viewport_bottom;
                                if (r.bottom - r.top) <= viewport_h {
                                    scroll_ref.write().scroll_by(diff);
                                } else {
                                    let top_diff = block_top - viewport_top;
                                    scroll_ref.write().scroll_by(top_diff);
                                }
                            }
                            needs_scroll.set(false);
                        }
                    }
                }
            }
        },
        (needs_scroll.get(), rect.is_some()),
    );

    element! {
        View() {
            #(props.child.as_ref().map(|f| f()).into_iter())
        }
    }
}

#[derive(Default, Props)]
pub struct BlocksRendererProps {
    pub blocks: Vec<Block>,
    pub content: String,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub cursor_offset: Option<Ref<usize>>,
    pub editor_state: Option<Ref<Option<EditorState>>>,
    pub scroll_handle: Option<Ref<ScrollViewHandle>>,
}

#[component]
pub fn BlocksRenderer(
    props: &BlocksRendererProps,
    mut hooks: Hooks,
) -> impl Into<AnyElement<'static>> {
    let cursor_offset_val = props.cursor_offset.as_ref().map(|r| r.get());
    let last_offset = hooks.use_state(|| cursor_offset_val.unwrap_or(0));
    let cursor_moved = cursor_offset_val.map_or(false, |off| off != last_offset.get());

    hooks.use_effect(
        {
            let mut last_offset = last_offset.clone();
            move || {
                if let Some(off) = cursor_offset_val {
                    last_offset.set(off);
                }
            }
        },
        cursor_offset_val,
    );
    let file_path = props.file_path.clone();
    let vh = props.viewport_height;
    let vw = props.viewport_width;
    let cursor_offset = props.cursor_offset.as_ref().map(|r| r.get());

    let (vis_start, vis_end, mode, is_editing_mode, cursor_row_col) =
        if let Some(state_ref) = &props.editor_state {
            let s_opt = state_ref.read();
            if let Some(s) = s_opt.as_ref() {
                let start = s.absolute_byte_offset_at(s.visual_start.0, s.visual_start.1);
                let end = s.absolute_byte_offset();
                let editing = matches!(s.mode, Mode::Insert | Mode::Command | Mode::Search { .. });
                (
                    Some(start.min(end)),
                    Some(start.max(end)),
                    s.mode.clone(),
                    editing,
                    Some((s.row, s.col)),
                )
            } else {
                (None, None, Mode::Normal, false, None)
            }
        } else {
            (None, None, Mode::Normal, false, None)
        };

    let block_counts = props.blocks.len();
    element! {
        View(flex_direction: FlexDirection::Column) {
            #(props.blocks.iter().enumerate().map(|(i, block)| {
                let span = block.span();

                // is_cursor_here: cursor is on this block (any mode)
                let is_last_block = i+1 == block_counts;
                let is_cursor_here = cursor_offset.map_or(false, |off| { if is_last_block {off >= span.0 && off <= span.1} else {off >= span.0 && off < span.1}});
                // Only show raw text editing view when cursor is on the block AND
                // the editor is in an editing mode (Insert/Command/Search).
                // In Normal mode, blocks stay as their rendered markdown form (view-only).
                let is_active = is_editing_mode && is_cursor_here;
                let is_selected = mode == Mode::Visual && vis_start.map_or(false, |start| {
                    vis_end.map_or(false, |end| {
                        span.0 <= end && span.1 >= start
                    })
                });

                if is_active || is_selected {
                    let off = cursor_offset.unwrap_or(0);
                    let text = &props.content[span.0..span.1];
                    let rel_off = off.saturating_sub(span.0).min(text.len());

                    let lines: Vec<&str> = text.split('\n').collect();
                    let mut current_byte_acc = 0;
                    let mut cursor_line_idx = None;
                    let mut cursor_rel_off = 0;

                    for (idx, line) in lines.iter().enumerate() {
                        let line_len = line.len();
                        if rel_off >= current_byte_acc && rel_off <= current_byte_acc + line_len {
                            cursor_line_idx = Some(idx);
                            cursor_rel_off = rel_off - current_byte_acc;
                        }
                        current_byte_acc += line_len + 1;
                    }

                    let cursor_bg = match mode {
                        Mode::Normal => theme::FG,
                        Mode::Insert => theme::GREEN,
                        Mode::Visual => theme::MAGENTA,
                        Mode::Command | Mode::Search { .. } => theme::YELLOW,
                    };

                    let (cursor_fg, cursor_bg_final, cursor_char) = if let Some(state_ref) = &props.editor_state {
                        let s_opt = state_ref.read();
                        if let Some(s) = s_opt.as_ref() {
                            if s.mode == Mode::Insert {
                                (cursor_bg, theme::DARK_BG, "┃")
                            } else if s.operator.is_some() {
                                (cursor_bg, theme::DARK_BG, "_")
                            } else {
                                (theme::DARK_BG, cursor_bg, " ")
                            }
                        } else {
                            (theme::DARK_BG, cursor_bg, " ")
                        }
                    } else {
                        (theme::DARK_BG, cursor_bg, " ")
                    };

                    element! {
                        View(
                            background_color: theme::DARK_BG,
                            padding_left: 2,
                            padding_right: 2,
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::Hidden,
                        ) {
                            #(lines.iter().enumerate().map(|(idx, line)| {
                                let line_start_off = span.0 + lines[..idx].iter().map(|l| l.len() + 1).sum::<usize>();
                                let wrap_width = (vw.unwrap_or(80) as i32 - theme::TOTAL_VIEWPORT_OFFSET as i32).max(1) as usize;
                                let mut segments = Vec::new();
                                let mut remaining: &str = line;
                                while !remaining.is_empty() {
                                    // Implement word-aware wrapping to match iocraft's TextWrap::Wrap
                                    let mut split_at = remaining.char_indices().nth(wrap_width).map(|(i, _)| i).unwrap_or(remaining.len());

                                    if split_at < remaining.len() {
                                        // Try to find the last whitespace before the wrap point
                                        if let Some(last_space) = remaining[..split_at].rfind(' ') {
                                            // Only wrap at space if the word being split is not the only thing on the line
                                            if last_space > 0 {
                                                split_at = last_space + 1;
                                            }
                                        }
                                    }
                                    segments.push(&remaining[..split_at]);
                                    remaining = &remaining[split_at..];
                                }

                                element! {
                                    View(flex_direction: FlexDirection::Column) {
                                        #(segments.iter().enumerate().map(|(seg_idx, segment)| {
                                            if mode == Mode::Visual {
                                                if let (Some(start), Some(end)) = (vis_start, vis_end) {
                                                    let seg_start_off = line_start_off + segments[..seg_idx].iter().map(|s| s.len()).sum::<usize>();
                                                    let mut line_parts: Vec<(bool, String)> = Vec::new();
                                                    let mut current_pos = seg_start_off;
                                                    let seg_chars: Vec<char> = segment.chars().collect();
                                                    for c in seg_chars {
                                                        let char_len = c.len_utf8();
                                                        let is_selected = current_pos >= start && current_pos < end;
                                                        if let Some(last) = line_parts.last_mut() {
                                                            if last.0 == is_selected {
                                                                last.1.push(c);
                                                                current_pos += char_len;
                                                                continue;
                                                            }
                                                        }
                                                        line_parts.push((is_selected, c.to_string()));
                                                        current_pos += char_len;
                                                    }
                                                    element! {
                                                        View(flex_direction: FlexDirection::Row) {
                                                            #(line_parts.iter().map(|(selected, text)| element! {
                                                                Text(content: text.clone(), color: if *selected { theme::MAGENTA } else { theme::FG }, wrap: TextWrap::Wrap)
                                                            }))
                                                        }
                                                    }.into_any()
                                                } else {
                                                    element! { Text(content: segment.to_string(), color: theme::FG, wrap: TextWrap::Wrap) }.into_any()
                                                }
                                            } else if Some(idx) == cursor_line_idx {
                                                let mut seg_idx_cursor = 0;
                                                let mut seg_rel_off = cursor_rel_off;
                                                for seg in &segments {
                                                    if seg_rel_off < seg.len() { break; }
                                                    seg_rel_off -= seg.len();
                                                    seg_idx_cursor += 1;
                                                }
                                                if seg_idx == seg_idx_cursor {
                                                    let (before, after_with_char) = segment.split_at(seg_rel_off.min(segment.len()));

                                                    let before_str = before.to_string();
                                                    let cursor_char_str = cursor_char.to_string();
                                                    let cursor_bg_final_clone = cursor_bg_final.clone();
                                                    let cursor_fg_clone = cursor_fg.clone();
                                                    let editor_state_clone = props.editor_state.clone();

                                                    let factory = if let Some(c) = after_with_char.chars().next() {
                                                        let char_len = c.len_utf8();
                                                        let after_str = after_with_char[char_len..].to_string();
                                                        let c_str = c.to_string();

                                                        Arc::new(move || {
                                                            if let Some(state_ref) = &editor_state_clone {
                                                                let s_opt = state_ref.read();
                                                                if let Some(s) = s_opt.as_ref() {
                                                                    if s.mode == Mode::Insert {
                                                                        element! {
                                                                            View(flex_direction: FlexDirection::Row) {
                                                                                Text(content: before_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                                View(background_color: cursor_bg_final_clone, width: 1) {
                                                                                    Text(content: cursor_char_str.clone(), color: cursor_fg_clone, wrap: TextWrap::Wrap)
                                                                                }
                                                                                Text(content: format!("{}{}", c_str, after_str), color: theme::FG, wrap: TextWrap::Wrap)
                                                                            }
                                                                        }.into_any()
                                                                    } else if s.operator.is_some() {
                                                                        element! {
                                                                            View(flex_direction: FlexDirection::Row) {
                                                                                Text(content: before_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                                View(background_color: cursor_bg_final_clone, width: 1) {
                                                                                    Text(content: c_str.clone(), color: cursor_fg_clone, wrap: TextWrap::Wrap)
                                                                                }
                                                                                Text(content: after_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                            }
                                                                        }.into_any()
                                                                    } else {
                                                                        element! {
                                                                            View(flex_direction: FlexDirection::Row) {
                                                                                Text(content: before_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                                View(background_color: cursor_bg_final_clone, width: 1) {
                                                                                    Text(content: c_str.clone(), color: cursor_fg_clone, wrap: TextWrap::Wrap)
                                                                                }
                                                                                Text(content: after_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                            }
                                                                        }.into_any()
                                                                    }
                                                                } else {
                                                                    element! {
                                                                        View(flex_direction: FlexDirection::Row) {
                                                                            Text(content: before_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                            View(background_color: cursor_bg_final_clone, width: 1) {
                                                                                Text(content: c_str.clone(), color: cursor_fg_clone, wrap: TextWrap::Wrap)
                                                                            }
                                                                            Text(content: after_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                        }
                                                                    }.into_any()
                                                                }
                                                            } else {
                                                                element! {
                                                                    View(flex_direction: FlexDirection::Row) {
                                                                        Text(content: before_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                        View(background_color: cursor_bg_final_clone, width: 1) {
                                                                            Text(content: c_str.clone(), color: cursor_fg_clone, wrap: TextWrap::Wrap)
                                                                        }
                                                                        Text(content: after_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                    }
                                                                }.into_any()
                                                            }
                                                        }) as Arc<dyn Fn() -> AnyElement<'static> + Send + Sync + 'static>
                                                    } else {
                                                        Arc::new(move || {
                                                            element! {
                                                                View(flex_direction: FlexDirection::Row) {
                                                                    Text(content: before_str.clone(), color: theme::FG, wrap: TextWrap::Wrap)
                                                                    View(background_color: cursor_bg_final_clone, width: 1) {
                                                                        Text(content: cursor_char_str.clone(), color: cursor_fg_clone, wrap: TextWrap::Wrap)
                                                                    }
                                                                    Text(content: "", color: theme::FG, wrap: TextWrap::Wrap)
                                                                }
                                                            }.into_any()
                                                        }) as Arc<dyn Fn() -> AnyElement<'static> + Send + Sync + 'static>
                                                    };

                                                    element! {
                                                        ScrollIntoViewContainer(
                                                            scroll_handle: props.scroll_handle.clone(),
                                                            viewport_height: props.viewport_height,
                                                            cursor_moved,
                                                            child: Some(factory),
                                                        )
                                                    }.into_any()
                                                } else {
                                                    element! { Text(content: segment.to_string(), color: theme::FG, wrap: TextWrap::Wrap) }.into_any()
                                                }
                                            } else {
                                                element! { Text(content: segment.to_string(), color: theme::FG, wrap: TextWrap::Wrap) }.into_any()
                                            }
                                        }))
                                    }
                                }.into_any()
                            }))
                        }
                    }.into_any()
                } else {
                    // Render block as formatted markdown.
                    // If cursor is on this block (Normal mode), wrap with a left-border
                    // accent so the user can see where the cursor is before pressing `i`.
                    let rendered = match block {
                        Block::Heading { level, content, id: _, .. } => element!{Heading(level: *level, content: content.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Paragraph { content, .. } => element!{Paragraph(content: content.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Code { language, code, .. } => element!{CodeBlock(language: language.clone(), code: code.clone())}.into_any(),
                        Block::Mermaid { source, .. } => element!{MermaidBlock(source: source.clone(), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Math { content, display, .. } => element!{MathBlock(content: content.clone(), display: *display, viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Quote { children, .. } => element!{QuoteBlock(children: children.clone(), file_path: Some(file_path.clone()), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::List { ordered, start, items, .. } => element!{ListBlock(ordered: *ordered, start: *start, items: items.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Table { headers, alignments, rows, .. } => element!{TableBlock(headers: headers.clone(), alignments: alignments.clone(), rows: rows.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::ThematicBreak{..} => element!{ThematicBreak()}.into_any(),
                        Block::Image { alt, url, title, .. } => element!{Image(url: url.clone(), file_path: file_path.clone(), title: title.clone(), alt: Some(alt.clone()), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Html { content, .. } => element!{HtmlBlock(content: content.clone())}.into_any(),
                    };

                    if is_cursor_here && !is_editing_mode {
                        // Show a left-border accent indicator on the active block
                        // so the user knows where the cursor is in Normal mode.
                        let off = cursor_offset.unwrap_or(0);
                        let text = &props.content[span.0..span.1];
                        let rel_off = off.saturating_sub(span.0).min(text.len());

                        let lines: Vec<&str> = text.split('\n').collect();
                        let mut current_byte_acc = 0;
                        let mut cursor_line_idx = None;
                        let mut cursor_rel_off = 0;

                        for (idx, line) in lines.iter().enumerate() {
                            let line_len = line.len();
                            if rel_off >= current_byte_acc && rel_off <= current_byte_acc + line_len {
                                cursor_line_idx = Some(idx);
                                cursor_rel_off = rel_off - current_byte_acc;
                            }
                            current_byte_acc += line_len + 1;
                        }

                        let mut cursor_line_text = "";
                        let mut cursor_char_idx = 0;
                        if let Some(idx) = cursor_line_idx {
                            if idx < lines.len() {
                                cursor_line_text = lines[idx];
                                cursor_char_idx = cursor_rel_off;
                            }
                        }

                        let before = &cursor_line_text[..cursor_char_idx.min(cursor_line_text.len())];
                        let char_at_cursor = cursor_line_text.char_indices()
                            .find(|&(idx, _)| idx == cursor_char_idx)
                            .map(|(_, c)| c);
                        let cursor_char = char_at_cursor.map(|c| c.to_string()).unwrap_or_else(|| " ".to_string());
                        let after = if let Some(c) = char_at_cursor {
                            let char_len = c.len_utf8();
                            &cursor_line_text[(cursor_char_idx + char_len).min(cursor_line_text.len())..]
                        } else {
                            ""
                        };

                        let block_clone = block.clone();
                        let file_path_clone = file_path.clone();
                        let vh_clone = vh;
                        let vw_clone = vw;
                        let before_str = before.to_string();
                        let cursor_char_str = cursor_char.to_string();
                        let after_str = after.to_string();
                        let cursor_row_col_clone = cursor_row_col.clone();


                        let prefix = format!(
                            "↳ Ln {}, Col {}: ",
                            cursor_row_col_clone.map(|(r, _)| r + 1).unwrap_or(1),
                            cursor_row_col_clone.map(|(_, c)| c).unwrap_or(0),
                        );
                        let total = vw_clone.unwrap_or(80).saturating_sub(theme::TOTAL_VIEWPORT_OFFSET + 12) as usize;
                        let budget  = total
                            .saturating_sub(UnicodeWidthStr::width(prefix.as_str()))
                            .saturating_sub(1)
                            .max(8);
                        let before_keep = budget / 2;
                        let after_keep = budget - before_keep;
                        let before_win = tail_to_width(&before_str, before_keep);
                        let after_win = head_to_width(&after_str, after_keep);

                        let factory: Arc<dyn Fn() -> AnyElement<'static> + Send + Sync + 'static> = Arc::new(move || {
                            let rendered = match &block_clone {
                                Block::Heading { level, content, id: _, .. } => element!{Heading(level: *level, content: content.clone(), file_path: file_path_clone.clone(), viewport_height: vh_clone, viewport_width: vw_clone)}.into_any(),
                                Block::Paragraph { content, .. } => element!{Paragraph(content: content.clone(), file_path: file_path_clone.clone(), viewport_height: vh_clone, viewport_width: vw_clone)}.into_any(),
                                Block::Code { language, code, .. } => element!{CodeBlock(language: language.clone(), code: code.clone())}.into_any(),
                                Block::Mermaid { source, .. } => element!{MermaidBlock(source: source.clone(), viewport_height: vh_clone, viewport_width: vw_clone)}.into_any(),
                                Block::Math { content, display, .. } => element!{MathBlock(content: content.clone(), display: *display, viewport_height: vh_clone, viewport_width: vw_clone)}.into_any(),
                                Block::Quote { children, .. } => element!{QuoteBlock(children: children.clone(), file_path: Some(file_path_clone.clone()), viewport_height: vh_clone, viewport_width: vw_clone)}.into_any(),
                                Block::List { ordered, start, items, .. } => element!{ListBlock(ordered: *ordered, start: *start, items: items.clone(), file_path: file_path_clone.clone(), viewport_height: vh_clone, viewport_width: vw_clone)}.into_any(),
                                Block::Table { headers, alignments, rows, .. } => element!{TableBlock(headers: headers.clone(), alignments: alignments.clone(), rows: rows.clone(), file_path: file_path_clone.clone(), viewport_height: vh_clone, viewport_width: vw_clone)}.into_any(),
                                Block::ThematicBreak{..} => element!{ThematicBreak()}.into_any(),
                                Block::Image { alt, url, title, .. } => element!{Image(url: url.clone(), file_path: file_path_clone.clone(), title: title.clone(), alt: Some(alt.clone()), viewport_height: vh_clone, viewport_width: vw_clone)}.into_any(),
                                Block::Html { content, .. } => element!{HtmlBlock(content: content.clone())}.into_any(),
                            };

                            element! {
                                View(flex_direction: FlexDirection::Column) {
                                    View(flex_direction: FlexDirection::Row) {
                                        View(width: 2, background_color: theme::BLUE) {}
                                        View(flex_grow: 1.0, background_color: theme::STATUS_BG) {
                                            #(Some(rendered).into_iter())
                                        }
                                    }
                                    View(
                                        padding_left: 4,
                                        padding_right: 2,
                                        margin_bottom: 1,
                                        flex_direction: FlexDirection::Row,
                                        background_color: theme::DARK_BG,
                                    ) {
                                        Text(content: prefix.clone(), color: theme::YELLOW, weight: Weight::Bold)
                                        Text(content: before_win.clone(), color: theme::FG)
                                        View(background_color: theme::BLUE) {
                                            Text(content: cursor_char_str.clone(), color: theme::DARK_BG)
                                        }
                                        Text(content: after_win.clone(), color: theme::FG)
                                    }
                                }
                            }.into_any()
                        });

                        element! {
                            ScrollIntoViewContainer(
                                scroll_handle: props.scroll_handle.clone(),
                                viewport_height: props.viewport_height,
                                cursor_moved,
                                child: Some(factory),
                            )
                        }.into_any()
                    } else {
                        rendered
                    }
                }
            }))
        }
    }
}
