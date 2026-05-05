use std::path::PathBuf;

use iocraft::prelude::*;

use crate::assets::cache::AssetCache;
use crate::components::image::{Image, ImageProps};
use crate::document::model::{Block, inlines_to_text};
use crate::document::parser::parse_markdown;

#[derive(Default, Props)]
pub struct DocumentProps {
    pub content: String,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn Document(props: &DocumentProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
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
                        Block::Heading { level, content, id } => element!{View{Text(content: inlines_to_text(content), color: Color::Green)}}.into_any(),
                        Block::Image { alt, url, title } => element!{View{Image(url: url, file_path: base_dir, title: None, alt: None, viewport_height: vh, viewport_width: vw )}}.into_any(),
                        _ => element!{View{Text(content: "__", color: Color::Green)}}.into_any(),

                        }
                    ))
            }
            }
            }
        }
    }
}
