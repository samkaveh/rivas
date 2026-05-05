use iocraft::prelude::*;
use crate::document::model::Inline;
use crate::components::inline_renderer::render_inlines_styled;

#[derive(Default, Props)]
pub struct HeadingProps {
    pub level: u8,
    pub content: Vec<Inline>,
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

    let styled_inlines = render_inlines_styled(&props.content, color);

    element! {
        View(padding: 1, margin_bottom: 1, flex_direction: FlexDirection::Row) {
            Text(content: format!("{} ", prefix), color: color)
            #(styled_inlines.iter().map(|(text, color, _bold, _italic)| {
                element! { Text(content: text.clone(), color: *color) }.into_any()
            }))
        }
    }
}
