use iocraft::prelude::*;
use std::io::Write;

use crate::{
    assets::mermaid::render_mermaid_to_png,
    output::{capabilities::TermCaps, kitty},
};

#[derive(Default, Props)]
pub struct MermaidBlockProps {
    pub source: String,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub scale: Option<f32>,
}

#[component]
pub fn MermaidBlock(props: &MermaidBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let scale = props.scale.unwrap_or(1.0);
    element! {
       View(flex_direction: FlexDirection::Column) {
           View(flex_direction: FlexDirection::Row, gap: 1, margin_bottom: 1) {
               View(background_color: Color::AnsiValue(238)) {
                   Text(content: " Mermaid ", weight: Weight::Bold)
               }
               View(background_color: Color::AnsiValue(240)) {
                   Text(content: " + ", color: Color::AnsiValue(255))
               }
               View(background_color: Color::AnsiValue(240)) {
                   Text(content: " - ", color: Color::AnsiValue(255))
               }
               Text(content: format!(" {:.1}x", scale), color: Color::AnsiValue(244))
           }
           KittyMermaid(source: props.source.clone(), viewport_height: props.viewport_height, viewport_width: props.viewport_width, scale: scale)
       }
    }
}

#[derive(Default, Props)]
pub struct KittyMermaidProps {
    pub source: String,
    pub viewport_height: Option<u32>,
    pub viewport_width: Option<u32>,
    pub scale: f32,
}

#[component]
pub fn KittyMermaid(props: &KittyMermaidProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let rect = hooks.use_component_rect();
    let (term_width, term_height) = hooks.use_terminal_size();
    let mut drawn_at = hooks.use_state(|| (-1i32, -1i32));
    let mut image_id = hooks.use_ref(|| kitty::ImageGuard::new());
    let mut data_cache = hooks.use_ref(|| Vec::<u8>::new());
    let mut cache_key = hooks.use_ref(String::new);
    let mut cols = hooks.use_ref(|| 0u32);
    let mut rows = hooks.use_ref(|| 0u32);

    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);
    let key = format!("{}:{}:{}", vw, props.scale, props.source);

    if *cache_key.read() != key {
        cache_key.set(key);
        data_cache.set(Vec::new());
        cols.set(0);
        rows.set(0);
        drawn_at.set((-1, -1));
    }

    if data_cache.read().is_empty() {
        let max_w = (2.0 * vw as f32 * props.scale).round() as u32;
        let loaded_image = match render_mermaid_to_png(&props.source, max_w) {
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
        cols_ = cols_.min((vw as f32 * props.scale).round() as u32);
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
            if visible && !data_cache.read().is_empty() {
                let (x, y) = pos;
                write!(stdout, "\x1b7").unwrap();
                write!(stdout, "\x1b[{};{}H", y + 1, x + 1).unwrap();
                let new_id = kitty::write_to(
                    &mut stdout,
                    &data_cache.read(),
                    vis_cols as u32, // clipped
                    vis_rows as u32, // clipped
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

            let img_cols = *cols.read() as i32;
            let img_rows = *rows.read() as i32;

            let (x, y) = pos;
            let visible_cols = img_cols.min(term_width as i32 - x).max(0);
            let mut visible_rows = img_rows.min(term_height as i32 - y - 1).max(0);

            let visible = x >= 0 && visible_cols > 0 && visible_rows > 0;
            if y < 0 && visible_rows >= 0 {
                visible_rows = (visible_rows + y).max(0);
                pos.1 = 0;
            }

            render_image((pos, visible, visible_cols, visible_rows));
        }
    }
    element! {View(width: cols.read().clone(), height: rows.read().clone())}
}
