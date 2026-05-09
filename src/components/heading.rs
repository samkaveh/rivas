use crate::components::inline_renderer::render_inlines;
use crate::document::model::Inline;
use iocraft::prelude::*;
use std::path::PathBuf;

#[derive(Default, Props)]
pub struct HeadingProps {
    pub level: u8,
    pub content: Vec<Inline>,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn Heading(props: &HeadingProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let prefix = "#".repeat(props.level as usize);
    let color = match props.level {
        1 => Color::Cyan,
        2 => Color::Green,
        3 => Color::Yellow,
        _ => Color::White,
    };

    let styled_elements = render_inlines(
        &props.content,
        color,
        true,
        &props.file_path,
        props.viewport_height,
        props.viewport_width,
    );

    element! {
        View(padding: 1, margin_bottom: 1, flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
            Text(content: format!("{} ", prefix), color: color)
            #(styled_elements)
        }
    }
}
