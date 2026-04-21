use crate::document::model::*;
use crate::render::theme::Theme;
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, Weight};

pub struct LayoutBlock {
    pub y: f32,
    pub height: f32,
    pub content: LayoutContent,
}

pub enum LayoutContent {
    /// Rich text rendered via cosmic_text
    Text { buffer: Buffer, x_offset: f32 },
    /// Code block: text buffer + background rect
    Code {
        buffer: Buffer,
        bg_x: f32,
        bg_y: f32,
        bg_w: f32,
        bg_h: f32,
    },
    /// Horizontal rule
    Rule,
    /// Blockquote: bar + child layout blocks
    BlockQuote {
        bar_x: f32,
        children: Vec<LayoutBlock>,
    },
    /// Placeholder for images/mermaid/math
    Placeholder { buffer: Buffer },
    /// Pre-rendered pixmap
    Pixmap {
        pixmap: tiny_skia::Pixmap,
        x_offset: f32,
    },
}

pub struct LayoutEngine<'a> {
    font_system: &'a mut FontSystem,
    theme: &'a Theme,
    content_width: f32,
    padding: f32,
}

impl<'a> LayoutEngine<'a> {
    pub fn new(font_system: &'a mut FontSystem, theme: &'a Theme, viewport_width: f32) -> Self {
        let content_width = (viewport_width - 2.0 * theme.padding).min(theme.max_content_width);
        Self {
            font_system,
            theme,
            content_width,
            padding: theme.padding,
        }
    }

    pub fn layout_all(&mut self, blocks: &[Block]) -> Vec<LayoutBlock> {
        let mut result = Vec::new();
        let mut y = self.theme.padding;
        for block in blocks {
            let lb = self.layout_block(block.clone(), y);
            y += lb.height + self.theme.block_spacing;
            result.push(lb);
        }
        result
    }

    fn layout_block(&mut self, block: Block, y: f32) -> LayoutBlock {
        match block {
            Block::Heading { level, content, .. } => {
                let metrics = self.theme.heading_metrics(level);
                let attrs = Attrs::new()
                    .family(Family::SansSerif)
                    .weight(Weight::BOLD)
                    .color(self.theme.heading.to_cosmic());
                let buffer = self.shape_inlines(&content, metrics, attrs, self.content_width);
                let height = self.buffer_height(&buffer);
                LayoutBlock {
                    y,
                    height,
                    content: LayoutContent::Text {
                        buffer,
                        x_offset: self.padding,
                    },
                }
            }

            _ => LayoutBlock {
                y,
                height: 0.0,
                content: LayoutContent::Rule,
            },
        }
    }

    fn buffer_height(&self, buffer: &Buffer) -> f32 {
        let height = buffer
            .layout_runs()
            .last()
            .map(|run| run.line_top + run.line_height)
            .unwrap_or(0.0);
        height
    }

    /// Shape a paragraph of rich inlines into cosmic_text Buffer
    fn shape_inlines(
        &mut self,
        inlines: &[Inline],
        metrics: Metrics,
        default_attrs: Attrs,
        width: f32,
    ) -> Buffer {
        // Flatten inlines into (text, attrs) spans
        let mut spans: Vec<(String, Attrs)> = Vec::new();
        self.flatten_inlines(inlines, default_attrs.clone(), &mut spans);

        let mut buffer = Buffer::new(self.font_system, metrics);
        buffer.set_size(self.font_system, Some(width), None);
        let rich = spans.iter().map(|f| (f.0.as_str(), f.1.clone()));
        buffer.set_rich_text(
            self.font_system,
            rich,
            &default_attrs.clone(),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(self.font_system, false);
        buffer
    }

    /// Flatten inline tree into a flat list of (text, attrs) spans
    fn flatten_inlines<'b>(
        &self,
        inlines: &[Inline],
        attrs: Attrs<'b>,
        out: &mut Vec<(String, Attrs<'b>)>,
    ) {
        for inline in inlines {
            match inline {
                Inline::Text(t) => out.push((t.clone(), attrs.clone())),
                Inline::Bold(ch) => {
                    self.flatten_inlines(ch, attrs.clone().weight(Weight::BOLD), out)
                }
                Inline::Italic(ch) => {
                    self.flatten_inlines(ch, attrs.clone().style(Style::Italic), out)
                }
                // TODO: text decorations
                Inline::Strikethrough(ch) => self.flatten_inlines(ch, attrs.clone(), out),
                Inline::Code(c) => out.push((
                    c.clone(),
                    attrs
                        .clone()
                        .family(Family::Monospace)
                        .color(self.theme.code_text.to_cosmic()),
                )),
                Inline::Math(m) => out.push((
                    m.clone(),
                    attrs
                        .clone()
                        .family(Family::Monospace)
                        .style(Style::Italic)
                        .color(self.theme.code_text.to_cosmic()),
                )),
                Inline::Link { text, .. } => {
                    self.flatten_inlines(
                        text,
                        attrs.clone().color(self.theme.link.to_cosmic()),
                        out,
                    );
                }
                Inline::SoftBreak => out.push((" ".into(), attrs.clone())),
                Inline::HardBreak => out.push(("\n".into(), attrs.clone())),
                Inline::Image { alt, .. } => out.push((format!("[{}]", alt), attrs.clone())),
            }
        }
    }

    fn shape_plane_text(&mut self, text: &str, metrics: Metrics, attrs: Attrs) -> Buffer {
        let mut buffer = Buffer::new(self.font_system, metrics);
        buffer.set_size(self.font_system, Some(self.content_width), None);
        buffer.set_rich_text(
            self.font_system,
            [(text, attrs.clone())],
            &attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(self.font_system, false);

        buffer
    }

    // fn layout_code_block(&mut self, lang: Option<&str>, code: &str, y: f32) -> LayoutBlock {
    //     let metrics = self.theme.code_metrics();
    //     let base_attrs = Attrs::new()
    //         .family(Family::Monospace)
    //         .color(self.theme.code_text.to_cosmic());
    //
    //     // Highlight with syntect if language is known
    //     let spans = crate::render::code::highlight(code, lang, base_attrs, &self.theme);
    //
    //     let inner_width = self.content_width = 2.0 * self.theme.code_padding;
    // }
}
