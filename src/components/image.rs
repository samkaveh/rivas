use crate::output::kitty;
use crate::theme;
use crate::{
    assets::images::{load_image, ImageData},
    output::capabilities::TermCaps,
};
use iocraft::prelude::*;
use std::{
    io::Write,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};

enum IoCmd {
    Render {
        id: u32,
        data: Arc<String>,
        frames: Vec<(Arc<String>, u32)>,
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

#[derive(Default, Props)]
pub struct ImageProps {
    pub url: String,
    pub file_path: PathBuf,
    pub title: Option<String>,
    pub alt: Option<String>,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn Image(props: &ImageProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_direction: FlexDirection::Column, margin_bottom: 1) {
            #(props.title.clone().map(|title| element! {
            View() {
                Text(content: title, color: theme::COMMENT)
            }
            }))
            KittyImage(url: props.url.clone(), file_path: props.file_path.clone(), viewport_height: props.viewport_height, viewport_width: props.viewport_width)
        }
    }
}

#[derive(Default, Props)]
pub struct KittyImageProps {
    pub url: String,
    pub file_path: PathBuf,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
}

#[component]
pub fn KittyImage(props: &KittyImageProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let rect = hooks.use_component_rect();
    let (term_width, term_height) = hooks.use_terminal_size();
    let mut drawn_at = hooks.use_state(|| (-1i32, -1i32));
    let mut image_id = hooks.use_ref(|| kitty::ImageGuard::new());
    let mut data_cache = hooks.use_ref(|| Arc::new(String::new()));
    let mut frames_cache = hooks.use_ref(|| Vec::<(Arc<String>, u32)>::new());
    let mut cache_key = hooks.use_ref(String::new);
    let mut cols = hooks.use_ref(|| 0u32);
    let mut rows = hooks.use_ref(|| 0u32);
    let mut error_msg = hooks.use_state(|| None::<String>);
    let mut loading = hooks.use_ref(|| false);
    let mut load_result = hooks.use_ref(|| Arc::new(Mutex::new(None::<Result<(String, Vec<(String, u32)>, u32, u32), String>>)));
    let mut transmitted = hooks.use_ref(|| false);
    let caps_cache = hooks.use_ref(|| TermCaps::detect().ok());
    let io_tx = hooks.use_ref(|| {
        let (tx, rx) = mpsc::channel::<IoCmd>();
        std::thread::spawn(move || {
            let mut last_id = 0u32;
            while let Ok(mut cmd) = rx.recv() {
                // Drain stale commands — skip intermediate positions
                // so rapid scrolling only processes the latest.
                while let Ok(next) = rx.try_recv() {
                    cmd = next;
                }

                let mut stdout = std::io::stdout().lock();
                match cmd {
                    IoCmd::Render {
                        id,
                        data,
                        frames,
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
                        kitty::write_to_cropped_encoded(
                            &mut stdout, id, data.as_str(),
                            vis_cols as u32, vis_rows as u32,
                            0, src_y_px, crop_w_px, crop_h_px,
                        );

                        if !frames.is_empty() {
                            let frames_ref: Vec<(&str, u32)> = frames.iter()
                                .map(|(s, d)| (s.as_str(), *d))
                                .collect();
                            kitty::write_animation_frames_encoded(
                                &mut stdout, id, &frames_ref,
                            );
                            kitty::start_animation(&mut stdout, id);
                        }

                        last_id = id;

                        write!(stdout, "\x1b8").unwrap();
                        stdout.flush().unwrap();
                    }
                    IoCmd::Place {
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
                            &mut stdout, id,
                            vis_cols as u32, vis_rows as u32,
                            0, src_y_px, crop_w_px, crop_h_px,
                        );

                        write!(stdout, "\x1b8").unwrap();
                        stdout.flush().unwrap();
                    }
                    IoCmd::Detach(id) => {
                        if id != 0 {
                            kitty::delete_placements(&mut stdout, id);
                            stdout.flush().unwrap();
                        }
                    }
                    IoCmd::Free(id) => {
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

    let url = &props.url;
    let base_dir = props.file_path.parent();
    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);
    let key = format!("{}:{}", vw, url);

    if *cache_key.read() != key {
        cache_key.set(key.clone());
        data_cache.set(Arc::new(String::new()));
        frames_cache.set(Vec::new());
        cols.set(0);
        rows.set(0);
        drawn_at.set((-1, -1));
        error_msg.set(None);
        loading.set(false);
        transmitted.set(false);
        load_result.set(Arc::new(Mutex::new(None)));
    }

    if error_msg.read().is_none() && data_cache.read().is_empty() {
        if !*loading.read() {
            loading.set(true);

            let result_shared = load_result.read().clone();
            let max_w = (vw as f32 * 100.0).round() as u32;
            let url = url.clone();
            let base_dir = base_dir.map(|p| p.to_path_buf());

            std::thread::spawn(move || {
                let encoded = load_image(&url, base_dir.as_deref(), max_w)
                    .map(|data| {
                        use base64::Engine;
                        let w = data.width();
                        let h = data.height();
                        match data {
                            ImageData::Png(raw, _, _) => {
                                let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
                                (b64, Vec::new(), w, h)
                            }
                            ImageData::Gif { frames, .. } => {
                                let first = base64::engine::general_purpose::STANDARD.encode(&frames[0].0);
                                let rest = frames[1..]
                                    .iter()
                                    .map(|(png, delay)| {
                                        (base64::engine::general_purpose::STANDARD.encode(png), *delay)
                                    })
                                    .collect();
                                (first, rest, w, h)
                            }
                        }
                    })
                    .map_err(|e| format!("{:#}", e));
                let mut guard = result_shared.lock().unwrap();
                *guard = Some(encoded);
            });
        }

        let maybe_result = {
            let arc = load_result.read().clone();
            let mut guard = arc.lock().unwrap();
            guard.take()
        };

        if let Some(result) = maybe_result {
            match result {
                Ok((b64_data, rest_frames, img_w, img_h)) => {
                    data_cache.set(Arc::new(b64_data));
                    frames_cache.set(rest_frames.into_iter().map(|(s, d)| (Arc::new(s), d)).collect());

                    let caps = caps_cache.read().clone().unwrap_or_default();
                    let mut cols_ = img_w;
                    let mut rows_ = img_h;
                    cols_ = ((cols_ as f32) / (caps.cell_w_px.max(1) as f32)).ceil() as u32;
                    cols_ = cols_.min((vw as f32).round() as u32);
                    rows_ = ((rows_ as f32) / (caps.cell_h_px.max(1) as f32)).ceil() as u32;
                    rows_ = rows_.min(vh);
                    cols.set(cols_);
                    rows.set(rows_);

                    drawn_at.set((-1, -1));
                }
                Err(err_str) => {
                    error_msg.set(Some(format!(
                        "Error: {err_str}, Base directory: {dir:?}",
                        dir = base_dir
                    )));
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
                        let data = Arc::clone(&data_cache.read());
                        let frames = frames_cache.read().clone();
                        let _ = tx.send(IoCmd::Render {
                            id,
                            data,
                            frames,
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
                        let _ = tx.send(IoCmd::Place {
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
                        let _ = tx.send(IoCmd::Detach(id));
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
