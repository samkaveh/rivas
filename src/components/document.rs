use iocraft::prelude::*;

use crate::components::image::{Image, ImageProps};
use crate::document::model::{Block, inlines_to_text};
use crate::document::parser::parse_markdown;

#[derive(Default, Props)]
pub struct DocumentProps {
    pub content: String,
}

#[component]
pub fn Document(props: &DocumentProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut should_exit = hooks.use_state(|| false);
    let (stdout_handle, _) = hooks.use_output();

    let content = props.content.clone();
    let doc = parse_markdown(&content);

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1) {
            #(doc.blocks.iter().map(|block| match block {
                    Block::Heading { level, content, id } => element!{View{Text(content: inlines_to_text(content), color: Color::Green)}}.into_any(),
                Block::Image { alt, url, title } => element!{View{Image(url, title: None, alt: None)}}.into_any(),
                _ => element!{View{Text(content: "__", color: Color::Green)}}.into_any(),

                }
            ))
        }
    }
}
