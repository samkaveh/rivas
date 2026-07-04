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

static NEXT_PLACEMENT_ID: AtomicU32 = AtomicU32::new(1);

pub fn next_placement_id() -> u32 {
    NEXT_PLACEMENT_ID.fetch_add(1, Ordering::Relaxed) & 0x00FF_FFFF
}

// --- helpers ---

fn crop_string(src_x: u32, src_y: u32, src_w: u32, src_h: u32) -> String {
    match (src_w > 0 || src_h > 0, src_x > 0 || src_y > 0) {
        (true, _) => format!(",x={},y={},w={},h={}", src_x, src_y, src_w, src_h),
        (false, true) => format!(",x={},y={}", src_x, src_y),
        (false, false) => String::new(),
    }
}

fn chunked_write<W: Write>(w: &mut W, first_control: &str, rest_control: &str, data: &str) {
    let bytes = data.as_bytes();
    let mut offset = 0;
    let len = bytes.len();
    while offset < len {
        let end = (offset + CHUNK_SIZE).min(len);
        let chunk = std::str::from_utf8(&bytes[offset..end]).unwrap();
        let more = if end < len { 1 } else { 0 };
        if offset == 0 {
            write!(w, "\x1b_G{},m={},q=2;{}\x1b\\", first_control, more, chunk).unwrap();
        } else if rest_control.is_empty() {
            write!(w, "\x1b_Gm={};{}\x1b\\", more, chunk).unwrap();
        } else {
            write!(w, "\x1b_G{},m={};{}\x1b\\", rest_control, more, chunk).unwrap();
        }
        offset = end;
    }
}

// --- raw-data API (base64-encode internally on every call) ---

pub fn write_to_cropped<W: Write>(
    w: &mut W,
    id: u32,
    png_data: &[u8],
    cols: u32,
    rows: u32,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);
    let crop = crop_string(src_x, src_y, src_w, src_h);
    chunked_write(
        w,
        &format!("a=T,f=100,t=d,i={},c={},r={}{}", id, cols, rows, crop),
        "",
        &encoded,
    );
}

pub fn write_animation_frames<W: Write>(w: &mut W, id: u32, frames: &[(Vec<u8>, u32)]) {
    for (png_data, delay_ms) in frames {
        let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);
        chunked_write(
            w,
            &format!("a=f,f=100,i={},z={}", id, delay_ms),
            "a=f",
            &encoded,
        );
    }
}

// --- pre-encoded API (for cached base64 data) ---

pub fn write_to_cropped_encoded<W: Write>(
    w: &mut W,
    id: u32,
    encoded: &str,
    cols: u32,
    rows: u32,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
) {
    let crop = crop_string(src_x, src_y, src_w, src_h);
    chunked_write(
        w,
        &format!("a=T,f=100,t=d,i={},c={},r={}{}", id, cols, rows, crop),
        "",
        encoded,
    );
}

pub fn write_animation_frames_encoded<W: Write>(w: &mut W, id: u32, frames: &[(&str, u32)]) {
    for (encoded, delay_ms) in frames {
        chunked_write(
            w,
            &format!("a=f,f=100,i={},z={}", id, delay_ms),
            "a=f",
            encoded,
        );
    }
}

// --- commands ---

pub fn start_animation<W: Write>(w: &mut W, id: u32) {
    write!(w, "\x1b_Ga=a,i={},s=3,v=1,q=2;\x1b\\", id).unwrap();
}

/// Delete placements only (lowercase d=i). Keeps image data cached so a=p can
/// re-display it without retransmission.
pub fn delete_placements<W: Write>(w: &mut W, id: u32) {
    write!(w, "\x1b_Ga=d,d=i,i={},q=2;\x1b\\", id).unwrap();
}

/// Delete placements AND free image data (uppercase d=I).
pub fn delete_image<W: Write>(w: &mut W, id: u32) {
    write!(w, "\x1b_Ga=d,d=I,i={},q=2;\x1b\\", id).unwrap();
}

/// Alias for delete_image — used by ImageGuard for cleanup on drop.
pub fn delete_by_id<W: Write>(w: &mut W, id: u32) {
    delete_image(w, id);
}

pub fn delete_all<W: Write>(w: &mut W) {
    write!(w, "\x1b_Ga=d,d=a,q=2;\x1b\\").unwrap();
}

/// Place an already-transmitted image at the cursor position without retransmitting data.
/// `placement_id` is the `p` key — if another placement with the same image+placement IDs
/// exists, it is replaced (moved/resized) instead of creating a new one.
pub fn place_image<W: Write>(
    w: &mut W,
    id: u32,
    placement_id: u32,
    cols: u32,
    rows: u32,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
) {
    let crop = crop_string(src_x, src_y, src_w, src_h);
    write!(
        w,
        "\x1b_Ga=p,i={},p={},c={},r={}{},q=2;\x1b\\",
        id, placement_id, cols, rows, crop
    )
    .unwrap();
}

// --- ImageGuard ---

pub struct ImageGuard {
    id: u32,
}

impl ImageGuard {
    pub fn new() -> Self {
        Self { id: 0 }
    }

    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    pub fn set(&mut self, id: u32) {
        if self.id != 0 && self.id != id {
            let mut stdout = std::io::stdout().lock();
            delete_by_id(&mut stdout, self.id);
            let _ = stdout.flush();
        }
        self.id = id;
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn clear(&mut self) {
        if self.id != 0 {
            let mut stdout = std::io::stdout().lock();
            delete_by_id(&mut stdout, self.id);
            let _ = stdout.flush();
            self.id = 0;
        }
    }
}

impl Drop for ImageGuard {
    fn drop(&mut self) {
        self.clear();
    }
}
