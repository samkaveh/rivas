use crate::components::code_block::CodeBlock;
use crate::components::editor::{EditorState, Mode};
use crate::components::footnote_block::FootnoteBlock;
use crate::components::heading::Heading;
use crate::components::html_block::HtmlBlock;
use crate::components::image::Image;
use crate::components::list_block::ListBlock;
use crate::components::math_block::MathBlock;
use crate::components::mermaid_block::MermaidBlock;
use crate::components::paragraph::Paragraph;
use crate::components::quote_block::QuoteBlock;
use crate::components::raw_buffer::{CursorStyle, RawState};
use crate::components::scroll::{
    Viewport, block_gap, build_cumulative_heights, compute_scroll_into_view_target,
    estimate_block_height, find_cursor_block, visible_range_with_cursor,
};
use crate::components::table_block::TableBlock;
use crate::components::thematic_break::ThematicBreak;
use crate::debug;
use crate::document::model::Block;
use crate::output::graphics_manager::IMAGE_HEIGHT_CACHE;
use crate::theme;
use iocraft::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default, Props)]
struct ScrollIntoViewContainerProps {
    pub scroll_handle: Option<Ref<ScrollViewHandle>>,
    pub cursor_moved: bool,
    pub child: Option<Arc<dyn Fn() -> AnyElement<'static> + Send + Sync + 'static>>,
    /// Row of the cursor within this block (0-indexed). Used for precise
    /// scroll-into-view when the block is taller than the viewport.
    pub cursor_row: Option<i32>,
    /// Extra rows below the block content to keep visible (e.g. the status
    /// box showing "Ln X, Col Y: ...").
    pub bottom_offset: Option<i32>,
    /// Identity of the block this container currently wraps (its block index).
    /// Used to detect when the cursor jumps to a different block (e.g. `gg`) so
    /// a stale baseline is never consumed.
    pub block_identity: Option<i32>,
}

#[component]
fn ScrollIntoViewContainer(
    props: &ScrollIntoViewContainerProps,
    mut hooks: Hooks,
) -> impl Into<AnyElement<'static>> {
    let rect = hooks.use_component_rect();
    let mut pending = hooks.use_state(|| false);
    let mut baseline = hooks.use_state(|| None::<(i32, i32)>);
    let mut last_rect_top = hooks.use_state(|| i32::MIN);
    let mut last_block_identity = hooks.use_state(|| None::<i32>);

    if props.cursor_moved {
        pending.set(true);
        // When the cursor jumps to a different block (e.g. `gg`), the stored
        // baseline still describes the *previous* block; consuming it would yank
        // the view to the old block's position. Invalidate it until the current
        // block's rect settles (next frame); the effect below retries as soon as
        // a valid baseline is re-captured.
        if props.block_identity != last_block_identity.get() {
            last_block_identity.set(props.block_identity);
            baseline.set(None);
        }
    }

    // Baseline invariant: `use_component_rect()` is measured from the *previous*
    // frame and is therefore one scroll-step stale. `rect.top + scroll_off`
    // recovers the block's true content-row position, but while a scroll is in
    // flight `rect.top` still reflects the old layout, so the sum is wrong for a
    // single frame. We capture the invariant only once `rect.top` actually
    // settles (changes), so a mid-flight stale rect never feeds a bogus target
    // and the auto-scroll converges in one step instead of oscillating.
    if let Some(r) = rect {
        if r.top != last_rect_top.get() {
            if let Some(scroll_ref) = &props.scroll_handle {
                let scroll_off = scroll_ref.read().scroll_offset();
                let top = r.top + scroll_off;
                let bottom = r.bottom + scroll_off;
                baseline.set(Some((top, bottom)));
                last_rect_top.set(r.top);
            }
        }
    }

    hooks.use_effect(
        {
            let mut pending = pending.clone();
            let scroll_handle = props.scroll_handle.clone();
            let baseline = baseline.clone();
            let cursor_row = props.cursor_row;
            let bottom_offset = props.bottom_offset.unwrap_or(0);
            move || {
                // Only consume the pending request once we actually have a
                // baseline (i.e. `use_component_rect` has reported at least one
                // real frame for this element). A freshly mounted cursor block
                // gets its first rect a frame after `cursor_moved`, so retrying
                // here is what makes `j`/`k` scroll the view to follow the
                // cursor instead of silently doing nothing.
                if pending.get() && baseline.get().is_some() {
                    if let (Some(scroll_ref), Some((block_top_content, block_bottom_content))) =
                        (&scroll_handle, baseline.get())
                    {
                        let mut scroll_ref = scroll_ref.clone();
                        let viewport_h = scroll_ref.read().viewport_height() as i32;
                        let content_h = scroll_ref.read().content_height() as i32;

                        if viewport_h > 0 {
                            let scroll_off = scroll_ref.read().scroll_offset();
                            let target = compute_scroll_into_view_target(
                                block_top_content,
                                block_bottom_content,
                                viewport_h,
                                content_h,
                                scroll_off,
                                cursor_row,
                                bottom_offset,
                            );
                            if let Some(target) = target {
                                scroll_ref.write().scroll_to(target);
                                debug::log_event(&debug::DebugEvent::CursorScroll {
                                    ts: debug::elapsed_ms(),
                                    block_top: block_top_content,
                                    block_bottom: block_bottom_content,
                                    scroll_off,
                                    target,
                                    viewport_h,
                                });
                            }
                        }
                        pending.set(false);
                    }
                }
            }
        },
        (pending.get(), baseline.get()),
    );

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(props.child.as_ref().map(|f| f()).into_iter())
        }
    }
}

/// Renders a single block as a Markdown element, or — when `raw` is set — as
/// its editable source.
fn render_block_element(
    block: &Block,
    file_path: &std::path::PathBuf,
    viewport: &Option<Viewport>,
    raw: Option<RawState>,
    muted: bool,
) -> AnyElement<'static> {
    let text_color = muted.then(theme::muted);
    match block {
        Block::Heading { level, content, .. } => element!{
    Heading(level: *level, content: content.clone(), file_path: file_path.clone(), viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::Paragraph { content, .. } => element!{Paragraph(content: content.clone(), file_path: file_path.clone(), viewport: viewport.clone(), raw: raw.clone(), color: text_color)}.into_any(),
        Block::Code { language, code, .. } => element!{CodeBlock(language: language.clone(), code: code.clone(), raw: raw.clone())}.into_any(),
        Block::Mermaid { source, .. } => element!{MermaidBlock(source: source.clone(), viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::Math { content, display, .. } => element!{MathBlock(content: content.clone(), display: *display, viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::Quote { children, .. } => element!{QuoteBlock(children: children.clone(), file_path: Some(file_path.clone()), viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::List { ordered, start, items, .. } => element!{ListBlock(ordered: *ordered, start: *start, items: items.clone(), file_path: file_path.clone(), viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::Table { headers, alignments, rows, .. } => element!{TableBlock(headers: headers.clone(), alignments: alignments.clone(), rows: rows.clone(), file_path: file_path.clone(), viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::ThematicBreak{..} => element!{ThematicBreak(viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::Image { alt, url, title, .. } => element!{Image(url: url.clone(), file_path: file_path.clone(), title: title.clone(), alt: Some(alt.clone()), viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::Html { content, .. } => element!{HtmlBlock(content: content.clone(), file_path: Some(file_path.clone()), viewport: viewport.clone(), raw: raw.clone())}.into_any(),
        Block::FootnoteDefinition { label, children, .. } => element!{FootnoteBlock(label: label.clone(), children: children.clone(), file_path: Some(file_path.clone()), viewport: viewport.clone(), raw: raw.clone())}.into_any(),
    }
}

/// Columns the edit marker hangs into the left gutter (marker + one space).
pub const EDIT_GUTTER: u32 = 2;

/// Wraps the block being edited with a full-height, mode-colored marker that
/// hangs in the left gutter (negative margin), so entering a block never
/// shifts its text horizontally — the source appears exactly where the
/// rendered block was.
fn with_edit_marker(child: AnyElement<'static>, color: Color) -> AnyElement<'static> {
    element! {
        View(
            flex_direction: FlexDirection::Column,
            margin_left: -(EDIT_GUTTER as i32),
            padding_left: EDIT_GUTTER - 1,
            border_style: BorderStyle::Custom(BorderCharacters {
                top_left: ' ', top_right: ' ', bottom_left: ' ', bottom_right: ' ',
                left: '▎', right: ' ', top: ' ', bottom: ' ',
            }),
            border_edges: Edges::Left,
            border_color: color,
        ) {
            #(Some(child))
        }
    }
    .into_any()
}

/// Splits `text` into lines and locates `rel_off` (a byte offset into `text`)
/// as `(line index, byte column)`.
fn locate_cursor(text: &str, rel_off: usize) -> (Option<usize>, usize) {
    let mut acc = 0usize;
    let mut found = (None, 0usize);
    for (idx, line) in text.split('\n').enumerate() {
        if rel_off >= acc && rel_off <= acc + line.len() {
            found = (Some(idx), rel_off - acc);
        }
        acc += line.len() + 1;
    }
    found
}

/// Properties for the [`BlocksRenderer`] component.
#[derive(Default, Props)]
pub struct BlocksRendererProps {
    /// The parsed Markdown blocks to render.
    pub blocks: Vec<Block>,
    /// The full source text of the document (for byte-offset calculations in the editor).
    pub content: String,
    /// File path for resolving relative image/link URLs.
    pub file_path: PathBuf,
    /// Optional viewport dimensions for line wrapping and responsive rendering.
    pub viewport: Option<Viewport>,
    /// Current cursor byte offset in the document (for editor cursor overlay).
    pub cursor_offset: Option<Ref<usize>>,
    /// Reference to the editor state (for cursor rendering and mode checks).
    pub editor_state: Option<Ref<Option<EditorState>>>,
    /// Handle for scrolling the cursor into view.
    pub scroll_handle: Option<Ref<ScrollViewHandle>>,
    /// `true` to render debug overlays (block boundaries, byte offsets).
    pub debug: bool,
    /// `true` to render inline debug annotations (highlight spans, block types).
    pub debug_annotations: bool,
    /// Stack blocks with no gap between them (list item contents).
    pub tight: bool,
    /// Render prose in the secondary (muted) text color (block quotes).
    pub muted: bool,
}

/// Renders a sequence of Markdown blocks with optional editor cursor overlay.
///
/// Each block is rendered as its appropriate component (heading, paragraph,
/// code, …) and followed by the gap from [`block_gap`]. The block under the
/// editor cursor (and any block touched by a visual selection) renders as its
/// syntax-highlighted source instead, through the same component in every
/// mode, so pressing `i`/`v`/`Esc` only changes the cursor and marker color.
#[component]
pub fn BlocksRenderer(
    props: &BlocksRendererProps,
    mut hooks: Hooks,
) -> impl Into<AnyElement<'static>> {
    let cursor_offset_val = props.cursor_offset.as_ref().map(|r| r.get());
    let last_offset = hooks.use_state(|| cursor_offset_val.unwrap_or(0));
    let cursor_moved = cursor_offset_val.map_or(false, |off| off != last_offset.get());

    hooks.use_effect(
        {
            let mut last_offset = last_offset.clone();
            move || {
                if let Some(off) = cursor_offset_val {
                    last_offset.set(off);
                }
            }
        },
        cursor_offset_val,
    );
    let file_path = props.file_path.clone();
    let vh = props.viewport.as_ref().and_then(|v| v.height);
    let vw = props.viewport.as_ref().and_then(|v| v.width);
    let cursor_offset = props.cursor_offset.as_ref().map(|r| r.get());

    let (vis_start, vis_end, mode, operator_pending) = if let Some(state_ref) = &props.editor_state {
        let s_opt = state_ref.read();
        if let Some(s) = s_opt.as_ref() {
            let start = s.absolute_byte_offset_at(s.visual_start.0, s.visual_start.1);
            let end = s.absolute_byte_offset();
            (
                Some(start.min(end)),
                Some(start.max(end)),
                s.mode.clone(),
                s.operator.is_some(),
            )
        } else {
            (None, None, Mode::Normal, false)
        }
    } else {
        (None, None, Mode::Normal, false)
    };
    let top_level = props.editor_state.is_some();

    let block_counts = props.blocks.len();

    // Cache cumulative block heights and start offsets — only recompute when
    // the document content, block count, width or graphics sizing changes. The
    // content fingerprint is what catches in-place edits (same block count):
    // without it, retyping inside a paragraph would keep stale placeholder
    // heights for virtualized blocks until a resize or image load.
    let mut content_hasher = DefaultHasher::new();
    props.content.hash(&mut content_hasher);
    let cum_key = format!(
        "{}:{:016x}:{}:{}",
        block_counts,
        content_hasher.finish(),
        vw.unwrap_or(0),
        IMAGE_HEIGHT_CACHE.generation()
    );
    let mut cum_data = hooks.use_ref(|| (Vec::<u32>::new(), Vec::<usize>::new()));
    let mut cum_key_ref = hooks.use_ref(String::new);
    if *cum_key_ref.read() != cum_key {
        cum_data.set(build_cumulative_heights(&props.blocks, &props.content, vw));
        cum_key_ref.set(cum_key);
    }

    // Binary search to find visible range using cached cumulative heights
    let scroll_offset = props
        .viewport
        .as_ref()
        .and_then(|v| v.scroll_offset)
        .unwrap_or(0)
        .max(0) as u32;
    let viewport_h = props.viewport.as_ref().map(|v| v.height()).unwrap_or(24);
    let buffer = viewport_h * 2;
    let (heights, starts) = {
        let d = cum_data.read();
        (d.0.clone(), d.1.clone())
    };

    let cursor_block_idx = find_cursor_block(&starts, cursor_offset, block_counts);

    // Only the top-level renderer virtualizes; nested containers (quotes, list
    // items) are short and receive no scroll offset of their own.
    let (first_visible, last_visible) = if top_level {
        visible_range_with_cursor(
            scroll_offset,
            viewport_h as u32,
            buffer,
            &heights,
            block_counts,
            cursor_block_idx,
        )
    } else {
        (0, block_counts)
    };

    // Log render tick for debug
    if props.debug {
        if let Some(state_ref) = &props.editor_state {
            let s_opt = state_ref.read();
            if let Some(s) = s_opt.as_ref() {
                debug::log_event(&debug::DebugEvent::RenderTick {
                    ts: debug::elapsed_ms(),
                    cursor: debug::CursorPos {
                        byte: s.absolute_byte_offset(),
                        row: s.row,
                        col: s.col,
                    },
                    scroll: props
                        .viewport
                        .as_ref()
                        .and_then(|v| v.scroll_offset)
                        .unwrap_or(0),
                    content_height: props
                        .scroll_handle
                        .as_ref()
                        .map(|h| h.read().content_height() as i32)
                        .unwrap_or(0),
                    viewport: debug::ViewportInfo {
                        w: vw.unwrap_or(80),
                        h: vh.unwrap_or(24),
                    },
                    blocks: block_counts,
                    mode: format!("{:?}", s.mode),
                });
            }
        }
    }

    let cursor_style = match mode {
        Mode::Insert => CursorStyle::Insert,
        _ if operator_pending => CursorStyle::Operator,
        _ => CursorStyle::Block,
    };

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(props.blocks.iter().enumerate().map(|(i, block)| {
                let gap = block_gap(props.blocks.get(i + 1), props.tight);

                // Virtual scrolling: skip off-screen blocks (spacer height
                // already includes the trailing gap).
                if i < first_visible || i >= last_visible {
                    let h = heights[i + 1] - heights[i];
                    return element! { View(height: h) {} }.into_any();
                }

                let span = block.span();
                let next_span_start = props.blocks.get(i + 1).map(|b| b.span().0).unwrap_or(props.content.len());

                // The cursor belongs to this block when it is on it or in the
                // blank gap before the next block.
                let is_cursor_here = top_level && cursor_offset.map_or(false, |off| {
                    if i + 1 == block_counts {
                        off >= span.0 && off <= next_span_start
                    } else {
                        off >= span.0 && off < next_span_start
                    }
                });
                let is_selected = top_level && mode == Mode::Visual && vis_start.map_or(false, |start| {
                    vis_end.map_or(false, |end| span.0 <= end && span.1 >= start)
                });

                let raw_state = if is_cursor_here || is_selected {
                    let off = cursor_offset.unwrap_or(0);
                    // When the cursor sits in the blank gap after the block,
                    // extend the source view down to it so the cursor is
                    // drawn on the line it is actually on.
                    let text_end = if is_cursor_here && off > span.1 {
                        if i + 1 == block_counts {
                            off.min(next_span_start)
                        } else {
                            off.min(next_span_start.saturating_sub(1))
                        }
                    } else {
                        span.1
                    };
                    let text_end = text_end.max(span.0).min(props.content.len());
                    let mut text = &props.content[span.0..text_end];
                    // Block spans often end with the newline(s) separating
                    // them from the next block; don't show those as empty
                    // source rows unless the cursor is actually on them.
                    let rel_off = off.saturating_sub(span.0);
                    while text.ends_with('\n') && (!is_cursor_here || rel_off < text.len()) {
                        text = &text[..text.len() - 1];
                    }
                    let text_end = span.0 + text.len();
                    let (cursor_line, cursor_col) = if is_cursor_here {
                        locate_cursor(text, off.saturating_sub(span.0).min(text.len()))
                    } else {
                        (None, 0)
                    };
                    let selection = if mode == Mode::Visual {
                        match (vis_start, vis_end) {
                            (Some(a), Some(b)) if b >= span.0 && a < text_end.max(span.0 + 1) => Some((
                                a.max(span.0) - span.0,
                                b.min(text_end.saturating_sub(1)).saturating_sub(span.0),
                            )),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    Some(RawState {
                        text: text.to_string(),
                        cursor_line,
                        cursor_col,
                        cursor_style,
                        selection,
                        highlight: None,
                    })
                } else {
                    None
                };
                let cursor_row = raw_state.as_ref().and_then(|r| r.cursor_line);

                let rendered = render_block_element(block, &file_path, &props.viewport, raw_state.clone(), props.muted);

                // Wrap with debug border/label if debug annotations are enabled
                let rendered = if props.debug_annotations {
                    let (label, color) = match block {
                        Block::Heading { .. } => ("H".to_string(), theme::dbg_heading()),
                        Block::Paragraph { .. } => ("P".to_string(), theme::dbg_paragraph()),
                        Block::Code { .. } => ("Code".to_string(), theme::dbg_code()),
                        Block::Image { .. } => ("Img".to_string(), theme::dbg_image()),
                        Block::Math { .. } => ("Math".to_string(), theme::dbg_math()),
                        Block::Mermaid { .. } => ("Mermaid".to_string(), theme::dbg_mermaid()),
                        Block::Quote { .. } => (">".to_string(), theme::dbg_quote()),
                        Block::Table { .. } => ("Table".to_string(), theme::dbg_table()),
                        Block::List { .. } => ("List".to_string(), theme::dbg_list()),
                        Block::ThematicBreak { .. } => ("---".to_string(), theme::dbg_break()),
                        Block::Html { .. } => ("HTML".to_string(), theme::dbg_html()),
                        Block::FootnoteDefinition { .. } => ("FN".to_string(), theme::dbg_quote()),
                    };
                    let est_h = estimate_block_height(block, &props.content, vw);
                    debug::log_event(&debug::DebugEvent::BlockLayout {
                        ts: debug::elapsed_ms(),
                        idx: i,
                        block_type: label.clone(),
                        span_start: span.0,
                        span_end: span.1,
                        est_height: est_h,
                    });
                    element! {
                        View(flex_direction: FlexDirection::Column) {
                            View(flex_direction: FlexDirection::Row, background_color: color, padding_left: 1) {
                                Text(content: format!("[{} {}..{} h={}]", label, span.0, span.1, est_h), color: theme::dark_bg(), weight: Weight::Bold)
                            }
                            View(border_style: BorderStyle::Single, border_color: color, background_color: theme::dbg_bg()) {
                                #(Some(rendered).into_iter())
                            }
                        }
                    }.into_any()
                } else {
                    rendered
                };

                let body = if is_cursor_here {
                    let block_clone = block.clone();
                    let file_path_clone = file_path.clone();
                    let viewport_clone = props.viewport.clone();
                    let raw_clone = raw_state.clone();
                    let marker = mode.color();
                    let factory: Arc<dyn Fn() -> AnyElement<'static> + Send + Sync + 'static> =
                        Arc::new(move || {
                            with_edit_marker(
                                render_block_element(
                                    &block_clone,
                                    &file_path_clone,
                                    &viewport_clone,
                                    raw_clone.clone(),
                                    false,
                                ),
                                marker,
                            )
                        });
                    element! {
                        ScrollIntoViewContainer(
                            scroll_handle: props.scroll_handle.clone(),
                            cursor_moved,
                            child: Some(factory),
                            cursor_row: cursor_row.map(|r| r as i32),
                            bottom_offset: Some(0),
                            block_identity: Some(i as i32),
                        )
                    }.into_any()
                } else if is_selected {
                    with_edit_marker(rendered, theme::magenta())
                } else {
                    rendered
                };

                element! {
                    View(flex_direction: FlexDirection::Column, margin_bottom: gap) {
                        #(Some(body))
                    }
                }.into_any()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::model::*;

    #[test]
    fn estimate_heading_height() {
        let block = Block::Heading {
            level: 1,
            content: vec![Inline::Text("Hello".to_string())],
            span: (0, 10),
        };
        assert_eq!(estimate_block_height(&block, "", Some(80)), 2);
    }

    #[test]
    fn estimate_paragraph_height_wraps() {
        // 160 chars at content width = 80 - CONTENT_H_INSET(6) = 74 → 3 rows
        let long_text = "a".repeat(160);
        let block = Block::Paragraph {
            content: vec![Inline::Text(long_text)],
            span: (0, 160),
        };
        assert_eq!(estimate_block_height(&block, "", Some(80)), 3);
    }

    #[test]
    fn estimate_paragraph_uses_content_inset() {
        let block = Block::Paragraph {
            content: vec![Inline::Text("a".repeat(74))],
            span: (0, 74),
        };
        assert_eq!(estimate_block_height(&block, "", Some(80)), 1);
        let block = Block::Paragraph {
            content: vec![Inline::Text("a".repeat(75))],
            span: (0, 75),
        };
        assert_eq!(estimate_block_height(&block, "", Some(80)), 2);
    }

    #[test]
    fn estimate_paragraph_short_fits_one_row() {
        let block = Block::Paragraph {
            content: vec![Inline::Text("short".to_string())],
            span: (0, 5),
        };
        assert_eq!(estimate_block_height(&block, "", Some(80)), 1);
    }

    #[test]
    fn estimate_code_block_height() {
        let block = Block::Code {
            language: Some("rust".to_string()),
            code: "fn main() {\n    println!(\"hi\");\n}".to_string(),
            span: (0, 30),
        };
        // 3 lines of code + 2 (language label + padding)
        assert_eq!(estimate_block_height(&block, "", Some(80)), 5);
    }

    #[test]
    fn estimate_table_height() {
        let block = Block::Table {
            headers: vec![TableCell {
                content: vec![Inline::Text("A".to_string())],
            }],
            alignments: vec![Alignment::Left],
            rows: vec![
                vec![TableCell {
                    content: vec![Inline::Text("1".to_string())],
                }],
                vec![TableCell {
                    content: vec![Inline::Text("2".to_string())],
                }],
            ],
            span: (0, 20),
        };
        // top border + header + header rule + 2 rows + bottom border = 6
        assert_eq!(estimate_block_height(&block, "", Some(80)), 6);
    }

    #[test]
    fn estimate_list_height() {
        let block = Block::List {
            ordered: false,
            start: None,
            items: vec![
                ListItem {
                    checked: None,
                    content: vec![Block::Paragraph {
                        content: vec![],
                        span: (0, 5),
                    }],
                },
                ListItem {
                    checked: None,
                    content: vec![Block::Paragraph {
                        content: vec![],
                        span: (6, 10),
                    }],
                },
            ],
            span: (0, 10),
        };
        assert_eq!(estimate_block_height(&block, "", Some(80)), 2);
    }

    #[test]
    fn estimate_thematic_break() {
        let block = Block::ThematicBreak { span: (0, 3) };
        assert_eq!(estimate_block_height(&block, "", Some(80)), 1);
    }
}
