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
        View(flex_direction: FlexDirection::Column, padding_left: 2, padding_right: 2, margin_bottom: 1, border_style: BorderStyle::Single) {
            View() {
                Text(content: "HTML Block".to_string(), color: crate::theme::RED)
            }
            View {
                Text(content: preview, color: crate::theme::COMMENT)
            }
        }
    }
}
