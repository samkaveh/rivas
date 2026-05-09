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
        View(flex_direction: FlexDirection::Column, padding: 1) {
            #(props.title.clone().map(|title| element! {
            View(margin_bottom: 1) {
                Text(content: title, color: Color::DarkGrey)
            }
            }))

            View(margin_bottom: 1) {
            KittyImage(url: props.url.clone(),file_path: props.file_path.clone(), viewport_height: props.viewport_height, viewport_width: props.viewport_width)
            }
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
    let mut drawn_at = hooks.use_state(|| (-1i32, -1i32));
    let image_id = hooks.use_state(|| kitty::next_placement_id()).get();
    let mut data_cache = hooks.use_ref(|| Vec::<u8>::new());
    let mut cols = hooks.use_ref(|| 0u32);
    let mut rows = hooks.use_ref(|| 0u32);

    let url = &props.url;
    let base_dir = props.file_path.parent();

    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);

    if data_cache.read().is_empty() {
        let loaded_image = match load_image_to_png(url.as_str(), base_dir, vw) {
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
        cols_ = cols_.min(vw);
        rows_ = ((rows_ as f32) / (caps.cell_h_px as f32)).ceil() as u32;
        rows_ = rows_.min(vh);

        cols.set(cols_);
        rows.set(rows_);
    }

    let view_h = props.viewport_height.unwrap_or(999) as i32;
    let view_w = props.viewport_width.unwrap_or(999) as i32;

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

            let (x, y) = pos;
            let img_cols = *cols.read() as i32;
            let img_rows = *rows.read() as i32;

            // How many cols/rows are actually visible
            let visible_cols = (img_cols).min(view_w - x).max(0);
            let visible_rows = (img_rows).min(view_h - y - 1).max(0);

            let visible = x >= 0 && y >= 0 && visible_cols > 0 && visible_rows > 0;

            render_image((pos, visible, visible_cols, visible_rows));
        }
    }

    element! {View(width: cols.read().clone(), height: rows.read().clone())}
}
