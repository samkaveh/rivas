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
}

#[component]
pub fn MermaidBlock(props: &MermaidBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    element! {
       KittyMermaid(source: props.source.clone(), viewport_height: props.viewport_height, viewport_width: props.viewport_width)
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
    let rect = hooks.use_component_rect();
    let mut drawn_at = hooks.use_state(|| (-1i32, -1i32));
    let image_id = hooks.use_state(|| kitty::next_placement_id()).get();
    let mut data_cache = hooks.use_ref(|| Vec::<u8>::new());
    let mut cols = hooks.use_ref(|| 0u32);
    let mut rows = hooks.use_ref(|| 0u32);

    let vw = props.viewport_width.unwrap_or(100);
    let vh = props.viewport_height.unwrap_or(100);

    if data_cache.read().is_empty() {
        let loaded_image = match render_mermaid_to_png(&props.source, 2 * vw) {
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

    let view_h = props.viewport_height.unwrap_or(999) as i32;
    let view_w = props.viewport_width.unwrap_or(999) as i32;

    let render_image =
        hooks.use_async_handler(move |(pos, visible): ((i32, i32), bool)| async move {
            //smol::Timer::after(Duration::from_millis(10)).await;

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
                    cols.read().clone(),
                    rows.read().clone(),
                );
                write!(stdout, "\x1b8").unwrap();
            }

            stdout.flush().unwrap();
        });

    if let Some(r) = rect {
        let pos = (r.left, r.top);
        if pos != drawn_at.get() {
            drawn_at.set(pos);

            let visible = pos.0 >= 0 && pos.1 >= 0 && pos.1 <= view_h && pos.0 <= view_w;
            render_image((pos, visible));
        }
    }

    element! {View(width: cols.read().clone(), height: rows.read().clone())}
}
