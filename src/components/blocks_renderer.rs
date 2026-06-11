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
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub scale: Option<f32>,
}

#[component]
pub fn BlocksRenderer(
    props: &BlocksRendererProps,
    _hooks: Hooks,
) -> impl Into<AnyElement<'static>> {
    let blocks = props.blocks.clone();
    let file_path = props.file_path.clone();
    let vh = props.viewport_height;
    let vw = props.viewport_width;
    let scale = props.scale;

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(blocks.iter().map(|block| match block {
                Block::Heading { level, content, id: _ } => element!{Heading(level: *level, content: content.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                Block::Paragraph { content } => element!{Paragraph(content: content.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                Block::Code { language, code } => element!{CodeBlock(language: language.clone(), code: code.clone())}.into_any(),
                Block::Mermaid { source } => element!{MermaidBlock(source: source.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                Block::Math { content, display } => element!{MathBlock(content: content.clone(), display: *display, viewport_height: vh, viewport_width: vw)}.into_any(),
                Block::Quote { children } => element!{QuoteBlock(children: children.clone(), file_path: Some(file_path.clone()), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                Block::List { ordered, start, items } => element!{ListBlock(ordered: *ordered, start: *start, items: items.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                Block::Table { headers, alignments, rows } => element!{TableBlock(headers: headers.clone(), alignments: alignments.clone(), rows: rows.clone(), file_path: file_path.clone(), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                Block::ThematicBreak => element!{ThematicBreak()}.into_any(),
                Block::Image { alt, url, title } => element!{Image(url: url.clone(), file_path: file_path.clone(), title: title.clone(), alt: Some(alt.clone()), viewport_height: vh, viewport_width: vw, scale)}.into_any(),
                Block::Html { content } => element!{HtmlBlock(content: content.clone())}.into_any(),
            }))
        }
    }
}
