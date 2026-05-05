use iocraft::prelude::*;
use crate::document::model::ListItem;

#[derive(Default, Props)]
pub struct ListBlockProps {
    pub ordered: bool,
    pub start: Option<u64>,
    pub items: Vec<ListItem>,
}

#[component]
pub fn ListBlock(props: &ListBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let list_type = if props.ordered { "Ordered List" } else { "Unordered List" };
    let item_count = props.items.len();
    let mut num = props.start.unwrap_or(1);

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, margin_bottom: 1) {
            View(margin_bottom: 1) {
                Text(content: format!("{} ({} items)", list_type, item_count), color: Color::White)
            }
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
                    View(flex_direction: FlexDirection::Row, padding_left: 2) {
                        Text(content: format!("{} ", marker), color: Color::Yellow)
                        Text(content: format!("{} items", item.content.len()), color: Color::DarkGrey)
                    }
                }
                .into_any()
            }))
        }
    }
}
