use crate::debug;
use crate::output::graphics_manager::{
    GfxRect, GfxSource, IMAGE_HEIGHT_CACHE, ReleaseGuard, acquire, detach, dims, gfx_error, place,
    release,
};
use crate::theme;
use iocraft::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Unique id generator for graphics components. Each occurrence of an image or
/// math formula gets its own terminal graphic id so that placing/detaching one
/// occurrence never affects another that happens to share the same content.
static INSTANCE_ID: AtomicU64 = AtomicU64::new(0);
fn next_instance_id() -> u64 {
    INSTANCE_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Default, Props)]
pub struct MathBlockProps {
    pub content: String,
    pub display: bool,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    /// Current scroll offset in rows, from the owning ScrollView. When it
    /// changes we re-run placement so a post-scroll frame with a stale canvas
    /// rect does not leave the image detached at the wrong place.
    pub scroll_offset: Option<i32>,
}

#[component]
pub fn MathBlock(props: &MathBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    element! {
        View(margin_bottom: 1) {
            KittyMath(content: props.content.clone(), display: props.display.clone(), viewport_height: props.viewport_height, viewport_width: props.viewport_width, scroll_offset: props.scroll_offset)
        }
    }
}

#[derive(Default, Props)]
pub struct KittyMathProps {
    pub content: String,
    pub display: bool,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub scroll_offset: Option<i32>,
}

#[component]
pub fn KittyMath(props: &KittyMathProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);
    // Unique per-occurrence key so identical formulas don't share a terminal
    // graphic id (which would let one occurrence's detach/place clobber others).
    let instance = hooks.use_ref(|| next_instance_id());
    let key = format!(
        "math:{}:{}:{}#{}",
        vw,
        props.display,
        props.content,
        *instance.read()
    );
    // Stable layout height (see image.rs for rationale): size the box from the
    // height cache so it never collapses to 0 while the formula is loading.
    let cache_key = format!("math:{}:{}:{}", vw, props.display, props.content);
    let declared_rows = IMAGE_HEIGHT_CACHE
        .get(&cache_key)
        .map(|(_, h)| h)
        .unwrap_or(if props.display { 2 } else { 1 });
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
    // Real-layout invariant: `baseline = rect.top + scroll_offset` is constant
    // across scrolls. Captured when the real canvas rect moves; `y` is then
    // `baseline - scroll_offset`, correct even when `use_component_rect()` lags
    // the scroll.
    let mut baseline = hooks.use_ref(|| 0i32);
    let mut baseline_scroll = hooks.use_ref(|| 0i32);
    let mut last_rect_y = hooks.use_ref(|| i32::MIN);

    if acquired_key.read().is_empty() || *acquired_key.read() != key {
        if !acquired_key.read().is_empty() {
            release(acquired_key.read().clone());
        }
        let cell_w = caps_cache
            .read()
            .clone()
            .unwrap_or_default()
            .cell_w_px
            .max(1) as u32;
        let max_w = (cell_w * 2).max(16) * vw;
        acquire(
            key.clone(),
            GfxSource::Math {
                content: props.content.clone(),
                display: props.display,
                max_w,
                max_cols: vw,
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
        let x = r.left;
        let y_raw = r.top;
        // Derive the correct vertical position from the scroll-invariant
        // baseline. `baseline = y_raw + scroll_offset` is constant for the real
        // layout, so recompute it only when the canvas rect actually moves.
        // On a stale-rect frame `y_raw` is unchanged, so we keep the last good
        // `baseline` and apply the current scroll. This makes `y` correct even
        // when `use_component_rect()` lags the scroll.
        let so = props.scroll_offset.unwrap_or(*baseline_scroll.read());
        if y_raw != *last_rect_y.read() {
            *baseline.write() = y_raw + so;
            *baseline_scroll.write() = so;
            *last_rect_y.write() = y_raw;
        }
        let y = *baseline.read() - so;

        let pos = (x, y);
        if pos != drawn_at.get() {
            drawn_at.set(pos);

            let caps = caps_cache.read().clone().unwrap_or_default();
            let img_cols = *cols.read() as i32;
            let img_rows = *rows.read() as i32;

            let visible_cols = img_cols.min(term_width as i32 - x).max(0);
            let visible_rows = img_rows.min(term_height as i32 - y - 3).max(0);

            let top_clip_rows = if y < 0 { (-y + 1).min(img_rows) } else { 0 };
            let actual_vis_rows = (visible_rows - top_clip_rows).max(0);
            let render_y = if y < 1 { 1 } else { y };

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
        let m_cols = cols.read().clone().max(8);
        let m_rows = rows.read().clone().max(3);
        let label = if props.display {
            "Math (display)"
        } else {
            "Math (inline)"
        };
        element! {
            View(
                width: m_cols,
                height: m_rows,
                border_style: BorderStyle::Single,
                border_color: theme::DBG_MATH,
                background_color: theme::DBG_BG,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            ) {
                Text(content: format!("{} {}x{}", label, m_cols, m_rows), color: theme::DBG_MATH, weight: Weight::Bold)
            }
        }
        .into_any()
    } else {
        element! {View(width: cols.read().clone().max(1), height: declared_rows.max(1))}.into_any()
    }
}
