use crate::components::blocks_renderer::BlocksRenderer;
use crate::document::model::ListItem;
use iocraft::prelude::*;
use std::path::PathBuf;

#[derive(Default, Props)]
pub struct ListBlockProps {
    pub ordered: bool,
    pub start: Option<u64>,
    pub items: Vec<ListItem>,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn ListBlock(props: &ListBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut num = props.start.unwrap_or(1);

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(props.items.iter().map(|item| {
                let marker = if let Some(checked) = item.checked {
                    if checked { "☒" } else { "☐" }.to_string()
                } else if props.ordered {
                    let m = format!("{}.", num);
                    num += 1;
                    m
                } else {
                    "•".to_string()
                };

                element! {
                    View(flex_direction: FlexDirection::Row) {
                        View(width: 4, padding_top: 1) {
                            Text(content: format!("{} ", marker), color: Color::Yellow)
                        }
                        View(flex_grow: 1.0) {
                            BlocksRenderer(
                                blocks: item.content.clone(),
                                file_path: props.file_path.clone(),
                                viewport_height: props.viewport_height,
                                viewport_width: props.viewport_width
                            )
                        }
                    }
                }
                .into_any()
            }))
        }
    }
}
