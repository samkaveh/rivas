use crate::components::inline_renderer::render_inlines;
use crate::document::model::Inline;
use iocraft::prelude::*;
use std::path::PathBuf;

#[derive(Default, Props)]
pub struct ParagraphProps {
    pub content: Vec<Inline>,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub scale: Option<f32>,
}

#[component]
pub fn Paragraph(props: &ParagraphProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let styled_elements = render_inlines(
        &props.content,
        crate::theme::FG,
        false,
        &props.file_path,
        props.viewport_height,
        props.viewport_width,
        props.scale,
    );

    element! {
        View(margin_bottom: 1, flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
            #(styled_elements)
        }
    }
}
