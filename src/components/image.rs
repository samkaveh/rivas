use crate::output::kitty;
use crate::{assets::images::load_image_to_png, output::capabilities::TermCaps};
use iocraft::prelude::*;
use std::{io::Write, path::PathBuf};

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
                Text(content: title, color: crate::theme::COMMENT)
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
    let mut data_cache = hooks.use_ref(|| Vec::<u8>::new());
    let mut cache_key = hooks.use_ref(String::new);
    let mut cols = hooks.use_ref(|| 0u32);
    let mut rows = hooks.use_ref(|| 0u32);

    let url = &props.url;
    let base_dir = props.file_path.parent();

    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);

    let key = format!("{}:{}", vw, url);

    if *cache_key.read() != key {
        cache_key.set(key);
        data_cache.set(Vec::new());
        cols.set(0);
        rows.set(0);
        drawn_at.set((-1, -1));
    }

    if data_cache.read().is_empty() {
        let max_w = (vw as f32 * 100.0).round() as u32;
        let loaded_image = match load_image_to_png(url.as_str(), base_dir, max_w) {
            Ok(v) => v,
            Err(e) => {
                return element! {
                    View() {
                        Text(content: format!("Error: {:#}, Base directory: {:?}", e, base_dir))
                    }
                };
            }
        };
        data_cache.set(loaded_image.0);
        let mut cols_ = loaded_image.1;
        let mut rows_ = loaded_image.2;
        let caps = TermCaps::detect().unwrap();

        cols_ = ((cols_ as f32) / (caps.cell_w_px as f32)).ceil() as u32;
        cols_ = cols_.min((vw as f32).round() as u32);
        rows_ = ((rows_ as f32) / (caps.cell_h_px as f32)).ceil() as u32;
        rows_ = rows_.min(vh);

        cols.set(cols_);
        rows.set(rows_);
    }

    let render_image = hooks.use_async_handler(
        move |(pos, visible, vis_cols, vis_rows, src_y_offset, cell_w, cell_h): (
            (i32, i32),
            bool,
            i32,
            i32,
            i32,
            u32,
            u32,
        )| async move {
            if !kitty::is_supported() {
                return;
            }
            let mut stdout = std::io::stdout().lock();
            if visible && !data_cache.read().is_empty() {
                let (x, y) = pos;
                write!(stdout, "\x1b7").unwrap();
                write!(stdout, "\x1b[{};{}H", y + 1, x + 1).unwrap();

                // Source crop in pixels
                let src_y_px = src_y_offset as u32 * cell_h;
                let crop_h_px = vis_rows as u32 * cell_h;
                let crop_w_px = vis_cols as u32 * cell_w;

                let new_id = kitty::write_to_cropped(
                    &mut stdout,
                    &data_cache.read(),
                    vis_cols as u32,
                    vis_rows as u32,
                    0,         // src x offset px
                    src_y_px,  // src y offset px
                    crop_w_px, // src crop width px
                    crop_h_px, // src crop height px
                );
                image_id.write().set(new_id);
                write!(stdout, "\x1b8").unwrap();
            } else {
                image_id.write().clear();
            }
            stdout.flush().unwrap();
        },
    );

    if let Some(r) = rect {
        let mut pos = (r.left, r.top);
        if pos != drawn_at.get() {
            drawn_at.set(pos);

            let caps = TermCaps::detect().unwrap();
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

            render_image((
                (x, render_y),
                visible,
                visible_cols,
                actual_vis_rows,
                top_clip_rows, // src y offset in cells
                caps.cell_w_px as u32,
                caps.cell_h_px as u32,
            ));
        }
    }

    element! {View(width: cols.read().clone(), height: rows.read().clone())}
}
