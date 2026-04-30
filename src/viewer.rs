use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::cursor;
use crossterm::event::KeyEventKind;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend;
use ratatui::prelude::CrosstermBackend;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::io::Write;
use std::io::{Stdout, stdout};
use std::result;
use std::time::Duration;
use std::u16;

use crate::assets::cache::AssetCache;
use crate::document::parser::parse_markdown;
use crate::output::capabilities::TermCaps;
use crate::output::kitty::KittyWriter;
use crate::render::text::PendingImage;
use crate::render::text::render_document;
use crate::render::theme::Theme;

pub struct Viewer {
    content: String,
    scroll: u16,
    prev_scroll: u16,
    needs_image_redraw: bool,
    theme: Theme,
    caps: TermCaps,
    asset_cache: AssetCache,
    kitty: KittyWriter<Stdout>,
    pending_images: Vec<PendingImage>,
    preview_lines: Vec<Line<'static>>,
}

impl Viewer {
    pub fn new(content: String, theme: Theme) -> Result<Self> {
        let caps = TermCaps::detect()?;
        Ok(Self {
            content,
            scroll: 0,
            prev_scroll: u16::MAX,
            needs_image_redraw: true,
            theme,
            caps,
            asset_cache: AssetCache::new(),
            kitty: KittyWriter::new(stdout()),
            pending_images: Vec::new(),
            preview_lines: Vec::new(),
        })
    }

    fn rebuild(&mut self) {
        let doc = parse_markdown(&self.content);
        let rendered = render_document(
            &doc.blocks,
            &self.theme,
            &mut self.asset_cache,
            &self.caps,
            None,
            &mut self.kitty,
        );
        self.pending_images = rendered.images;
        self.preview_lines = rendered.lines;
        self.needs_image_redraw = false;
    }

    fn draw(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        let lines = self.preview_lines.clone();
        let scroll = self.scroll;

        terminal.draw(|frame| {
            let area = frame.area();
            let paragraph = Paragraph::new(lines)
                .style(Style::default())
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0));
            frame.render_widget(paragraph, area);
        })?;

        if self.scroll != self.prev_scroll || self.needs_image_redraw {
            self.place_visible_images()?;
            self.prev_scroll = self.scroll;
            self.needs_image_redraw = false;
        }
        Ok(())
    }

    fn place_visible_images(&mut self) -> Result<()> {
        if !self.caps.has_kitty || self.pending_images.is_empty() {
            return Ok(());
        };

        let _ = self.kitty.delete_all();
        let visible_rows = self.caps.rows;

        for img in &self.pending_images {
            let screen_row = (img.row as i32) - (self.scroll as i32);
            if screen_row < 0 || screen_row >= visible_rows as i32 {
                continue;
            }

            let image_cols = ((img.width_px as f32) / (self.caps.cell_w_px as f32)).ceil() as u16;
            let display_cols = image_cols.min(self.caps.cols);
            let display_rows = img
                .rows
                .min((visible_rows as i32 - screen_row.max(0)) as u16);
            let _ = self.kitty.move_cursor(0, screen_row as u16);
            let _ = self.kitty.display_png(
                &img.png_data,
                img.image_id,
                Some(display_cols),
                Some(display_rows),
            );
        }
        let _ = stdout().flush();
        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;

        stdout().execute(EnterAlternateScreen);
        stdout().execute(event::EnableMouseCapture);
        stdout().execute(cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        self.rebuild();
        let result = self.event_loop(&mut terminal);

        // Clean up
        let _ = self.kitty.delete_all();
        let _ = stdout().execute(cursor::Show);
        let _ = stdout().execute(event::DisableMouseCapture);
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();

        result
    }

    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        // Parse and render once (re-render on resize)

        loop {
            // Draw
            self.draw(terminal)?;

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        if k.modifiers.contains(KeyModifiers::CONTROL)
                            && k.code == KeyCode::Char('c')
                        {
                            break;
                        }
                        match k.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Down | KeyCode::Char('j') => {
                                self.scroll = self.scroll.saturating_add(1);
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                self.scroll = self.scroll.saturating_sub(1);
                            }
                            KeyCode::PageDown | KeyCode::Char(' ') => {
                                let page = self.caps.rows.saturating_sub(2);
                                self.scroll = self.scroll.saturating_add(page);
                            }
                            KeyCode::PageUp => {
                                let page = self.caps.rows.saturating_sub(2);
                                self.scroll = self.scroll.saturating_sub(page);
                            }
                            KeyCode::Home | KeyCode::Char('g') => {
                                self.scroll = 0;
                            }
                            KeyCode::End | KeyCode::Char('G') => {
                                self.scroll = (self.preview_lines.len() as u16)
                                    .saturating_sub(self.caps.rows);
                            }
                            _ => {}
                        }
                    }
                    Event::Mouse(m) => match m.kind {
                        MouseEventKind::ScrollDown => {
                            self.scroll = self.scroll.saturating_add(3);
                        }
                        MouseEventKind::ScrollUp => {
                            self.scroll = self.scroll.saturating_sub(3);
                        }
                        _ => {}
                    },
                    Event::Resize(_, _) => {
                        self.caps = TermCaps::detect()?;
                        self.rebuild();
                    }

                    _ => {}
                }
            }
        }

        Ok(())
    }
}
