use super::theme::Theme;
use crate::document::model::*;
use crate::render::code::render_code_block;
use crate::render::table::render_table;
use crate::render::theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// A rendered block: either styled text lines or a placeholder for rich content.
pub enum RenderedBlock {
    /// Terminal text lines (headings, paragraphs, code, lists, etc.)
    Lines(Vec<Line<'static>>),
    /// Placeholder for content that needs Kitty graphics
    /// Contains: (rows_needed, label_text)
    ImagePlaceholder { label: String, rows: u16 },
}

/// Render all blocks in the document to a flat list of ratatui Lines.
/// Inserts blank lines between blocks for spacing
pub fn render_document(blocks: &[Block], theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for block in blocks {
        let rendered = render_block(block, theme, 0);
        match rendered {
            RenderedBlock::Lines(block_lines) => {
                lines.extend(block_lines);
            }
            RenderedBlock::ImagePlaceholder { label, .. } => {
                lines.push(Line::from(Span::styled(
                    format!(" [{}] ", label),
                    theme.placeholder,
                )));
            }
        }
        lines.push(Line::from(""));
    }
    lines
}

fn render_block(block: &Block, theme: &Theme, indent: usize) -> RenderedBlock {
    let pad = " ".repeat(indent);
    match block {
        Block::Heading { level, content, .. } => {
            let prefix = "#".repeat(*level as usize);
            let style = theme.heading_style(*level);
            let mut spans = vec![Span::styled(format!("{}{} ", pad, prefix), style)];
            render_inlines(content, style, theme, &mut spans);
            RenderedBlock::Lines(vec![Line::from(spans)])
        }

        Block::Paragraph { content } => {
            let mut spans = Vec::new();
            if indent > 0 {
                spans.push(Span::raw(pad.clone()));
            }
            render_inlines(content, theme.text, theme, &mut spans);
            RenderedBlock::Lines(vec![Line::from(spans)])
        }
        Block::Code { language, code } => render_code_block(language.as_deref(), code, theme),
        Block::Quote { children } => {
            let mut lines = Vec::new();
            for child in children {
                if let RenderedBlock::Lines(child_lines) = render_block(child, theme, 0) {
                    for line in child_lines {
                        let mut spans = vec![Span::styled("  | ", theme.blockquote_bar)];
                        spans.extend(
                            line.spans
                                .into_iter()
                                .map(|s| Span::styled(s.content, theme.blockquote_text)),
                        );
                        lines.push(Line::from(spans));
                    }
                }
            }
            RenderedBlock::Lines(lines)
        }
        Block::List {
            ordered,
            start,
            items,
        } => {
            let mut lines = Vec::new();
            let mut num = start.unwrap_or(1);
            for item in items {
                let marker = if let Some(checked) = item.checked {
                    if checked { " ☒ " } else { " ☐" }
                } else if *ordered {
                    &format!(" {}.", num)
                } else {
                    " ."
                };

                let marker_span = Span::styled(marker.to_string(), theme.list_marker);

                for (i, child_block) in item.content.iter().enumerate() {
                    if let RenderedBlock::Lines(child_lines) = render_block(child_block, theme, 4) {
                        for (j, line) in child_lines.into_iter().enumerate() {
                            let mut spans = Vec::new();
                            if i == 0 && j == 0 {
                                spans.push(marker_span.clone());
                            } else {
                                spans.push(Span::raw("  "));
                            }
                            spans.extend(line.spans);
                            lines.push(Line::from(spans));
                        }
                    }
                }
                num += 1;
            }

            RenderedBlock::Lines(lines)
        }

        Block::Table {
            headers,
            alignments,
            rows,
        } => render_table(headers, rows, theme),
        Block::ThematicBreak => RenderedBlock::Lines(vec![Line::from(Span::styled(
            " ____________________________",
            theme.rule,
        ))]),
        Block::Image { alt, url, .. } => RenderedBlock::ImagePlaceholder {
            label: format!("Image: {} ({})", alt, url),
            rows: 1,
        },

        Block::Mermaid { source } => RenderedBlock::ImagePlaceholder {
            label: format!("Mermaid: {}", source),
            rows: 1,
        },
        Block::Math { content, .. } => RenderedBlock::ImagePlaceholder {
            label: format!("Math: {}", content),
            rows: 1,
        },

        _ => RenderedBlock::Lines(vec![Line::from("")]),
    }
}

/// Flatten inline into ratatui Spans with nested styles.
fn render_inlines(inlines: &[Inline], base: Style, theme: &Theme, out: &mut Vec<Span<'static>>) {
    for inline in inlines {
        match inline {
            Inline::Text(t) => out.push(Span::styled(t.clone(), base)),
            Inline::Bold(ch) => render_inlines(ch, base.add_modifier(Modifier::BOLD), theme, out),
            Inline::Italic(ch) => {
                render_inlines(ch, base.add_modifier(Modifier::ITALIC), theme, out)
            }
            Inline::Strikethrough(ch) => {
                render_inlines(ch, base.add_modifier(Modifier::CROSSED_OUT), theme, out)
            }
            Inline::Code(c) => out.push(Span::styled(format!(" {} ", c), theme.inline_code)),
            Inline::Math(m) => {
                out.push(Span::styled(m.clone(), base.add_modifier(Modifier::ITALIC)))
            }
            Inline::Link { text, url, .. } => {
                let label = inlines_to_strings(text);
                let hyperlink = format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, label);
                out.push(Span::styled(hyperlink, theme.inline_code));
            }
            Inline::SoftBreak => out.push(Span::raw(" ")),
            Inline::HardBreak => {}
            Inline::Image { alt, .. } => {
                out.push(Span::styled(format!("[{}]", alt), theme.placeholder))
            }
        }
    }
}

pub fn inlines_to_strings(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for i in inlines {
        match i {
            Inline::Text(t) => s.push_str(t),
            Inline::Code(c) | Inline::Math(c) => s.push_str(c),
            Inline::Bold(ch) | Inline::Italic(ch) | Inline::Strikethrough(ch) => {
                s.push_str(&inlines_to_strings(ch))
            }
            Inline::Link { text, .. } => s.push_str(&inlines_to_strings(text)),
            Inline::SoftBreak => s.push(' '),
            _ => {}
        }
    }
    s
}
