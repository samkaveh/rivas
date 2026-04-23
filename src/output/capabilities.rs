use anyhow::Result;

pub struct TermCaps {
    pub cols: u16,
    pub rows: u16,
    pub cell_w: u16,
    pub cell_h: u16,
    pub has_kitty_graphics: bool,
}

impl TermCaps {
    pub fn detect() -> Result<Self> {
        let (cols, rows) = crossterm::terminal::size()?;
        let (cell_w, cell_h) = cell_pixel_size().unwrap_or((8, 16));
        Ok(Self {
            cols,
            rows,
            cell_w,
            cell_h,
            has_kitty_graphics: has_kitty(),
        })
    }
    pub fn area_pixels(&self, cols: u16, rows: u16) -> (u32, u32) {
        (
            cols as u32 * self.cell_w as u32,
            rows as u32 * self.cell_h as u32,
        )
    }
}

fn cell_pixel_size() -> Option<(u16, u16)> {
    None
}

fn has_kitty() -> bool {
    let t = std::env::var("TERM").unwrap_or_default();
    let p = std::env::var("TERM_PROGRAM").unwrap_or_default();
    t.contains("kitty")
        || ["kitty", "wezterm", "ghostty"]
            .iter()
            .any(|k| p.eq_ignore_ascii_case(k))
}
