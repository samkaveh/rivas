use ratatui::text::{Line, Span};

use crate::{
    document::model::TableCell,
    render::{
        text::{RenderedBlock, inlines_to_strings},
        theme::Theme,
    },
};

pub fn render_table(
    headers: &[TableCell],
    rows: &[Vec<TableCell>],
    theme: &Theme,
) -> RenderedBlock {
    let ncols = headers.len();
    if ncols == 0 {
        return RenderedBlock::Lines(vec![]);
    }

    let col_width = 20usize;

    let mut lines = Vec::new();

    let top = format!(" ┌{}┐ ", vec!["-".repeat(col_width); ncols].join("┬"));
    lines.push(Line::from(Span::styled(top, theme.table_border)));

    let header_cells: Vec<String> = headers
        .iter()
        .map(|cell| {
            let text = inlines_to_strings(&cell.content);
            format!("{:^w$}", text, w = col_width)
        })
        .collect();
    let header_line = format!(" |{}|", header_cells.join("|"));
    lines.push(Line::from(Span::styled(header_line, theme.table_header)));

    let sep = format!(" ├{}┤", vec!["-".repeat(col_width); ncols].join("┼"));
    lines.push(Line::from(Span::styled(sep, theme.table_border)));

    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| {
                let text = inlines_to_strings(&cell.content);
                format!("{:<w$}", text, w = col_width)
            })
            .collect();
        let mut padded = cells;
        while padded.len() < ncols {
            padded.push(" ".repeat(col_width));
        }
        let row_line = format!(" |{}|", header_cells.join("|"));
        lines.push(Line::from(Span::styled(row_line, theme.text)));
    }

    let bottom = format!(" └{}┘ ", vec!["-".repeat(col_width); ncols].join("┴"));
    lines.push(Line::from(Span::styled(bottom, theme.table_border)));

    RenderedBlock::Lines(lines)
}
