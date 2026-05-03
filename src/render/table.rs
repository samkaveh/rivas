use ratatui::text::{Line, Span};

use crate::{
    document::model::{self, TableCell},
    render::{text::RenderedBlock, theme::Theme},
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

    // Top border: ┌────────────────────┬────────────────────┐
    let top = format!(
        " ┌{}┐",
        (0..ncols)
            .map(|_| "─".repeat(col_width))
            .collect::<Vec<_>>()
            .join("┬")
    );
    lines.push(Line::from(Span::styled(top, theme.table_border)));

    // Header row — borders styled separately from header text
    {
        let mut spans: Vec<Span<'static>> = vec![Span::styled(" │", theme.table_border)];
        for (i, cell) in headers.iter().enumerate() {
            let text = model::inlines_to_text(&cell.content);
            let padded = format!("{:^w$}", text, w = col_width);
            spans.push(Span::styled(padded, theme.table_header));
            if i < ncols - 1 {
                spans.push(Span::styled("│", theme.table_border));
            }
        }
        spans.push(Span::styled("│", theme.table_border));
        lines.push(Line::from(spans));
    }

    // Separator: ├────────────────────┼────────────────────┤
    let sep = format!(
        " ├{}┤",
        (0..ncols)
            .map(|_| "─".repeat(col_width))
            .collect::<Vec<_>>()
            .join("┼")
    );
    lines.push(Line::from(Span::styled(sep, theme.table_border)));

    // Data rows — borders styled separately from cell text
    for row in rows {
        let mut spans: Vec<Span<'static>> = vec![Span::styled(" │", theme.table_border)];
        for i in 0..ncols {
            let text = if i < row.len() {
                model::inlines_to_text(&row[i].content)
            } else {
                String::new()
            };
            let padded = format!("{:<w$}", text, w = col_width);
            spans.push(Span::styled(padded, theme.text));
            if i < ncols - 1 {
                spans.push(Span::styled("│", theme.table_border));
            }
        }
        spans.push(Span::styled("│", theme.table_border));
        lines.push(Line::from(spans));
    }

    // Bottom border: └────────────────────┴────────────────────┘
    let bottom = format!(
        " └{}┘",
        (0..ncols)
            .map(|_| "─".repeat(col_width))
            .collect::<Vec<_>>()
            .join("┴")
    );
    lines.push(Line::from(Span::styled(bottom, theme.table_border)));

    RenderedBlock::Lines(lines)
}
