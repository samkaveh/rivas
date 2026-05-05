use iocraft::prelude::*;
use crate::document::model::{TableCell, Alignment, inlines_to_text};
use crate::components::inline_renderer::render_inlines_styled;

#[derive(Default, Props)]
pub struct TableBlockProps {
    pub headers: Vec<TableCell>,
    pub alignments: Vec<Alignment>,
    pub rows: Vec<Vec<TableCell>>,
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

    let col_width = 20usize;
    let mut lines = Vec::new();

    // Top border
    let top = format!(
        " ┌{}┐",
        (0..ncols)
            .map(|_| "─".repeat(col_width))
            .collect::<Vec<_>>()
            .join("┬")
    );
    lines.push(top);

    // Header row with styled inlines
    let mut header_row = String::from(" │");
    for (i, cell) in props.headers.iter().enumerate() {
        let text = inlines_to_text(&cell.content);
        let padded = format!("{:^w$}", text, w = col_width);
        header_row.push_str(&padded);
        if i < ncols - 1 {
            header_row.push('│');
        }
    }
    header_row.push('│');
    lines.push(header_row);

    // Separator
    let sep = format!(
        " ├{}┤",
        (0..ncols)
            .map(|_| "─".repeat(col_width))
            .collect::<Vec<_>>()
            .join("┼")
    );
    lines.push(sep);

    // Data rows
    for row in &props.rows {
        let mut row_str = String::from(" │");
        for i in 0..ncols {
            let text = if i < row.len() {
                inlines_to_text(&row[i].content)
            } else {
                String::new()
            };
            let padded = format!("{:<w$}", text, w = col_width);
            row_str.push_str(&padded);
            if i < ncols - 1 {
                row_str.push('│');
            }
        }
        row_str.push('│');
        lines.push(row_str);
    }

    // Bottom border
    let bottom = format!(
        " └{}┘",
        (0..ncols)
            .map(|_| "─".repeat(col_width))
            .collect::<Vec<_>>()
            .join("┴")
    );
    lines.push(bottom);

    let table_content = lines.join("\n");

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, margin_bottom: 1) {
            View {
                Text(content: table_content, color: Color::Cyan)
            }
        }
    }
    .into_any()
}
