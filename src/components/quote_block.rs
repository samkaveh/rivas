use crate::components::blocks_renderer::BlocksRenderer;
use crate::components::highlight::{MARKDOWN, UseHighlight};
use crate::components::raw_buffer::{RawBuffer, RawState};
use crate::components::scroll::Viewport;
use crate::document::model::Block;
use crate::theme;
use iocraft::prelude::*;
use std::path::PathBuf;

/// Properties for the [`QuoteBlock`] component.
#[derive(Default, Props)]
pub struct QuoteBlockProps {
    /// Child blocks inside the blockquote.
    pub children: Vec<Block>,
    /// Optional file path for link resolution in nested content.
    pub file_path: Option<PathBuf>,
    /// Optional viewport dimensions for responsive rendering.
    pub viewport: Option<Viewport>,
    /// Optional raw buffer + cursor for the Normal-mode source view.
    pub raw: Option<RawState>,
}

/// A left-edge-only border drawn with `▎`, used for block quotes so the bar
/// spans the full height of the quote (however many lines it wraps to).
pub fn quote_bar() -> BorderStyle {
    BorderStyle::Custom(BorderCharacters {
        top_left: ' ',
        top_right: ' ',
        bottom_left: ' ',
        bottom_right: ' ',
        left: '▎',
        right: ' ',
        top: ' ',
        bottom: ' ',
    })
}

/// Renders a blockquote: a full-height accent bar and the quoted blocks set
/// in a softer color, with no background box (quotes recede; code advances).
#[component]
pub fn QuoteBlock(props: &QuoteBlockProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let file_path = props.file_path.clone().unwrap_or_default();
    let source = props.raw.as_ref().map(|r| r.text.as_str()).unwrap_or("");
    let highlighted = hooks.use_cached_highlight(source, MARKDOWN, theme::fg());
    if let Some(mut raw) = props.raw.clone() {
        raw.highlight = Some(highlighted);
        return element! {
            View(background_color: theme::dark_bg()) {
                RawBuffer(raw: raw, color: theme::fg())
            }
        };
    }
    element! {
        View(
            flex_direction: FlexDirection::Column,
            border_style: quote_bar(),
            border_edges: Edges::Left,
            border_color: theme::dark_grey(),
            padding_left: 2,
        ) {
            BlocksRenderer(
                blocks: props.children.clone(),
                file_path: file_path,
                viewport: props.viewport.clone(),
                muted: true,
            )
        }
    }
}
