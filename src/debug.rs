use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Global debug JSON logging flag.
pub static DEBUG_MODE: AtomicBool = AtomicBool::new(false);

/// Global debug annotations (visual overlay) flag.
pub static ANNOTATIONS_MODE: AtomicBool = AtomicBool::new(false);

/// Timestamp of app start for relative ms values.
static mut START: Option<Instant> = None;

/// JSONL log writer, guarded by a Mutex.
static mut LOG_WRITER: Option<Mutex<BufWriter<File>>> = None;

pub fn init(logging: bool, annotations: bool) {
    DEBUG_MODE.store(logging, Ordering::Relaxed);
    ANNOTATIONS_MODE.store(annotations, Ordering::Relaxed);
    if logging {
        unsafe {
            START = Some(Instant::now());
        }
        let file = File::create("rivas-debug.jsonl").expect("failed to create rivas-debug.jsonl");
        unsafe {
            LOG_WRITER = Some(Mutex::new(BufWriter::new(file)));
        }
    }
}

pub fn is_enabled() -> bool {
    DEBUG_MODE.load(Ordering::Relaxed)
}

pub fn are_annotations_enabled() -> bool {
    ANNOTATIONS_MODE.load(Ordering::Relaxed)
}

pub fn elapsed_ms() -> u128 {
    unsafe { START.map(|t| t.elapsed().as_millis()).unwrap_or(0) }
}

pub fn log_event(event: &DebugEvent) {
    if !is_enabled() {
        return;
    }
    let mut payload = serde_json::to_vec(event).unwrap_or_default();
    payload.push(b'\n');
    unsafe {
        if let Some(ref w) = LOG_WRITER {
            if let Ok(mut guard) = w.lock() {
                let _ = guard.write_all(&payload);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Event types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct CursorPos {
    pub byte: usize,
    pub row: usize,
    pub col: usize,
}

#[derive(Serialize)]
pub struct ViewportInfo {
    pub w: u32,
    pub h: u32,
}

#[derive(Serialize)]
#[serde(tag = "event")]
pub enum DebugEvent {
    #[serde(rename = "render_tick")]
    RenderTick {
        ts: u128,
        cursor: CursorPos,
        scroll: i32,
        viewport: ViewportInfo,
        blocks: usize,
        mode: String,
    },
    #[serde(rename = "image_load")]
    ImageLoad {
        ts: u128,
        url: String,
        pixel_w: u32,
        pixel_h: u32,
        cell_cols: u32,
        cell_rows: u32,
        load_ms: u128,
    },
    #[serde(rename = "image_place")]
    ImagePlace {
        ts: u128,
        id: u32,
        x: i32,
        y: i32,
        cols: i32,
        rows: i32,
        src_y_offset: i32,
    },
    #[serde(rename = "image_detach")]
    ImageDetach { ts: u128, id: u32, reason: String },
    #[serde(rename = "block_layout")]
    BlockLayout {
        ts: u128,
        idx: usize,
        block_type: String,
        span_start: usize,
        span_end: usize,
        est_height: u32,
    },
    #[serde(rename = "scroll")]
    Scroll { ts: u128, old: i32, new: i32 },
    // ── Kitty protocol events ──────────────────────────────────────────────────
    #[serde(rename = "kitty_transmit")]
    KittyTransmit {
        ts: u128,
        id: u32,
        cols: u32,
        rows: u32,
        crop_x: u32,
        crop_y: u32,
        crop_w: u32,
        crop_h: u32,
        data_size: usize,
        has_animation: bool,
    },
    #[serde(rename = "kitty_place")]
    KittyPlace {
        ts: u128,
        id: u32,
        cols: u32,
        rows: u32,
        crop_x: u32,
        crop_y: u32,
        crop_w: u32,
        crop_h: u32,
    },
    #[serde(rename = "kitty_delete")]
    KittyDelete {
        ts: u128,
        id: u32,
        scope: String, // "placements" or "image" or "all"
    },
}
