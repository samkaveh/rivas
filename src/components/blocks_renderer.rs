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
use iocraft::prelude::*;
use std::path::PathBuf;

#[derive(Default, Props)]
pub struct BlocksRendererProps {
    pub blocks: Vec<Block>,
    pub content: String,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub cursor_offset: Option<Ref<usize>>,
    pub editor_state: Option<Ref<Option<EditorState>>>,
}

#[component]
pub fn BlocksRenderer(
    props: &BlocksRendererProps,
    _hooks: Hooks,
) -> impl Into<AnyElement<'static>> {
    let file_path = props.file_path.clone();
    let vh = props.viewport_height;
    let vw = props.viewport_width;
    let cursor_offset = props.cursor_offset.as_ref().map(|r| r.get());

    let (vis_start, vis_end, mode) = if let Some(state_ref) = &props.editor_state {
        let s_opt = state_ref.read();
        if let Some(s) = s_opt.as_ref() {
            let start = s.absolute_byte_offset_at(s.visual_start.0, s.visual_start.1);
            let end = s.absolute_byte_offset();
            (Some(start.min(end)), Some(start.max(end)), s.mode.clone())
        } else {
            (None, None, Mode::Normal)
        }
    } else {
        (None, None, Mode::Normal)
    };

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(props.blocks.iter().map(|block| {
                let span = block.span();
                let is_active = cursor_offset.map_or(false, |off| off >= span.0 && off <= span.1);
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
                        Mode::Normal => crate::theme::FG,
                        Mode::Insert => crate::theme::GREEN,
                        Mode::Visual => crate::theme::MAGENTA,
                        Mode::Command | Mode::Search { .. } => crate::theme::YELLOW,
                    };

                    let (cursor_fg, cursor_bg_final, cursor_char) = if let Some(state_ref) = &props.editor_state {
                        let s_opt = state_ref.read();
                        if let Some(s) = s_opt.as_ref() {
                            if s.mode == Mode::Insert {
                                (cursor_bg, crate::theme::DARK_BG, "┃")
                            } else if s.operator.is_some() {
                                (cursor_bg, crate::theme::DARK_BG, "_")
                            } else {
                                (crate::theme::DARK_BG, cursor_bg, " ")
                            }
                        } else {
                            (crate::theme::DARK_BG, cursor_bg, " ")
                        }
                    } else {
                        (crate::theme::DARK_BG, cursor_bg, " ")
                    };

                    element! {
                        View(
                            background_color: crate::theme::DARK_BG,
                            padding_left: 2,
                            padding_right: 2,
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::Hidden,
                        ) {
                            #(lines.iter().enumerate().map(|(idx, line)| {
                                let line_start_off = span.0 + lines[..idx].iter().map(|l| l.len() + 1).sum::<usize>();
                                let wrap_width = (vw.unwrap_or(80) as i32 - crate::theme::TOTAL_VIEWPORT_OFFSET as i32).max(1) as usize;
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
                                                                Text(content: text.clone(), color: if *selected { crate::theme::MAGENTA } else { crate::theme::FG }, wrap: TextWrap::Wrap)
                                                            }))
                                                        }
                                                    }.into_any()
                                                } else {
                                                    element! { Text(content: segment.to_string(), color: crate::theme::FG, wrap: TextWrap::Wrap) }.into_any()
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

                                                    if let Some(c) = after_with_char.chars().next() {
                                                        let char_len = c.len_utf8();
                                                        let after = &after_with_char[char_len..];

                                                        if let Some(state_ref) = &props.editor_state {
                                                            let s_opt = state_ref.read();
                                                            if let Some(s) = s_opt.as_ref() {
                                                                if s.mode == Mode::Insert {
                                                                    element! {
                                                                        View(flex_direction: FlexDirection::Row) {
                                                                            Text(content: before, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                            View(background_color: cursor_bg_final, width: 1) {
                                                                                Text(content: cursor_char, color: cursor_fg, wrap: TextWrap::Wrap)
                                                                            }
                                                                            Text(content: &format!("{}{}", c, after), color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                        }
                                                                    }.into_any()
                                                                } else if s.operator.is_some() {
                                                                    element! {
                                                                        View(flex_direction: FlexDirection::Row) {
                                                                            Text(content: before, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                            View(background_color: cursor_bg_final, width: 1) {
                                                                                Text(content: c.to_string(), color: cursor_fg, wrap: TextWrap::Wrap)
                                                                            }
                                                                            Text(content: after, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                        }
                                                                    }.into_any()
                                                                } else {
                                                                    element! {
                                                                        View(flex_direction: FlexDirection::Row) {
                                                                            Text(content: before, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                            View(background_color: cursor_bg_final, width: 1) {
                                                                                Text(content: c.to_string(), color: cursor_fg, wrap: TextWrap::Wrap)
                                                                            }
                                                                            Text(content: after, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                        }
                                                                    }.into_any()
                                                                }
                                                            } else {
                                                                element! {
                                                                    View(flex_direction: FlexDirection::Row) {
                                                                        Text(content: before, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                        View(background_color: cursor_bg_final, width: 1) {
                                                                            Text(content: c.to_string(), color: cursor_fg, wrap: TextWrap::Wrap)
                                                                        }
                                                                        Text(content: after, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                    }
                                                                }.into_any()
                                                            }
                                                        } else {
                                                            element! {
                                                                View(flex_direction: FlexDirection::Row) {
                                                                    Text(content: before, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                    View(background_color: cursor_bg_final, width: 1) {
                                                                        Text(content: c.to_string(), color: cursor_fg, wrap: TextWrap::Wrap)
                                                                    }
                                                                    Text(content: after, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                }
                                                            }.into_any()
                                                        }
                                                    } else {
                                                        element! {
                                                            View(flex_direction: FlexDirection::Row) {
                                                                Text(content: before, color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                                View(background_color: cursor_bg_final, width: 1) {
                                                                    Text(content: cursor_char, color: cursor_fg, wrap: TextWrap::Wrap)
                                                                }
                                                                Text(content: "", color: crate::theme::FG, wrap: TextWrap::Wrap)
                                                            }
                                                        }.into_any()
                                                    }
                                                } else {
                                                    element! { Text(content: segment.to_string(), color: crate::theme::FG, wrap: TextWrap::Wrap) }.into_any()
                                                }
                                            } else {
                                                element! { Text(content: segment.to_string(), color: crate::theme::FG, wrap: TextWrap::Wrap) }.into_any()
                                            }
                                        }))
                                    }
                                }.into_any()
                            }))
                        }
                    }.into_any()
                } else {
                    match block {
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
                    }
                }
            }))
        }
    }
}
