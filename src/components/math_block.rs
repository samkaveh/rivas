use crate::theme;
use crate::{
    assets::math::render_math,
    output::{capabilities::TermCaps, kitty},
};
use iocraft::prelude::*;
use std::io::Write;
use std::sync::{Arc, Mutex, mpsc};

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
        View(margin_bottom: 1) {
            KittyMath(content: props.content.clone(), display: props.display.clone(), viewport_height: props.viewport_height, viewport_width: props.viewport_width)
        }
    }
}

#[derive(Default, Props)]
pub struct KittyMathProps {
    pub content: String,
    pub display: bool,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

enum MathCmd {
    Render {
        id: u32,
        data: Vec<u8>,
        x: i32,
        y: i32,
        vis_cols: i32,
        vis_rows: i32,
        src_y_offset: i32,
        cell_w: u32,
        cell_h: u32,
    },
    Place {
        id: u32,
        x: i32,
        y: i32,
        vis_cols: i32,
        vis_rows: i32,
        src_y_offset: i32,
        cell_w: u32,
        cell_h: u32,
    },
    Detach(u32),
    Free(u32),
}

#[component]
pub fn KittyMath(props: &KittyMathProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let rect = hooks.use_component_rect();
    let (term_width, term_height) = hooks.use_terminal_size();
    let mut drawn_at = hooks.use_state(|| (-1i32, -1i32));
    let mut image_id = hooks.use_ref(|| kitty::ImageGuard::new());
    let mut data_cache = hooks.use_ref(|| Vec::<u8>::new());
    let mut cache_key = hooks.use_ref(String::new);
    let mut cols = hooks.use_ref(|| 0u32);
    let mut rows = hooks.use_ref(|| 0u32);
    let mut error_msg = hooks.use_state(|| None::<String>);
    let mut loading = hooks.use_ref(|| false);
    let mut load_result =
        hooks.use_ref(|| Arc::new(Mutex::new(None::<Result<(Vec<u8>, u32, u32), String>>)));
    let mut transmitted = hooks.use_ref(|| false);
    let caps_cache = hooks.use_ref(|| TermCaps::detect().ok());
    let io_tx = hooks.use_ref(|| {
        let (tx, rx) = mpsc::channel::<MathCmd>();
        std::thread::spawn(move || {
            let mut last_id = 0u32;
            while let Ok(mut cmd) = rx.recv() {
                while let Ok(next) = rx.try_recv() {
                    cmd = next;
                }
                let mut stdout = std::io::stdout().lock();
                match cmd {
                    MathCmd::Render {
                        id,
                        data,
                        x,
                        y,
                        vis_cols,
                        vis_rows,
                        src_y_offset,
                        cell_w,
                        cell_h,
                    } => {
                        let src_y_px = src_y_offset as u32 * cell_h;
                        let crop_h_px = vis_rows as u32 * cell_h;
                        let crop_w_px = vis_cols as u32 * cell_w;

                        write!(stdout, "\x1b7").unwrap();
                        write!(stdout, "\x1b[{};{}H", y + 1, x + 1).unwrap();

                        if last_id != 0 {
                            kitty::delete_image(&mut stdout, last_id);
                        }

                        // a=T auto-places at cursor — placement is tracked by id
                        kitty::write_to_cropped(
                            &mut stdout,
                            id,
                            &data,
                            vis_cols as u32,
                            vis_rows as u32,
                            0,
                            src_y_px,
                            crop_w_px,
                            crop_h_px,
                        );

                        last_id = id;

                        write!(stdout, "\x1b8").unwrap();
                        stdout.flush().unwrap();
                    }
                    MathCmd::Place {
                        id,
                        x,
                        y,
                        vis_cols,
                        vis_rows,
                        src_y_offset,
                        cell_w,
                        cell_h,
                    } => {
                        let src_y_px = src_y_offset as u32 * cell_h;
                        let crop_h_px = vis_rows as u32 * cell_h;
                        let crop_w_px = vis_cols as u32 * cell_w;

                        write!(stdout, "\x1b7").unwrap();
                        write!(stdout, "\x1b[{};{}H", y + 1, x + 1).unwrap();

                        // Delete old placement (keep data cached)
                        kitty::delete_placements(&mut stdout, id);

                        // Create fresh placement at cursor (no retransmission)
                        kitty::place_image(
                            &mut stdout,
                            id,
                            vis_cols as u32,
                            vis_rows as u32,
                            0,
                            src_y_px,
                            crop_w_px,
                            crop_h_px,
                        );

                        write!(stdout, "\x1b8").unwrap();
                        stdout.flush().unwrap();
                    }
                    MathCmd::Detach(id) => {
                        if id != 0 {
                            kitty::delete_placements(&mut stdout, id);
                            stdout.flush().unwrap();
                        }
                    }
                    MathCmd::Free(id) => {
                        if id != 0 {
                            kitty::delete_image(&mut stdout, id);
                            stdout.flush().unwrap();
                            last_id = 0;
                        }
                    }
                }
            }
        });
        Some(tx)
    });

    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);
    let key = format!("{}:{}:{}", vw, props.display, props.content);

    if *cache_key.read() != key {
        cache_key.set(key);
        transmitted.set(false);
        data_cache.set(Vec::new());
        cols.set(0);
        rows.set(0);
        drawn_at.set((-1, -1));
        error_msg.set(None);
        loading.set(false);
        load_result.set(Arc::new(Mutex::new(None)));
    }

    if error_msg.read().is_none() && data_cache.read().is_empty() {
        if !*loading.read() {
            loading.set(true);

            let result_shared = load_result.read().clone();
            let cell_w = caps_cache
                .read()
                .clone()
                .unwrap_or_default()
                .cell_w_px
                .max(1) as u32;
            let max_w = (cell_w * 2).max(16) * vw;
            let content = props.content.clone();
            let display = props.display;

            std::thread::spawn(move || {
                let result =
                    render_math(&content, display, max_w, true).map_err(|e| format!("{:#}", e));
                let mut guard = result_shared.lock().unwrap();
                *guard = Some(result);
            });
        }

        let maybe_result = {
            let arc = load_result.read().clone();
            let mut guard = arc.lock().unwrap();
            guard.take()
        };

        if let Some(result) = maybe_result {
            match result {
                Ok((png_data, img_w, img_h)) => {
                    data_cache.set(png_data);
                    let mut cols_ = img_w;
                    let mut rows_ = img_h;
                    let caps = caps_cache.read().clone().unwrap_or_default();

                    cols_ = ((cols_ as f32) / (caps.cell_w_px as f32)).ceil() as u32;
                    cols_ = cols_.min(vw);
                    rows_ = ((rows_ as f32) / (caps.cell_h_px as f32)).ceil() as u32;
                    rows_ = rows_.min(vh);

                    cols.set(cols_);
                    rows.set(rows_);

                    drawn_at.set((-1, -1));
                }
                Err(err_str) => {
                    error_msg.set(Some(err_str));
                    loading.set(false);
                }
            }
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
            let visible_rows = img_rows.min(term_height as i32 - y - 1).max(0);

            // How many rows are scrolled off the top
            let top_clip_rows = if y < 0 { (-y).min(img_rows) } else { 0 };
            let actual_vis_rows = (visible_rows - top_clip_rows).max(0);
            let render_y = if y < 0 { 0 } else { y };

            let visible = x >= 0 && actual_vis_rows > 0 && visible_cols > 0;

            if visible && !data_cache.read().is_empty() {
                let id = if *transmitted.read() {
                    image_id.read().id()
                } else {
                    let new_id = kitty::next_placement_id();
                    image_id.write().set_id(new_id);
                    new_id
                };

                if let Some(ref tx) = *io_tx.read() {
                    if !*transmitted.read() {
                        let data = data_cache.read().clone();
                        let _ = tx.send(MathCmd::Render {
                            id,
                            data,
                            x,
                            y: render_y,
                            vis_cols: visible_cols,
                            vis_rows: actual_vis_rows,
                            src_y_offset: top_clip_rows,
                            cell_w: caps.cell_w_px as u32,
                            cell_h: caps.cell_h_px as u32,
                        });
                        transmitted.set(true);
                    } else {
                        let _ = tx.send(MathCmd::Place {
                            id,
                            x,
                            y: render_y,
                            vis_cols: visible_cols,
                            vis_rows: actual_vis_rows,
                            src_y_offset: top_clip_rows,
                            cell_w: caps.cell_w_px as u32,
                            cell_h: caps.cell_h_px as u32,
                        });
                    }
                }
            } else if !visible && *transmitted.read() {
                let id = image_id.read().id();
                if id != 0 {
                    if let Some(ref tx) = *io_tx.read() {
                        let _ = tx.send(MathCmd::Detach(id));
                    }
                }
            }
        }
    }

    if let Some(err) = error_msg.read().clone() {
        return element! {
            View() {
                Text(content: err, color: theme::RED)
            }
        }
        .into_any();
    }

    element! {View(width: cols.read().clone(), height: rows.read().clone())}.into_any()
}
