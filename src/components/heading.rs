use crate::components::highlight::{MARKDOWN, UseHighlight};
use crate::components::inline_renderer::render_inlines;
use crate::components::raw_buffer::{RawBuffer, RawState};
use crate::components::scroll::Viewport;
use crate::document::model::Inline;
use crate::theme;
use iocraft::prelude::*;
use std::path::PathBuf;

/// Properties for the [`Heading`] component.
#[derive(Default, Props)]
pub struct HeadingProps {
    /// Heading level (1–6).
    pub level: u8,
    /// Inline content to render as the heading text.
    pub content: Vec<Inline>,
    /// Path to the file being rendered (for link resolution).
    pub file_path: PathBuf,
    /// Optional viewport dimensions for responsive rendering.
    pub viewport: Option<Viewport>,
    /// Optional raw buffer + cursor for the source view.
    pub raw: Option<RawState>,
}

/// Text color for each heading level.
///
/// A terminal can't change font size, so hierarchy comes from (in order of
/// strength) a rule under the heading, color, and weight: `#`/`##` share the
/// accent color and are told apart by their rule; `###` uses the secondary
/// accent; deeper levels step down to body text and then muted text.
pub fn heading_color(level: u8) -> Color {
    match level {
        1 | 2 => theme::blue(),
        3 => theme::cyan(),
        4 => theme::fg(),
        _ => theme::comment(),
    }
}

/// Renders an ATX heading.
///
/// `#` gets a heavy rule beneath it and `##` a light one, both spanning the
/// content column. The source view keeps the same rule so entering a heading
/// to edit it never changes its height.
#[component]
pub fn Heading(props: &HeadingProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let color = heading_color(props.level);
    let source = props.raw.as_ref().map(|r| r.text.as_str()).unwrap_or("");
    let highlighted = hooks.use_cached_highlight(source, MARKDOWN, color);

    // `#` gets a heavy rule, `##` a light one. Rules are drawn as a bottom
    // border rather than a run of box-drawing text: a border always spans the
    // exact container width and needs no text measurement (a long text rule
    // gets measured at narrow widths during layout and inflates the height).
    let rule = match props.level {
        1 => Some((BorderStyle::Bold, theme::dark_grey())),
        2 => Some((BorderStyle::Single, theme::border())),
        _ => None,
    };

    let body = if let Some(mut raw) = props.raw.clone() {
        raw.highlight = Some(highlighted);
        element! {
            View(background_color: theme::dark_bg()) {
                RawBuffer(raw: raw, color: color)
            }
        }
        .into_any()
    } else {
        let styled_elements = render_inlines(
            &props.content,
            color,
            true,
            Some(&props.file_path),
            props.viewport.as_ref().and_then(|v| v.height),
            props.viewport.as_ref().and_then(|v| v.width),
        );
        element! {
            View(flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
                #(styled_elements)
            }
        }
        .into_any()
    };

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(Some(body))
            #(rule.map(|(style, color)| element! {
                View(height: 1, border_style: style, border_edges: Edges::Bottom, border_color: color) {}
            }.into_any()))
        }
    }
}
