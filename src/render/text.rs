use super::theme::Theme;
use crate::document::model::*;
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

fn inlines_to_strings(inlines: &[Inline]) -> String {
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
