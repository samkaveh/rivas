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
}

#[component]
pub fn Document(props: &DocumentProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let content = props.content.clone();
    let doc = parse_markdown(&content);

    let vh = props.viewport_height;
    let vw = props.viewport_width;

    let file_path = props.file_path.clone();

    element! {
    View(width: vw.unwrap_or(100), height: vh.unwrap_or(100), flex_direction: FlexDirection::Column, background_color: Color::AnsiValue(234)) {
        View(flex_grow: 1.0, border_style: BorderStyle::Single, border_color: Color::AnsiValue(238)){
                ScrollView {
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
