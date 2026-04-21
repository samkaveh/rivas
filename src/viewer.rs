use anyhow::{Ok, Result};
use cosmic_text::{FontSystem, SwashCache};
use crossterm::ExecutableCommand;
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use std::io::{Stdout, stdout};
use std::time::Duration;

use crate::document::parser::parse_markdown;
use crate::output::capabilities::TermCaps;
use crate::output::kitty::KittyWriter;
use crate::render::{layout::LayoutEngine, paint::pain_document, theme::Theme};

pub struct Viewer {
    content: String,
    scroll_y: f32,
    total_doc_height: f32,
    caps: TermCaps,
    theme: Theme,
    font_system: FontSystem,
    swash_cache: SwashCache,
    kitty: KittyWriter<Stdout>,
    current_image_id: Option<u32>,
    needs_redraw: bool,
}

impl Viewer {
    pub fn new(content: String, caps: TermCaps, theme: Theme) -> Result<Self> {
        Ok(Self {
            content,
            scroll_y: 0.0,
            total_doc_height: 0.0,
            caps,
            theme,
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            kitty: KittyWriter::new(stdout()),
            current_image_id: None,
            needs_redraw: true,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen);
        stdout().execute(event::EnableMouseCapture);
        stdout().execute(cursor::Hide)?;

        let result = self.event_loop();

        // Clean up
        let _ = self.kitty.delete_all();
        let _ = stdout().execute(cursor::Show);
        let _ = stdout().execute(event::DisableMouseCapture);
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();

        result
    }

    fn event_loop(&mut self) -> Result<()> {
        loop {
            if self.needs_redraw {
                self.render()?;
                self.needs_redraw = false;
            }
        }

        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        let (w, h) = self.caps.area_pixels(self.caps.cols, self.caps.rows);
        println!("{w},{h}");
        let doc = parse_markdown(&self.content);
        println!("{:?}", doc.blocks);
        let mut engine = LayoutEngine::new(&mut self.font_system, &self.theme, w as f32);
        let layout = engine.layout_all(&doc.blocks);

        // Track total document height for scroll clamping.
        if let Some(last) = layout.last() {
            self.total_doc_height = last.y + last.height + self.theme.padding;
        }

        let pixmap = pain_document(
            &layout,
            &self.theme,
            w as u32,
            h as u32,
            self.scroll_y,
            &mut self.font_system,
            &mut self.swash_cache,
        );

        let png = pixmap
            .encode_png()
            .map_err(|e| anyhow::anyhow!("PNG encode {e}"))?;

        if let Some(old) = self.current_image_id {
            self.kitty.delete_image(old)?;
        }

        let id = self.kitty.alloc_id();
        self.kitty.move_cursor(0, 0)?;
        self.kitty
            .display_png(&png, id, Some(self.caps.cols), Some(self.caps.rows))?;
        self.current_image_id = Some(id);
        Ok(())
    }
}
