use crate::assets::images::load_image_to_png;
use base64::{Engine, write};
use iocraft::prelude::*;
use ratatui::macros::row;
use std::{io::Write, time::Duration};

fn supports_kitty_graphics() -> bool {
    if let Ok(term) = std::env::var("TERM_PROGRAM") {
        return matches!(term.as_str(), "kitty" | "WezTerm" | "ghostty");
    }
    if let Ok(term) = std::env::var("TERM") {
        return term.contains("kitty");
    }
    false
}
/// cols and rows determine how many cells the image should occupy.
fn write_kitty_image_to<W: Write>(w: &mut W, png_data: &[u8], cols: u32, rows: u32) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);
    let chunk_size = 4096;
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(chunk_size).collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let more = if i < chunks.len() - 1 { 1 } else { 0 };
        let chunk_str = std::str::from_utf8(*chunk).unwrap();

        if i == 0 {
            // first chunk include control patterns for kitty
            // a=T: transmit and display
            // f=100: PNG format
            // t=d direct (data in payload)
            // c=cols, r=rows: display size in cells
            // m=more: 1 if more chunks follows, 0 if not
            // q=2: quiet
            write!(
                w,
                "\x1b_Ga=T,f=100,t=d,c={},r={},m={},q=2;{}\x1b\\",
                cols, rows, more, chunk_str
            )
            .unwrap();
        } else {
            write!(w, "\x1b_Gm={};{}\x1b\\", more, chunk_str).unwrap();
        }
    }
    //w.flush().unwrap();
}

/// cols and rows determine how many cells the image should occupy.
fn write_kitty_image_to_handle(handle: &StdoutHandle, png_data: &[u8], cols: u32, rows: u32) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);
    let chunk_size = 4096;
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(chunk_size).collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let more = if i < chunks.len() - 1 { 1 } else { 0 };
        let chunk_str = std::str::from_utf8(*chunk).unwrap();

        if i == 0 {
            // first chunk include control patterns for kitty
            // a=T: transmit and display
            // f=100: PNG format
            // t=d direct (data in payload)
            // c=cols, r=rows: display size in cells
            // m=more: 1 if more chunks follows, 0 if not
            // q=2: quiet
            handle.print(format!(
                "\x1b_Ga=T,f=100,t=d,c={},r={},m={},q=2;{}\x1b\\",
                cols, rows, more, chunk_str
            ));
        } else {
            handle.print(format!("\x1b_Gm={};{}\x1b\\", more, chunk_str));
        }
    }
}

fn write_kitty_image(handle: &StdoutHandle, png_data: &[u8], cols: u32, rows: u32) {
    write_kitty_image_to_handle(handle, png_data, cols, rows);
}
#[derive(Default, Props)]
pub struct ImageProps {
    pub url: String,
    pub title: Option<String>,
    pub alt: Option<String>,
}

#[component]
pub fn Image(props: &ImageProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut should_exit = hooks.use_state(|| false);
    let (stdout_handle, _) = hooks.use_output();
    let mut image_sent = hooks.use_state(|| false);
    let url = props.url.clone();

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1) {
            View(margin_bottom: 1) {
                Text(content: "The image:", color: Color::DarkGrey)
            }
            KittyImage(url)
            View(border_style: BorderStyle::Round, border_color: Color::Green) {
                Text(content: " The image is rendered ^^", color: Color::Green)
            }
            View(margin_top: 1) {
                Text(content: "The image is rerendered on each loop.", color: Color::DarkGrey)
            }
            View(margin_top: 1) {
                Text(content: "Press q to quit", color: Color::DarkBlue)
            }
        }
    }
}

#[derive(Default, Props)]
pub struct KittyImageProps {
    pub url: String,
}

#[component]
pub fn KittyImage(props: &KittyImageProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let rect = hooks.use_component_rect();
    let mut last_pos = hooks.use_state(|| (0i32, 0i32));
    let mut data_cach = hooks.use_state(|| Vec::<u8>::new());
    let url = props.url.clone();
    let mut cols = 2;
    let mut rows = 2;
    if data_cach.read().is_empty() {
        let loaded_image = load_image_to_png(url.as_str(), None, 200).unwrap();
        //cols = loaded_image.1 / 100;
        //rows = loaded_image.2 / 100;
        data_cach.set(loaded_image.0.clone());
    }

    if let Some(r) = rect {
        let new_pos = (r.left, r.top);
        if new_pos != last_pos.get() {
            last_pos.set(new_pos);
        }
    }

    hooks.use_future(async move {
        loop {
            smol::Timer::after(Duration::from_millis(20)).await;

            let (x, y) = last_pos.get();
            let data = data_cach.read().clone();

            if !data.is_empty() && supports_kitty_graphics() && (x > 0 || y > 0) {
                let mut stdout = std::io::stdout().lock();
                write!(stdout, "\x1b[s").unwrap();
                write!(stdout, "\x1b[{};{}H", y + 1, x + 1).unwrap();
                write_kitty_image_to(&mut stdout, &data, cols, rows);
                write!(stdout, "\x1b[u").unwrap();
                stdout.flush().unwrap();
            }
        }
    });

    element! {View(width: cols, height: rows)}
}
