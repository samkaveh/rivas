use iocraft::prelude::*;
use std::path::PathBuf;
use crate::document::model::{TableCell, Alignment};
use crate::components::inline_renderer::render_inlines;

#[derive(Default, Props)]
pub struct TableBlockProps {
    pub headers: Vec<TableCell>,
    pub alignments: Vec<Alignment>,
    pub rows: Vec<Vec<TableCell>>,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn TableBlock(props: &TableBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let ncols = props.headers.len();
    if ncols == 0 {
        return element! {
            View(padding: 1, margin_bottom: 1) {
                Text(content: "Empty table".to_string(), color: Color::DarkGrey)
            }
        }
        .into_any();
    }

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, margin_bottom: 1, border_style: BorderStyle::Single) {
            // Header Row
            View(flex_direction: FlexDirection::Row, border_style: BorderStyle::Single, border_edges: Edges::Bottom) {
                #(props.headers.iter().map(|cell| {
                    element! {
                        View(flex_grow: 1.0, padding: 1, border_style: BorderStyle::Single, border_edges: Edges::Right) {
                            View(flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
                                #(render_inlines(&cell.content, Color::Cyan, &props.file_path, props.viewport_height, props.viewport_width))
                            }
                        }
                    }.into_any()
                }))
            }
            // Data Rows
            #(props.rows.iter().map(|row| {
                element! {
                    View(flex_direction: FlexDirection::Row, border_style: BorderStyle::Single, border_edges: Edges::Bottom) {
                        #(row.iter().map(|cell| {
                            element! {
                                View(flex_grow: 1.0, padding: 1, border_style: BorderStyle::Single, border_edges: Edges::Right) {
                                    View(flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
                                        #(render_inlines(&cell.content, Color::White, &props.file_path, props.viewport_height, props.viewport_width))
                                    }
                                }
                            }.into_any()
                        }))
                    }
                }.into_any()
            }))
        }
    }.into_any()
}
