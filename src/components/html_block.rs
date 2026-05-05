use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct HtmlBlockProps {
    pub content: String,
}

#[component]
pub fn HtmlBlock(props: &HtmlBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let preview = props
        .content
        .lines()
        .next()
        .unwrap_or("<html>")
        .chars()
        .take(50)
        .collect::<String>();

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, margin_bottom: 1, border_style: BorderStyle::Single) {
            View(margin_bottom: 1) {
                Text(content: "HTML Block".to_string(), color: Color::Red)
            }
            View {
                Text(content: preview, color: Color::DarkGrey)
            }
        }
    }
}
