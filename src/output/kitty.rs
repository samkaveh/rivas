use base64::Engine;
use std::{
    io::Write,
    sync::atomic::{AtomicU32, Ordering},
};

const CHUNK_SIZE: usize = 4096;

pub fn is_supported() -> bool {
    if let Ok(term) = std::env::var("TERM_PROGRAM") {
        return matches!(term.as_str(), "kitty" | "WezTerm" | "ghostty");
    }
    if let Ok(term) = std::env::var("TERM") {
        return term.contains("kitty");
    }
    false
}

pub fn write_to<W: Write>(w: &mut W, png_data: &[u8], cols: u32, rows: u32) -> u32 {
    let id = next_placement_id();
    write_with_id_to(w, png_data, cols, rows, id);
    id
}

static NEXT_PLACEMENT_ID: AtomicU32 = AtomicU32::new(1);

pub fn next_placement_id() -> u32 {
    NEXT_PLACEMENT_ID.fetch_add(1, Ordering::Relaxed) & 0x00FF_FFFF
}

pub fn write_with_id_to<W: Write>(w: &mut W, png_data: &[u8], cols: u32, rows: u32, id: u32) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);
    let chunk_size = CHUNK_SIZE;
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(chunk_size).collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let more = if i < chunks.len() - 1 { 1 } else { 0 };
        let chunk_str = std::str::from_utf8(chunk).unwrap();

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
                "\x1b_Ga=T,f=100,t=d,i={},c={},r={},m={},q=2;{}\x1b\\",
                id, cols, rows, more, chunk_str
            )
            .unwrap();
        } else {
            write!(w, "\x1b_Gm={};{}\x1b\\", more, chunk_str).unwrap();
        }
    }
}

pub fn delete_by_id<W: Write>(w: &mut W, id: u32) {
    write!(w, "\x1b_Ga=d,d=i,i={},q=2;\x1b\\", id).unwrap();
}

pub fn delete_all<W: Write>(w: &mut W) {
    write!(w, "\x1b_Ga=d,d=a,q=2;\x1b\\").unwrap();
}
