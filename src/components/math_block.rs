use iocraft::prelude::*;
use std::io::Write;

use crate::{
    assets::math::render_math,
    output::{capabilities::TermCaps, kitty},
};

#[derive(Default, Props)]
pub struct MathBlockProps {
    pub content: String,
    pub display: bool,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn MathBlock(props: &MathBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    element! {
       KittyMath(content: props.content.clone(), display: props.display.clone(), viewport_height: props.viewport_height, viewport_width: props.viewport_width)
    }
}

#[derive(Default, Props)]
pub struct KittyMathProps {
    pub content: String,
    pub display: bool,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn KittyMath(props: &KittyMathProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let rect = hooks.use_component_rect();
    let (term_width, term_height) = hooks.use_terminal_size();
    let mut drawn_at = hooks.use_state(|| (-1i32, -1i32));
    let image_id = hooks.use_state(|| kitty::next_placement_id()).get();
    let mut data_cache = hooks.use_ref(|| Vec::<u8>::new());
    let mut cols = hooks.use_ref(|| 0u32);
    let mut rows = hooks.use_ref(|| 0u32);

    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);

    if data_cache.read().is_empty() {
        let loaded_image = match render_math(props.content.as_str(), props.display, vw, true) {
            Ok(v) => v,
            Err(e) => {
                return element! {
                    View() {
                        Text(content: format!("Error: {:#}", e))
                    }
                };
            }
        };
        data_cache.set(loaded_image.0);
        let mut cols_ = loaded_image.1;
        let mut rows_ = loaded_image.2;
        let caps = TermCaps::detect().unwrap();

        cols_ = ((cols_ as f32) / (caps.cell_w_px as f32)).ceil() as u32;
        cols_ = cols_.min(vw);
        rows_ = ((rows_ as f32) / (caps.cell_h_px as f32)).ceil() as u32;
        rows_ = rows_.min(vh);

        cols.set(cols_);
        rows.set(rows_);
    }

    let render_image = hooks.use_async_handler(
        move |(pos, visible, vis_cols, vis_rows): ((i32, i32), bool, i32, i32)| async move {
            if !kitty::is_supported() {
                return;
            }
            let mut stdout = std::io::stdout().lock();
            kitty::delete_by_id(&mut stdout, image_id);
            if visible && !data_cache.read().is_empty() {
                let (x, y) = pos;
                write!(stdout, "\x1b7").unwrap();
                write!(stdout, "\x1b[{};{}H", y + 1, x + 1).unwrap();
                kitty::write_to(
                    &mut stdout,
                    &data_cache.read(),
                    vis_cols as u32, // clipped
                    vis_rows as u32, // clipped
                );
                write!(stdout, "\x1b8").unwrap();
            }
            stdout.flush().unwrap();
        },
    );
    if let Some(r) = rect {
        let pos = (r.left, r.top);
        if pos != drawn_at.get() {
            drawn_at.set(pos);

            let img_cols = *cols.read() as i32;
            let img_rows = *rows.read() as i32;

            let (x, y) = pos;
            let visible_cols = img_cols.min(term_width as i32 - x).max(0);
            let visible_rows = img_rows.min(term_height as i32 - y - 1).max(0);

            let visible = x >= 0 && y >= 0 && visible_cols > 0 && visible_rows > 0;

            render_image((pos, visible, visible_cols, visible_rows));
        }
    }

    element! {View(width: cols.read().clone(), height: rows.read().clone())}
}
