use std::sync::LazyLock;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use syntect::{easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet};

use crate::render::{text::RenderedBlock, theme::Theme};

static SS: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static TS: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

pub fn render_code_block(lang: Option<&str>, code: &str, theme: &Theme) -> RenderedBlock {
    let syntax = lang
        .and_then(|l| SS.find_syntax_by_token(l))
        .unwrap_or_else(|| SS.find_syntax_plain_text());
    let st = &TS.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, st);

    let mut lines: Vec<Line<'static>> = Vec::new();

    for line in code.lines() {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::raw(" "));

        match h.highlight_line(line, &SS) {
            Ok(regions) => {
                for (style, text) in regions {
                    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                    let mut s = Style::default().fg(fg).bg(theme.code_block_bg);
                    if style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::BOLD)
                    {
                        s = s.add_modifier(Modifier::BOLD);
                    }
                    if style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::ITALIC)
                    {
                        s = s.add_modifier(Modifier::ITALIC);
                    }
                    spans.push(Span::styled(text.to_string(), s));
                }
            }
            Err(_) => {
                spans.push(Span::styled(
                    line.to_string(),
                    theme.code_block_text.bg(theme.code_block_bg),
                ));
            }
        }
        lines.push(Line::from(spans));
    }

    RenderedBlock::Lines(lines)
}
