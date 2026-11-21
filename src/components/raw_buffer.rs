use iocraft::prelude::*;

use crate::theme;

/// How the editing cursor is drawn inside a raw (source) block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorStyle {
    /// Normal / Visual mode: the character under the cursor is inverted.
    #[default]
    Block,
    /// Insert mode: the character under the cursor sits on an accent-colored
    /// cell, so the insertion point is unmistakable without shifting text.
    Insert,
    /// A pending operator (`d`, `c`, `y`…): an underscore placeholder.
    Operator,
}

/// Raw buffer + cursor info used to render a block's source while editing.
#[derive(Clone, Default)]
pub struct RawState {
    /// The raw source text of the block (`span.0..span.1`).
    pub text: String,
    /// Zero-indexed line within the block the cursor is on (`None` when the
    /// cursor is outside this block, e.g. a block that is only selected).
    pub cursor_line: Option<usize>,
    /// Byte offset of the cursor within that line.
    pub cursor_col: usize,
    /// How to draw the cursor.
    pub cursor_style: CursorStyle,
    /// Visual-mode selection as an inclusive byte range relative to `text`.
    pub selection: Option<(usize, usize)>,
    /// Optional per-line syntax-highlighted spans (byte-aligned with `text`).
    pub highlight: Option<Vec<Vec<(String, Color)>>>,
}

/// Props for the [`RawBuffer`] component.
#[derive(Default, Props)]
pub struct RawBufferProps {
    /// Raw buffer + cursor state. When `None`, renders nothing.
    pub raw: Option<RawState>,
    /// Foreground color for the raw text (used when highlighting is absent).
    pub color: Option<Color>,
}

/// Per-character styling class used to group a line into styled runs.
#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Plain(Color),
    Selected,
    Cursor(Color),
}

/// Builds the styled segments of one logical source line.
///
/// `line_start` is the byte offset of the line within the block text, so the
/// (block-relative) selection range can be intersected with it. The cursor
/// (if on this line) is a style-only change: it never inserts or removes
/// characters, so the wrapped width of the line never depends on where the
/// cursor sits — the text does not jitter while you type or move.
fn build_line_contents(
    spans: &[(String, Color)],
    line_start: usize,
    cursor_col: Option<usize>,
    style: CursorStyle,
    selection: Option<(usize, usize)>,
    fg: Color,
) -> Vec<MixedTextContent> {
    let cursor_color = |token: Color| match style {
        CursorStyle::Insert => theme::green(),
        _ => token,
    };

    let mut runs: Vec<(Cell, String)> = Vec::new();
    let mut push = |cell: Cell, s: &str| match runs.last_mut() {
        Some((c, text)) if *c == cell && !matches!(cell, Cell::Cursor(_)) => text.push_str(s),
        _ => runs.push((cell, s.to_string())),
    };

    let mut pos = 0usize; // byte offset within the line
    let mut cursor_drawn = false;
    for (text, color) in spans {
        for ch in text.chars() {
            let abs = line_start + pos;
            let mut buf = [0u8; 4];
            let s: &str = ch.encode_utf8(&mut buf);
            if Some(pos) == cursor_col && !cursor_drawn {
                cursor_drawn = true;
                let glyph = if style == CursorStyle::Operator { "_" } else { s };
                push(Cell::Cursor(cursor_color(*color)), glyph);
            } else if selection.is_some_and(|(a, b)| abs >= a && abs <= b) {
                push(Cell::Selected, s);
            } else {
                push(Cell::Plain(*color), s);
            }
            pos += ch.len_utf8();
        }
    }
    // Cursor past the end of the line (or on an empty line): draw a trailing
    // cell so the position is always visible.
    if let Some(col) = cursor_col {
        if !cursor_drawn && col >= pos {
            push(Cell::Cursor(cursor_color(fg)), " ");
        }
    }

    runs.into_iter()
        .map(|(cell, text)| match cell {
            Cell::Plain(c) => MixedTextContent::new(text).color(c),
            Cell::Selected => MixedTextContent::new(text).color(theme::magenta()).invert(),
            Cell::Cursor(c) => MixedTextContent::new(text)
                .color(c)
                .weight(Weight::Bold)
                .invert(),
        })
        .collect()
}

/// Renders raw source lines with syntax highlighting, the editing cursor and
/// any visual selection.
///
/// Each logical line is a single [`MixedText`], so wrapping is measured over
/// the full line. Normal, Insert and Visual mode all render through this one
/// component, so switching modes changes only the cursor color — never the
/// block's layout or highlighting.
#[component]
pub fn RawBuffer(props: &RawBufferProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let fg = props.color.unwrap_or(theme::fg());

    let lines = props.raw.clone().map(|raw| {
        let highlight = raw.highlight.clone();
        let mut line_start = 0usize;
        raw.text
            .split('\n')
            .enumerate()
            .map(|(idx, line)| {
                let mut spans: Vec<(String, Color)> = highlight
                    .as_ref()
                    .and_then(|h| h.get(idx).cloned())
                    .unwrap_or_else(|| vec![(line.to_string(), fg)]);
                // Highlighters may drop or alter text (e.g. trailing
                // whitespace); fall back to the plain line so cursor and
                // selection offsets always line up with the real bytes.
                if spans.iter().map(|(t, _)| t.len()).sum::<usize>() != line.len() {
                    spans = vec![(line.to_string(), fg)];
                }
                let cursor_col = (Some(idx) == raw.cursor_line).then_some(raw.cursor_col);
                let contents = build_line_contents(
                    &spans,
                    line_start,
                    cursor_col,
                    raw.cursor_style,
                    raw.selection,
                    fg,
                );
                line_start += line.len() + 1;
                element! {
                    MixedText(contents: contents, wrap: TextWrap::Wrap)
                }
                .into_any()
            })
            .collect::<Vec<_>>()
    });

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(lines.into_iter().flatten())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(parts: &[MixedTextContent]) -> Vec<String> {
        parts.iter().map(|p| p.text.clone()).collect()
    }

    #[test]
    fn cursor_splits_line_without_changing_text() {
        let spans = vec![("hello".to_string(), theme::fg())];
        let parts = build_line_contents(&spans, 0, Some(2), CursorStyle::Block, None, theme::fg());
        assert_eq!(texts(&parts), vec!["he", "l", "lo"]);
        assert!(parts[1].invert);
    }

    #[test]
    fn cursor_past_end_appends_cell() {
        let spans = vec![("hi".to_string(), theme::fg())];
        let parts = build_line_contents(&spans, 0, Some(2), CursorStyle::Insert, None, theme::fg());
        assert_eq!(texts(&parts), vec!["hi", " "]);
    }

    #[test]
    fn selection_is_inclusive_and_offset_by_line_start() {
        let spans = vec![("abcdef".to_string(), theme::fg())];
        // Line starts at byte 10 of the block; select bytes 11..=12 → "bc".
        let parts = build_line_contents(&spans, 10, None, CursorStyle::Block, Some((11, 12)), theme::fg());
        assert_eq!(texts(&parts), vec!["a", "bc", "def"]);
        assert!(parts[1].invert);
    }

    #[test]
    fn operator_cursor_draws_underscore() {
        let spans = vec![("ab".to_string(), theme::fg())];
        let parts = build_line_contents(&spans, 0, Some(0), CursorStyle::Operator, None, theme::fg());
        assert_eq!(texts(&parts), vec!["_", "b"]);
    }
}
