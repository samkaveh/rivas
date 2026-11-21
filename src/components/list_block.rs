use crate::components::blocks_renderer::BlocksRenderer;
use crate::components::highlight::{MARKDOWN, UseHighlight};
use crate::components::inline_renderer::render_inlines;
use crate::components::raw_buffer::{RawBuffer, RawState};
use crate::components::scroll::Viewport;
use crate::document::model::{Block, ListItem};
use crate::theme;
use iocraft::prelude::*;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

/// Properties for the [`ListBlock`] component.
#[derive(Default, Props)]
pub struct ListBlockProps {
    /// `true` for ordered lists (numbered), `false` for bullet lists.
    pub ordered: bool,
    /// Starting number for ordered lists (defaults to 1).
    pub start: Option<u64>,
    /// The list items to render.
    pub items: Vec<ListItem>,
    /// File path for link resolution in nested content.
    pub file_path: PathBuf,
    /// Optional viewport dimensions for responsive rendering.
    pub viewport: Option<Viewport>,
    /// Optional raw buffer + cursor for the Normal-mode source view.
    pub raw: Option<RawState>,
    /// Nesting depth (0 = top-level list); picks the bullet glyph.
    pub depth: u32,
}

/// Bullet glyph for a nesting depth: • → ◦ → ▪, then repeating.
fn bullet(depth: u32) -> &'static str {
    match depth % 3 {
        0 => "•",
        1 => "◦",
        _ => "▪",
    }
}

/// Renders a bullet, ordered or task list.
///
/// Items stack tightly (no blank rows between them) next to a fixed marker
/// column that never shrinks, so wrapped item text always hangs under its
/// first word. Ordered numbers are right-aligned to the widest number, nested
/// lists step through bullet shapes, and completed tasks are dimmed.
#[component]
pub fn ListBlock(props: &ListBlockProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let source = props.raw.as_ref().map(|r| r.text.as_str()).unwrap_or("");
    let highlighted = hooks.use_cached_highlight(source, MARKDOWN, theme::fg());

    if let Some(mut raw) = props.raw.clone() {
        raw.highlight = Some(highlighted);
        return element! {
            View(background_color: theme::dark_bg()) {
                RawBuffer(raw: raw, color: theme::fg())
            }
        }
        .into_any();
    }

    let start = props.start.unwrap_or(1);
    let has_tasks = props.items.iter().any(|i| i.checked.is_some());
    // Marker column: widest marker + one space of separation.
    let marker_w = if props.ordered {
        let last = start + props.items.len().saturating_sub(1) as u64;
        format!("{last}.").width() as u32 + 1
    } else {
        2
    };
    let marker_w = if has_tasks { marker_w.max(2) } else { marker_w };

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(props.items.iter().enumerate().map(|(n, item)| {
                let (marker, marker_color) = match item.checked {
                    Some(true) => ("☑".to_string(), theme::green()),
                    Some(false) => ("☐".to_string(), theme::muted()),
                    None if props.ordered => {
                        let label = format!("{}.", start + n as u64);
                        let pad = (marker_w as usize - 1).saturating_sub(label.width());
                        (format!("{}{}", " ".repeat(pad), label), theme::blue())
                    }
                    None => (bullet(props.depth).to_string(), theme::blue()),
                };
                let text_color = if item.checked == Some(true) { theme::muted() } else { theme::fg() };
                element! {
                    View(flex_direction: FlexDirection::Row) {
                        View(width: marker_w, flex_shrink: 0.0) {
                            Text(content: marker, color: marker_color)
                        }
                        View(flex_grow: 1.0, flex_shrink: 1.0, flex_direction: FlexDirection::Column) {
                            #(render_item_blocks(&item.content, text_color, props.depth, &props.file_path, &props.viewport))
                        }
                    }
                }
                .into_any()
            }))
        }
    }
    .into_any()
}

/// Renders the blocks inside one list item, stacked with no gaps.
///
/// Paragraphs are laid out inline (no block wrapper, so they hug the marker),
/// nested lists recurse with `depth + 1`, and anything else (code, quotes,
/// tables…) goes through a tight [`BlocksRenderer`].
fn render_item_blocks(
    blocks: &[Block],
    color: Color,
    depth: u32,
    file_path: &PathBuf,
    viewport: &Option<Viewport>,
) -> Vec<AnyElement<'static>> {
    blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph { content, .. } => {
                let inlines = render_inlines(
                    content,
                    color,
                    false,
                    Some(file_path),
                    viewport.as_ref().and_then(|v| v.height),
                    viewport.as_ref().and_then(|v| v.width),
                );
                element! {
                    View(flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap) {
                        #(inlines)
                    }
                }
                .into_any()
            }
            Block::List { ordered, start, items, .. } => element! {
                ListBlock(
                    ordered: *ordered,
                    start: *start,
                    items: items.clone(),
                    file_path: file_path.clone(),
                    viewport: viewport.clone(),
                    depth: depth + 1,
                )
            }
            .into_any(),
            other => element! {
                BlocksRenderer(
                    blocks: vec![other.clone()],
                    file_path: file_path.clone(),
                    viewport: viewport.clone(),
                    tight: true,
                )
            }
            .into_any(),
        })
        .collect()
}
