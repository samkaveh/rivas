use std::path::PathBuf;

use iocraft::prelude::*;

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
use crate::document::parser::parse_markdown;

#[derive(Default, Props)]
pub struct DocumentProps {
    pub content: String,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn Document(props: &DocumentProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let content = props.content.clone();
    let doc = parse_markdown(&content);

    let vh = props.viewport_height;
    let vw = props.viewport_width;

    let base_dir = &props.file_path;

    element! {
    View(width: vw.unwrap_or(100), height: vh.unwrap_or(100), flex_direction: FlexDirection::Column, background_color: Color::AnsiValue(235)) {
        View(flex_grow: 1.0, border_style: BorderStyle::Single){
                ScrollView {
                    View(flex_direction:FlexDirection::Column, padding: 1){
                    #(doc.blocks.iter().map(|block| match block {
                        Block::Heading { level, content, id: _ } => element!{Heading(level: *level, content: content.clone())}.into_any(),
                        Block::Paragraph { content } => element!{Paragraph(content: content.clone())}.into_any(),
                        Block::Code { language, code } => element!{CodeBlock(language: language.clone(), code: code.clone())}.into_any(),
                        Block::Mermaid { source } => element!{MermaidBlock(source: source.clone(), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Math { content, display } => element!{MathBlock(content: content.clone(), display: *display, viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Quote { children } => element!{QuoteBlock(children: children.clone(), file_path: Some(base_dir.clone()), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::List { ordered, start, items } => element!{ListBlock(ordered: *ordered, start: *start, items: items.clone())}.into_any(),
                        Block::Table { headers, alignments, rows } => element!{TableBlock(headers: headers.clone(), alignments: alignments.clone(), rows: rows.clone())}.into_any(),
                        Block::ThematicBreak => element!{ThematicBreak()}.into_any(),
                        Block::Image { alt, url, title } => element!{Image(url: url.clone(), file_path: base_dir.clone(), title: title.clone(), alt: Some(alt.clone()), viewport_height: vh, viewport_width: vw)}.into_any(),
                        Block::Html { content } => element!{HtmlBlock(content: content.clone())}.into_any(),
                        _ => element!{View{Text(content: "__", color: Color::Green)}}.into_any(),
                    }))
            }
            }
            }
        }
    }
}
