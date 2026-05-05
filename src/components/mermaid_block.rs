use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct MermaidBlockProps {
    pub source: String,
}

#[component]
pub fn MermaidBlock(props: &MermaidBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let preview = props
        .source
        .lines()
        .next()
        .unwrap_or("Mermaid diagram")
        .to_string();

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, margin_bottom: 1, border_style: BorderStyle::Double) {
            View(margin_bottom: 1) {
                Text(content: "Mermaid Diagram".to_string(), color: Color::Cyan)
            }
            View {
                Text(content: preview, color: Color::DarkGrey)
            }
        }
    }
}
