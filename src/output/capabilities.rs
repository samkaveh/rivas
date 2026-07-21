use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global kitty support flag. Set once during `TermCaps::detect()`, can be
/// overridden by `--force-kitty`.
static HAS_KITTY: AtomicBool = AtomicBool::new(false);

/// Returns `true` if the terminal supports the Kitty graphics protocol.
/// Checks the global flag set during capability detection (or overridden by
/// `--force-kitty`).
pub fn has_kitty() -> bool {
    HAS_KITTY.load(Ordering::Relaxed)
}

/// Force-set the global kitty support flag. Used by `--force-kitty`.
pub fn force_kitty() {
    HAS_KITTY.store(true, Ordering::Relaxed);
}

#[derive(Clone, Debug)]
pub struct TermCaps {
    pub cols: u16,
    pub rows: u16,
    pub cell_w_px: u16,
    pub cell_h_px: u16,
    pub has_kitty: bool,
}

impl Default for TermCaps {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            cell_w_px: 8,
            cell_h_px: 16,
            has_kitty: true,
        }
    }
}

impl TermCaps {
    pub fn detect() -> Result<Self> {
        let (cols, rows) = crossterm::terminal::size()?;
        let (cell_w_px, cell_h_px) = cell_pixel_size().unwrap_or((8, 16));
        let kitty = detect_kitty();
        HAS_KITTY.store(kitty, Ordering::Relaxed);
        Ok(Self {
            cols,
            rows,
            cell_w_px,
            cell_h_px,
            has_kitty: kitty,
        })
    }

    /// returns how many terminal rows an images of height_px occupy.
    pub fn image_rows(&self, height_px: u32) -> u16 {
        ((height_px as f32 / self.cell_h_px as f32).ceil() as u16).max(1)
    }

    /// Pixel area of the content.
    pub fn content_width_px(&self) -> u32 {
        self.cols as u32 * self.cell_w_px as u32
    }
}

#[cfg(windows)]
fn cell_pixel_size() -> Option<(u16, u16)> {
    None
}

#[cfg(unix)]
fn cell_pixel_size() -> Option<(u16, u16)> {
    unsafe {
        let mut ws: std::mem::MaybeUninit<libc::winsize> = std::mem::MaybeUninit::uninit();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, ws.as_mut_ptr()) == 0 {
            let ws = ws.assume_init();
            if ws.ws_xpixel > 0 && ws.ws_col > 0 {
                return Some((ws.ws_xpixel / ws.ws_col, ws.ws_ypixel / ws.ws_row));
            }
        }
    }
    None
}

fn detect_kitty() -> bool {
    let t = std::env::var("TERM").unwrap_or_default();
    let p = std::env::var("TERM_PROGRAM").unwrap_or_default();
    t.contains("kitty")
        || ["kitty", "wezterm", "ghostty"]
            .iter()
            .any(|k| p.eq_ignore_ascii_case(k))
}
