use std::io::Stdout;
use std::path::Path;

use super::theme::Theme;
use crate::assets::cache::AssetCache;
use crate::document::model::*;
use crate::output::capabilities::TermCaps;
use crate::output::kitty::KittyWriter;
use crate::render::code::render_code_block;
use crate::render::table::render_table;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Rendered output ready for the viewer.
pub struct DocumentRender {
    pub lines: Vec<Line<'static>>,
    pub images: Vec<PendingImage>,
}

pub struct PendingImage {
    pub row: u16, // row index in the lines vector
    pub png_data: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
    pub rows: u16, // rows the images will occupy
    pub image_id: u32,
}

/// A rendered block: either styled text lines or a placeholder for rich content.
pub enum RenderedBlock {
    /// Terminal text lines (headings, paragraphs, code, lists, etc.)
    Lines(Vec<Line<'static>>),
    ImagePlaceholder {
        label: String,
        rows: u16,
    },
    /// A Kitty image to display inline.
    /// first rows of blank lines are set in output which are replaced later.
    InlineImage {
        png_data: Vec<u8>,
        width_px: u32,
        height_px: u32,
        rows: u16,
        label: String,
    },
    /// Fallback text for failed renders.
    Fallback(Vec<Line<'static>>),
}

/// Render all blocks in the document to a flat list of ratatui Lines.
/// Inserts blank lines between blocks for spacing
pub fn render_document(
    blocks: &[Block],
    theme: &Theme,
    cache: &mut AssetCache,
    caps: &TermCaps,
    base_dir: Option<&Path>,
    kitty: &mut KittyWriter<Stdout>,
) -> DocumentRender {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut images: Vec<PendingImage> = Vec::new();

    for block in blocks {
        let rendered = render_block(block, theme, 0, Some(cache), Some(caps), base_dir);
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
            RenderedBlock::InlineImage {
                png_data,
                width_px,
                height_px,
                rows,
                label,
            } => {
                if caps.has_kitty {
                    let row_pos = lines.len() as u16;
                    for _ in 0..rows {
                        lines.push(Line::from(""));
                    }
                    images.push(PendingImage {
                        row: row_pos,
                        png_data,
                        width_px,
                        height_px,
                        rows,
                        image_id: kitty.alloc_id(),
                    });
                } else {
                    lines.push(Line::from(Span::styled(
                        format!(" [{}]", label),
                        theme.placeholder,
                    )));
                }
            }
            RenderedBlock::Fallback(fallback_lines) => {
                lines.extend(fallback_lines);
            }
        }
        lines.push(Line::from(""));
    }
    DocumentRender { lines, images }
}

fn render_block(
    block: &Block,
    theme: &Theme,
    indent: usize,
    cache: Option<&mut AssetCache>,
    caps: Option<&TermCaps>,
    base_dir: Option<&Path>,
) -> RenderedBlock {
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
                if let RenderedBlock::Lines(child_lines) =
                    render_block(child, theme, 0, None, caps, base_dir)
                {
                    for line in child_lines {
                        let mut spans = vec![Span::styled("  ▎ ", theme.blockquote_bar)];
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
                    if let RenderedBlock::Lines(child_lines) =
                        render_block(child_block, theme, 4, None, caps, base_dir)
                    {
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
            alignments: _,
            rows,
        } => render_table(headers, rows, theme),
        Block::ThematicBreak => RenderedBlock::Lines(vec![Line::from(Span::styled(
            " ╶─────────────────────────────────╴",
            theme.rule,
        ))]),
        Block::Image { alt, url, .. } => render_image_block(url, alt, theme, cache, caps, base_dir),

        Block::Mermaid { source } => render_mermaid_block(source, theme, cache, caps),
        Block::Math { content, display } => {
            render_math_block(content, display, theme, cache, caps, base_dir)
        }

        _ => RenderedBlock::Lines(vec![Line::from("")]),
    }
}

fn render_math_block(
    content: &str,
    display: &bool,
    theme: &Theme,
    cache: Option<&mut AssetCache>,
    caps: Option<&TermCaps>,
    _base_dir: Option<&Path>,
) -> RenderedBlock {
    let (Some(cache), Some(caps)) = (cache, caps) else {
        return RenderedBlock::ImagePlaceholder {
            label: format!("Math: {}", content),
            rows: 1,
        };
    };

    match cache.get_or_render_math(content, *display, caps.content_width_px(), theme.is_dark) {
        Ok((png, w, h)) => {
            if caps.has_kitty {
                return RenderedBlock::InlineImage {
                    png_data: png.clone(),
                    width_px: *w,
                    height_px: *h,
                    rows: caps.image_rows(*h),
                    label: "Math Block".to_string(),
                };
            } else {
                RenderedBlock::ImagePlaceholder {
                    label: format!("Math: {}", content),
                    rows: 1,
                }
            }
        }
        Err(err_msg) => {
            let error_style = Style::default().fg(Color::Red);
            RenderedBlock::Fallback(vec![
                Line::from(vec![
                    Span::styled(" ⚠ Math rendering error:", error_style),
                    Span::styled(err_msg.to_string(), error_style),
                ]),
                Line::from(Span::styled(
                    format!("  src {}", content),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        }
    }
}

fn render_image_block(
    url: &str,
    alt: &str,
    _theme: &Theme,
    cache: Option<&mut AssetCache>,
    caps: Option<&TermCaps>,
    base_dir: Option<&Path>,
) -> RenderedBlock {
    let (Some(cache), Some(caps)) = (cache, caps) else {
        return RenderedBlock::ImagePlaceholder {
            label: format!("Image: {} ({})", alt, url),
            rows: 1,
        };
    };

    match cache.get_or_load_image(url, base_dir, caps.content_width_px()) {
        Ok((png, w, h)) => {
            if caps.has_kitty {
                return RenderedBlock::InlineImage {
                    png_data: png.clone(),
                    width_px: *w,
                    height_px: *h,
                    rows: caps.image_rows(*h),
                    label: alt.to_string(),
                };
            } else {
                RenderedBlock::ImagePlaceholder {
                    label: format!("Image: {}", alt),
                    rows: 1,
                }
            }
        }
        Err(err_msg) => {
            let error_style = Style::default().fg(Color::Red);
            RenderedBlock::Fallback(vec![
                Line::from(vec![
                    Span::styled(" ⚠ Image error:", error_style),
                    Span::styled(err_msg.to_string(), error_style),
                ]),
                Line::from(Span::styled(
                    format!("  src {}", url),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        }
    }
}

fn render_mermaid_block(
    source: &str,
    _theme: &Theme,
    cache: Option<&mut AssetCache>,
    caps: Option<&TermCaps>,
) -> RenderedBlock {
    let (Some(cache), Some(caps)) = (cache, caps) else {
        return RenderedBlock::ImagePlaceholder {
            label: format!(
                "Mermaid: {} ",
                &source[..source.len().min(50)].replace('\n', " ")
            ),
            rows: 1,
        };
    };

    match cache.get_or_render_mermaid(source, caps.content_width_px()) {
        Ok((png, w, h)) => {
            if caps.has_kitty {
                return RenderedBlock::InlineImage {
                    png_data: png.clone(),
                    width_px: *w,
                    height_px: *h,
                    rows: caps.image_rows(*h),
                    label: "Mermaid Diagram".to_string(),
                };
            } else {
                RenderedBlock::ImagePlaceholder {
                    label: "Mermaid diagram".to_string(),
                    rows: 1,
                }
            }
        }
        Err(err_msg) => {
            let error_style = Style::default().fg(Color::Red);
            RenderedBlock::Fallback(vec![
                Line::from(vec![
                    Span::styled(" ⚠ Mermaid error:", error_style),
                    Span::styled(err_msg.to_string(), error_style),
                ]),
                Line::from(Span::styled(
                    format!("  src {}", source),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        }
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
