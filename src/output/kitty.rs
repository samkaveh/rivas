use base64::{Engine, engine::general_purpose::STANDARD as B64};
use std::{fmt::format, fs, io::Write};

const CHUNK_SIZE: usize = 4096;

pub struct KittyWriter<W: Write> {
    w: W,
    next_id: u32,
}

impl<W: Write> KittyWriter<W> {
    pub fn new(w: W) -> Self {
        Self { w, next_id: 1 }
    }

    /// Allocate a unique image ID.
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Transmit a PNG image and display it at the current cursor position.
    /// The terminal scales it to fit columns and rows.
    pub fn display_png(
        &mut self,
        png: &[u8],
        id: u32,
        cols: Option<u16>,
        rows: Option<u16>,
    ) -> std::io::Result<()> {
        let b64 = B64.encode(png);
        let chunks: Vec<&[u8]> = b64.as_bytes().chunks(CHUNK_SIZE).collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == chunks.len() - 1;
            let m = if is_last { 0 } else { 1 };
            let chunk_str = std::str::from_utf8(chunk).unwrap();

            if i == 0 {
                // First chunk - full control data
                let mut ctrl = format!("a=T,f=100,t=d,i={id},q=2,m={m}");
                if let Some(c) = cols {
                    ctrl += &format!(",c={c}");
                }
                if let Some(r) = rows {
                    ctrl += &format!(",r={r}");
                }

                write!(self.w, "\x1b_G={ctrl};{chunk_str}\x1b\\")?;
            } else {
                // Continuation - only m flag
                write!(self.w, "\x1b_Gm={m};{chunk_str}\x1b\\")?;
            }
        }

        self.w.flush()
    }

    /// Delete a specific image by ID (removes all its placements).
    pub fn delete_image(&mut self, id: u32) -> std::io::Result<()> {
        write!(self.w, "\x1b_Ga=d,d=i,i={id},q=2\x1b\\")?;
        self.w.flush()
    }
    /// Delete all images in the terminal.
    pub fn delete_all(&mut self) -> std::io::Result<()> {
        write!(self.w, "\x1b_Ga=d,d=a,q=2\x1b\\")?;
        self.w.flush()
    }
    /// Move the cursor to a cell position (0-indexed).
    pub fn move_cursor(&mut self, col: u16, row: u16) -> std::io::Result<()> {
        write!(self.w, "\x1b[{};{}H", row + 1, col + 1)
    }
}
