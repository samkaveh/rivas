use iocraft::prelude::*;
use crate::document::model::Inline;
use crate::components::inline_renderer::render_inlines_styled;

#[derive(Default, Props)]
pub struct ParagraphProps {
    pub content: Vec<Inline>,
}

#[component]
pub fn Paragraph(props: &ParagraphProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let styled_inlines = render_inlines_styled(&props.content, Color::White);

    element! {
        View(padding: 1, margin_bottom: 1, flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
            #(styled_inlines.iter().map(|(text, color, _bold, _italic)| {
                element! { Text(content: text.clone(), color: *color) }.into_any()
            }))
        }
    }
}
