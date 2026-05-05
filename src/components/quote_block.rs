use iocraft::prelude::*;
use crate::document::model::Block;
use std::path::PathBuf;
use crate::components::document::Document;
use crate::components::document::DocumentProps;

#[derive(Default, Props)]
pub struct QuoteBlockProps {
    pub children: Vec<Block>,
    pub file_path: Option<PathBuf>,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn QuoteBlock(props: &QuoteBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, margin_bottom: 1, padding_left: 2, border_style: BorderStyle::Single) {
            View(margin_bottom: 1) {
                Text(content: "Quote".to_string(), color: Color::Blue)
            }
            View {
                Text(content: format!("{} blocks", props.children.len()), color: Color::DarkGrey)
            }
        }
    }
}
