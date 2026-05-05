use crate::components::blocks_renderer::BlocksRenderer;
use crate::document::model::Block;
use iocraft::prelude::*;
use std::path::PathBuf;

#[derive(Default, Props)]
pub struct QuoteBlockProps {
    pub children: Vec<Block>,
    pub file_path: Option<PathBuf>,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn QuoteBlock(props: &QuoteBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let file_path = props.file_path.clone().unwrap_or_default();
    element! {
        View(flex_direction: FlexDirection::Row, padding: 1, margin_bottom: 1, padding_left: 2, background_color: Color::DarkGrey) {
            View() {
                Text(content: " ▎".to_string(), color: Color::White)
            }
            BlocksRenderer(
                blocks: props.children.clone(),
                file_path: file_path,
                viewport_height: props.viewport_height,
                viewport_width: props.viewport_width
            )
        }
    }
}
