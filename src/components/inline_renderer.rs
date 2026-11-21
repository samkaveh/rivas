use crate::assets::math::{MathMode, math_mode, scripts_to_unicode};
use crate::components::image::KittyImage;
use crate::components::math_block::{KittyMath, UnicodeMath};
use crate::components::scroll::Viewport;
use crate::document::model::{Inline, inlines_to_text};
use crate::theme;
use iocraft::prelude::*;
use std::path::PathBuf;

/// Text styling inherited while rendering inline content.
#[derive(Clone, Copy)]
struct TextStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

impl TextStyle {
    fn plain() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
        }
    }
}

/// Combines each non-whitespace character with U+0336 (COMBINING LONG STROKE
/// OVERLAY) so the text renders struck-through in the terminal. Terminals
/// support strike-through only rarely, so like sub/superscripts we fall back
/// to a Unicode glyph overlay instead of an ANSI attribute.
fn strike_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        out.push(c);
        if !c.is_whitespace() {
            out.push('\u{0336}');
        }
    }
    out
}

/// One styled piece of inline content, before it is grouped into words.
enum Piece {
    /// A run of text in one style (may contain spaces).
    Text {
        text: String,
        color: Color,
        style: TextStyle,
        link: Option<String>,
    },
    /// Inline code: rendered as a padded chip on a tinted background.
    Code(String),
    /// An atomic element (inline math, image).
    Element(AnyElement<'static>),
    /// A hard line break.
    Break,
}

/// Flattens nested inlines into a linear list of styled pieces.
fn collect_pieces(
    inlines: &[Inline],
    color: Color,
    style: TextStyle,
    link: Option<&str>,
    file_path: Option<&PathBuf>,
    viewport_height: Option<u32>,
    viewport_width: Option<u32>,
    out: &mut Vec<Piece>,
) {
    let text_piece = |text: String, color: Color, style: TextStyle, link: Option<&str>| Piece::Text {
        text,
        color,
        style,
        link: link.map(str::to_string),
    };
    for inline in inlines {
        match inline {
            Inline::Text(t) => out.push(text_piece(t.clone(), color, style, link)),
            Inline::SoftBreak => out.push(text_piece(" ".into(), color, style, link)),
            Inline::HardBreak => out.push(Piece::Break),
            Inline::Bold(ch) => collect_pieces(ch, color, TextStyle { bold: true, ..style }, link, file_path, viewport_height, viewport_width, out),
            Inline::Italic(ch) => collect_pieces(ch, color, TextStyle { italic: true, ..style }, link, file_path, viewport_height, viewport_width, out),
            Inline::Strikethrough(ch) => collect_pieces(ch, color, TextStyle { strikethrough: true, ..style }, link, file_path, viewport_height, viewport_width, out),
            Inline::Underline(ch) => collect_pieces(ch, color, TextStyle { underline: true, ..style }, link, file_path, viewport_height, viewport_width, out),
            Inline::Subscript(ch) => out.push(text_piece(scripts_to_unicode(&inlines_to_text(ch), false), color, style, link)),
            Inline::Superscript(ch) => out.push(text_piece(scripts_to_unicode(&inlines_to_text(ch), true), color, style, link)),
            Inline::Code(c) => out.push(Piece::Code(c.clone())),
            Inline::Link { text, url } => {
                let link_style = TextStyle { underline: true, ..style };
                if inlines_to_text(text).trim().is_empty() {
                    // Links with no visible label fall back to the URL itself
                    // so `[](target)` never disappears entirely.
                    out.push(text_piece(url.clone(), theme::blue(), link_style, Some(url)));
                } else {
                    collect_pieces(text, theme::blue(), link_style, Some(url), file_path, viewport_height, viewport_width, out);
                }
            }
            Inline::FootnoteRef(label) => {
                // Numeric labels render as Unicode superscripts (e.g. `text¹`);
                // anything else keeps the readable `[^label]` form.
                let content = if !label.is_empty() && label.chars().all(|c| c.is_ascii_digit()) {
                    scripts_to_unicode(label, true)
                } else {
                    format!("[^{}]", label)
                };
                out.push(text_piece(content, theme::blue(), TextStyle::plain(), None));
            }
            Inline::Math(m) => {
                let el = if math_mode() == MathMode::Image {
                    let vp = viewport_height.map(|h| Viewport {
                        height: Some(h),
                        width: viewport_width,
                        scroll_offset: None,
                    });
                    element! { KittyMath(content: m.clone(), display: false, viewport: vp) }.into_any()
                } else {
                    element! { UnicodeMath(content: m.clone(), display: false) }.into_any()
                };
                out.push(Piece::Element(el));
            }
            Inline::Image { alt: _, url } => {
                let vp = viewport_height.map(|h| Viewport {
                    height: Some(h),
                    width: viewport_width,
                    scroll_offset: None,
                });
                out.push(Piece::Element(element! {
                    KittyImage(url: url.clone(), file_path: file_path.cloned().unwrap_or_default(), viewport: vp)
                }.into_any()));
            }
        }
    }
}

/// Groups pieces into unbreakable "words": a word ends after a run of
/// whitespace, so `**bold**,` or `` `code`. `` stay together while the line
/// may wrap at any space. Each group becomes one flex item.
fn group_words(pieces: Vec<Piece>) -> Vec<Vec<Piece>> {
    let mut groups: Vec<Vec<Piece>> = Vec::new();
    let mut cur: Vec<Piece> = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Text { text, color, style, link } => {
                let mut buf = String::new();
                let mut in_ws = false;
                for ch in text.chars() {
                    if ch.is_whitespace() {
                        // Collapse whitespace runs (incl. newlines) to one space.
                        if !in_ws {
                            buf.push(' ');
                        }
                        in_ws = true;
                    } else {
                        if in_ws {
                            cur.push(Piece::Text { text: std::mem::take(&mut buf), color, style, link: link.clone() });
                            groups.push(std::mem::take(&mut cur));
                            in_ws = false;
                        }
                        buf.push(ch);
                    }
                }
                if !buf.is_empty() {
                    // A run that is only whitespace attaches to the previous
                    // word (e.g. the space after a code chip).
                    cur.push(Piece::Text { text: buf, color, style, link: link.clone() });
                    if in_ws {
                        groups.push(std::mem::take(&mut cur));
                    }
                }
            }
            Piece::Break => {
                if !cur.is_empty() {
                    groups.push(std::mem::take(&mut cur));
                }
                groups.push(vec![Piece::Break]);
            }
            other => cur.push(other),
        }
    }
    if !cur.is_empty() {
        groups.push(cur);
    }
    groups
}

fn render_piece(piece: Piece) -> AnyElement<'static> {
    match piece {
        Piece::Text { text, color, style, link } => {
            let content = if style.strikethrough { strike_text(&text) } else { text };
            element! {
                Text(
                    content: content,
                    color: color,
                    weight: if style.bold { Weight::Bold } else { Weight::Normal },
                    italic: style.italic,
                    decoration: if style.underline { TextDecoration::Underline } else { TextDecoration::None },
                    hyperlink: link,
                )
            }
            .into_any()
        }
        Piece::Code(code) => {
            let content = match code.strip_suffix('\u{0}') {
                Some(tight) => format!(" {tight}"),
                None => format!(" {code} "),
            };
            element! {
                View(background_color: theme::surface(), flex_shrink: 1.0) {
                    Text(content: content, color: theme::green())
                }
            }
            .into_any()
        }
        Piece::Element(el) => el,
        // A zero-width text never forces a flex wrap, so a hard break is a
        // full-width empty item: the next word starts a new line.
        Piece::Break => element! { View(width: 100pct) {} }.into_any(),
    }
}

/// Renders a list of inlines into flex items for a wrapping row container.
///
/// Each item is one word group (see [`group_words`]), so the surrounding
/// `View(flex_wrap: Wrap)` breaks lines only between words, like a text
/// layout engine would, while code chips and math keep their own styling.
pub fn render_inlines(
    inlines: &[Inline],
    base_color: Color,
    bold: bool,
    file_path: Option<&PathBuf>,
    viewport_height: Option<u32>,
    viewport_width: Option<u32>,
) -> Vec<AnyElement<'static>> {
    let mut pieces = Vec::new();
    collect_pieces(
        inlines,
        base_color,
        TextStyle { bold, ..TextStyle::plain() },
        None,
        file_path,
        viewport_height,
        viewport_width,
        &mut pieces,
    );
    // Leading whitespace (e.g. from a soft break at the start) never renders.
    if let Some(Piece::Text { text, .. }) = pieces.first_mut() {
        let trimmed = text.trim_start().to_string();
        *text = trimmed;
    }
    group_words(pieces).into_iter().map(render_group).collect()
}

/// Converts a text piece into a styled [`MixedTextContent`] run.
fn text_content(text: String, color: Color, style: TextStyle, link: Option<String>) -> MixedTextContent {
    let content = if style.strikethrough { strike_text(&text) } else { text };
    let mut c = MixedTextContent::new(content)
        .color(color)
        .weight(if style.bold { Weight::Bold } else { Weight::Normal })
        .decoration(if style.underline { TextDecoration::Underline } else { TextDecoration::None });
    if style.italic {
        c = c.italic();
    }
    if let Some(url) = link {
        c = c.hyperlink(url);
    }
    c
}

/// Renders one word group as a single flex item.
///
/// Text-only groups (the common case, e.g. `**bold**,`) become one
/// [`MixedText`] node; groups containing a code chip or math use a
/// non-wrapping row so the pieces stay together.
fn render_group(group: Vec<Piece>) -> AnyElement<'static> {
    if group.iter().all(|p| matches!(p, Piece::Text { .. })) {
        let contents: Vec<MixedTextContent> = group
            .into_iter()
            .filter_map(|p| match p {
                Piece::Text { text, color, style, link } => Some(text_content(text, color, style, link)),
                _ => None,
            })
            .collect();
        return element! { MixedText(contents: contents, wrap: TextWrap::NoWrap) }.into_any();
    }
    if group.len() == 1 {
        return render_piece(group.into_iter().next().unwrap());
    }
    // A chip directly followed by punctuation drops its trailing pad so it
    // reads `code,` rather than `code ,`.
    let mut pieces: Vec<Piece> = group;
    for i in 0..pieces.len().saturating_sub(1) {
        let next_is_punct = matches!(&pieces[i + 1], Piece::Text { text, .. }
            if text.starts_with([',', '.', ';', ':', '!', '?', ')', ']', '}', '\'', '"']));
        if next_is_punct {
            if let Piece::Code(c) = &mut pieces[i] {
                c.push('\u{0}');
            }
        }
    }
    element! {
        View(flex_direction: FlexDirection::Row, flex_shrink: 0.0) {
            #(pieces.into_iter().map(render_piece))
        }
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strike_text_overlays_every_non_whitespace_char() {
        assert_eq!(strike_text("abc"), "a\u{0336}b\u{0336}c\u{0336}");
        assert_eq!(strike_text("a b"), "a\u{0336} b\u{0336}");
        assert_eq!(strike_text(" "), " ");
    }

    #[test]
    fn strike_text_preserves_display_width() {
        let plain = "% a b c";
        let struck = strike_text(plain);
        assert_eq!(
            unicode_width::UnicodeWidthStr::width(struck.as_str()),
            unicode_width::UnicodeWidthStr::width(plain),
            "combining overlay must stay width zero so layout is unaffected"
        );
    }
}
