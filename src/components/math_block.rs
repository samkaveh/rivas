use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct MathBlockProps {
    pub content: String,
    pub display: bool,
}

#[component]
pub fn MathBlock(props: &MathBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mode = if props.display { "Display" } else { "Inline" };
    let preview = props
        .content
        .chars()
        .take(40)
        .collect::<String>();

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, margin_bottom: 1) {
            View(margin_bottom: 1) {
                Text(content: format!("Math ({})", mode), color: Color::Yellow)
            }
            View {
                Text(content: preview, color: Color::Green)
            }
        }
    }
}
