use crate::components::code_block::CodeBlock;
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
    pub scale: Option<f32>,
}

#[component]
pub fn BlocksRenderer(
    props: &BlocksRendererProps,
    _hooks: Hooks,
) -> impl Into<AnyElement<'static>> {
    let file_path = props.file_path.clone();
    let vh = props.viewport_height;
    let vw = props.viewport_width;
    let scale = props.scale;
    let cursor_offset = props.cursor_offset.as_ref().map(|r| r.get());

    element! {
        View(flex_direction: FlexDirection::Column) {
            // Iterate over &props.blocks instead of cloning the entire tree
            #(props.blocks.iter().map(|block| {
                let span = block.span();
                let is_active = cursor_offset.map_or(false, |off| off >= span.0 && off <= span.1);

                if is_active {
                    let off = cursor_offset.unwrap();
                    let text = &props.content[span.0..span.1];
                    let rel_off = off.saturating_sub(span.0);
                    let rel_off = rel_off.min(text.len());
                    
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
                        current_byte_acc += line_len + 1; // +1 for \n
                    }

                    element! {
                        View(
                            background_color: crate::theme::DARK_BG,
                            padding_left: 2,
                            padding_right: 2,
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::Hidden,
                        ) {
                            #(lines.iter().enumerate().map(|(idx, line)| {
                                if Some(idx) == cursor_line_idx {
                                    let (before, after) = line.split_at(cursor_rel_off.min(line.len()));
                                    element! {
                                        View(flex_direction: FlexDirection::Row) {
                                            Text(content: before, color: crate::theme::FG)
                                            View(background_color: crate::theme::FG, width: 1) {
                                                Text(content: " ", color: crate::theme::DARK_BG)
                                            }
                                            Text(content: after, color: crate::theme::FG)
                                        }
                                    }.into_any()
                                } else {
                                    element! {
                                        Text(content: line.to_string(), color: crate::theme::FG)
                                    }.into_any()
                                }
                            }))
                        }
                    }.into_any()
                } else {
                    match block {
                        Block::Heading { level, content, id: _, .. } => element!{Heading(level: *level, content: content.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                        Block::Paragraph { content, .. } => element!{Paragraph(content: content.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                        Block::Code { language, code, .. } => element!{CodeBlock(language: language.clone(), code: code.clone())}.into_any(),
                        Block::Mermaid { source, .. } => element!{MermaidBlock(source: source.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                        Block::Math { content, display, .. } => element!{MathBlock(content: content.clone(), display: *display, viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Quote { children, .. } => element!{QuoteBlock(children: children.clone(), file_path: Some(file_path.clone()), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                        Block::List { ordered, start, items, .. } => element!{ListBlock(ordered: *ordered, start: *start, items: items.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                        Block::Table { headers, alignments, rows, .. } => element!{TableBlock(headers: headers.clone(), alignments: alignments.clone(), rows: rows.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                        Block::ThematicBreak{..} => element!{ThematicBreak()}.into_any(),
                        Block::Image { alt, url, title, .. } => element!{Image(url: url.clone(), file_path: file_path.clone(), title: title.clone(), alt: Some(alt.clone()), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                        Block::Html { content, .. } => element!{HtmlBlock(content: content.clone())}.into_any(),
                    }
                }
            }))
        }
    }
}
