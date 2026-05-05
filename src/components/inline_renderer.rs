use iocraft::prelude::*;
use crate::document::model::{Inline, inlines_to_text};

/// Renders a list of inlines into a Vec of (text, color, bold, italic) tuples for styling
pub fn render_inlines_styled(
    inlines: &[Inline],
    base_color: Color,
) -> Vec<(String, Color, bool, bool)> {
    let mut spans = Vec::new();
    render_inlines_recursive(inlines, base_color, false, false, &mut spans);
    spans
}

fn render_inlines_recursive(
    inlines: &[Inline],
    color: Color,
    bold: bool,
    italic: bool,
    out: &mut Vec<(String, Color, bool, bool)>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(t) => {
                out.push((t.clone(), color, bold, italic));
            }
            Inline::Bold(ch) => {
                render_inlines_recursive(ch, color, true, italic, out);
            }
            Inline::Italic(ch) => {
                render_inlines_recursive(ch, color, bold, true, out);
            }
            Inline::Strikethrough(ch) => {
                render_inlines_recursive(ch, color, bold, italic, out);
            }
            Inline::Code(c) => {
                out.push((format!(" {} ", c), Color::Green, bold, italic));
            }
            Inline::Link { text, url, .. } => {
                let label = inlines_to_text(text);
                out.push((format!("{} ({})", label, url), Color::Blue, bold, italic));
            }
            Inline::SoftBreak => {
                out.push((" ".to_string(), color, bold, italic));
            }
            Inline::HardBreak => {
                out.push(("\n".to_string(), color, bold, italic));
            }
            Inline::Math(m) => {
                let cleaned = m.replace('\r', "").replace('\n', " ");
                out.push((format!("${cleaned}$"), color, true, italic));
            }
            Inline::Image { alt, .. } => {
                out.push((format!("[{}]", alt), Color::Yellow, bold, italic));
            }
        }
    }
}

/// Create iocraft text elements from styled inlines
pub fn create_styled_text_elements(
    styled_inlines: &[(String, Color, bool, bool)],
) -> Vec<AnyElement<'static>> {
    styled_inlines
        .iter()
        .map(|(text, color, bold, _italic)| {
            // Note: iocraft may not support italic in Text component, but bold can be approximated
            // by using higher contrast color or separate styling if available
            element! {
                Text(content: text.clone(), color: *color)
            }
            .into_any()
        })
        .collect()
}
