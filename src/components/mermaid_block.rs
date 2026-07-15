use crate::debug;
use crate::output::graphics_manager::{
    GfxRect, GfxSource, ReleaseGuard, acquire, detach, dims, gfx_error, place, release,
};
use crate::theme;
use iocraft::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Unique id generator for graphics components. Each occurrence of a mermaid
/// diagram (or image/math formula) gets its own terminal graphic id so that
/// placing/detaching one occurrence never affects another that shares content.
static INSTANCE_ID: AtomicU64 = AtomicU64::new(0);
fn next_instance_id() -> u64 {
    INSTANCE_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Default, Props)]
pub struct MermaidBlockProps {
    pub source: String,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn MermaidBlock(props: &MermaidBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    element! {
       View(flex_direction: FlexDirection::Column, margin_bottom: 1) {
           KittyMermaid(source: props.source.clone(), viewport_height: props.viewport_height, viewport_width: props.viewport_width)
       }
    }
}

#[derive(Default, Props)]
pub struct KittyMermaidProps {
    pub source: String,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn KittyMermaid(props: &KittyMermaidProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);
    // Unique per-occurrence key so identical diagrams don't share a terminal
    // graphic id (which would let one occurrence's detach/place clobber others).
    let instance = hooks.use_ref(|| next_instance_id());
    let key = format!("{}:{}#{}", vw, props.source, *instance.read());
    let (cached_cols, cached_rows) = dims(&key).unwrap_or((0, 0));

    let rect = hooks.use_component_rect();
    let (term_width, term_height) = hooks.use_terminal_size();
    let mut drawn_at = hooks.use_state(|| (-1i32, -1i32));
    let mut cols = hooks.use_ref(|| cached_cols);
    let mut rows = hooks.use_ref(|| cached_rows);
    let mut error_msg = hooks.use_state(|| None::<String>);
    let mut sized = hooks.use_state(|| false);
    let mut acquired_key = hooks.use_ref(|| String::new());
    let mut cur_key = hooks.use_ref(|| Arc::new(Mutex::new(String::new())));
    let caps_cache = hooks.use_ref(|| crate::output::capabilities::TermCaps::detect().ok());

    if acquired_key.read().is_empty() || *acquired_key.read() != key {
        if !acquired_key.read().is_empty() {
            release(acquired_key.read().clone());
        }
        let cell_w = caps_cache
            .read()
            .clone()
            .unwrap_or_default()
            .cell_w_px
            .max(1) as f32;
        let max_w = ((vw as f32) * cell_w * 2.0).round() as u32;
        acquire(
            key.clone(),
            GfxSource::Mermaid {
                source: props.source.clone(),
                max_w,
                max_cols: (2.0 * vw as f32).round() as u32,
                max_rows: vh,
            },
        );
        *cur_key.read().lock().unwrap() = key.clone();
        acquired_key.set(key.clone());
        if dims(&key).is_none() {
            cols.set(0);
            rows.set(0);
            sized.set(false);
        }
    }

    if let Some((c, r)) = dims(&key) {
        if *cols.read() != c || *rows.read() != r {
            cols.set(c);
            rows.set(r);
            sized.set(true);
            drawn_at.set((-1, -1));
        }
    }
    if let Some(err) = gfx_error(&key) {
        if error_msg.read().is_none() {
            error_msg.set(Some(err));
        }
    }

    if let Some(r) = rect {
        let pos = (r.left, r.top);
        if pos != drawn_at.get() {
            drawn_at.set(pos);

            let caps = caps_cache.read().clone().unwrap_or_default();
            let img_cols = *cols.read() as i32;
            let img_rows = *rows.read() as i32;

            let (x, y) = pos;
            let visible_cols = img_cols.min(term_width as i32 - x).max(0);
            let visible_rows = img_rows.min(term_height as i32 - y - 3).max(0);

            let top_clip_rows = if y < 0 { (-y).min(img_rows) } else { 0 };
            let actual_vis_rows = (visible_rows - top_clip_rows).max(0);
            let render_y = if y < 0 { 0 } else { y };

            let visible = x >= 0 && actual_vis_rows > 0 && visible_cols > 0;

            let rect_cmd = GfxRect {
                x,
                y: render_y,
                vis_cols: visible_cols,
                vis_rows: actual_vis_rows,
                src_y_offset: top_clip_rows,
                cell_w: caps.cell_w_px as u32,
                cell_h: caps.cell_h_px as u32,
            };

            if visible {
                place(key.clone(), rect_cmd);
                debug::log_event(&debug::DebugEvent::ImagePlace {
                    ts: debug::elapsed_ms(),
                    id: 0,
                    x,
                    y: render_y,
                    cols: visible_cols,
                    rows: actual_vis_rows,
                    src_y_offset: top_clip_rows,
                });
            } else {
                detach(key.clone());
                debug::log_event(&debug::DebugEvent::ImageDetach {
                    ts: debug::elapsed_ms(),
                    id: 0,
                    reason: "scrolled_offscreen".into(),
                });
            }
        }
    }

    let _release_guard = hooks.use_ref({
        let ck = cur_key.read().clone();
        move || ReleaseGuard { key: ck }
    });

    if let Some(err) = error_msg.read().clone() {
        return element! {
            View() {
                Text(content: err, color: theme::RED)
            }
        }
        .into_any();
    }

    if debug::are_annotations_enabled() {
        let m_cols = cols.read().clone().max(10);
        let m_rows = rows.read().clone().max(5);
        element! {
            View(
                width: m_cols,
                height: m_rows,
                border_style: BorderStyle::Single,
                border_color: theme::DBG_MERMAID,
                background_color: theme::DBG_BG,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            ) {
                Text(content: format!("Mermaid {}x{}", m_cols, m_rows), color: theme::DBG_MERMAID, weight: Weight::Bold)
            }
        }
        .into_any()
    } else {
        element! {View(width: cols.read().clone(), height: rows.read().clone())}.into_any()
    }
}
