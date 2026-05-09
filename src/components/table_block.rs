use crate::components::inline_renderer::render_inlines;
use crate::document::model::{Alignment, TableCell};
use iocraft::prelude::*;
use std::path::PathBuf;

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
        View(
            flex_direction: FlexDirection::Column,
            margin_bottom: 1,
            border_style: BorderStyle::Round,
            border_color: Color::Grey,
            width: 100pct,
        ) {
            // Header Row
            View(border_style: BorderStyle::Single, border_edges: Edges::Bottom, border_color: Color::Grey) {
                #(props.headers.iter().enumerate().map(|(i, cell)| {
                    let alignment = props.alignments.get(i).cloned().unwrap_or(Alignment::None);
                    let justify = match alignment {
                        Alignment::Left | Alignment::None => JustifyContent::Start,
                        Alignment::Center => JustifyContent::Center,
                        Alignment::Right => JustifyContent::End,
                    };
                    element! {
                        View(flex_grow: 1.0, width: 0pct, justify_content: justify, padding: 1) {
                            View(flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
                                #(render_inlines(&cell.content, Color::Cyan, true, &props.file_path, props.viewport_height, props.viewport_width))
                            }
                        }
                    }.into_any()
                }))
            }
            // Data Rows
            #(props.rows.iter().enumerate().map(|(row_idx, row)| {
                element! {
                    View(
                        flex_direction: FlexDirection::Row,
                        background_color: if row_idx % 2 == 1 { Some(Color::Rgb{r: 35, g: 38, b: 52}) } else { None }
                    ) {
                        #(row.iter().enumerate().map(|(col_idx, cell)| {
                            let alignment = props.alignments.get(col_idx).cloned().unwrap_or(Alignment::None);
                            let justify = match alignment {
                                Alignment::Left | Alignment::None => JustifyContent::Start,
                                Alignment::Center => JustifyContent::Center,
                                Alignment::Right => JustifyContent::End,
                            };
                            element! {
                                View(flex_grow: 1.0, width: 0pct, justify_content: justify, padding: 1) {
                                    View(flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
                                        #(render_inlines(&cell.content, Color::White, false, &props.file_path, props.viewport_height, props.viewport_width))
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
