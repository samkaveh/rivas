use anyhow::{Ok, Result};
use crossterm::ExecutableCommand;
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend;
use ratatui::prelude::CrosstermBackend;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::io::{Stdout, stdout};
use std::time::Duration;

use crate::document::parser::parse_markdown;
use crate::output::capabilities::TermCaps;
use crate::output::kitty::KittyWriter;
use crate::render::text::render_document;
use crate::render::theme::Theme;

pub struct Viewer {
    content: String,
    scroll: u16,
    total_lines: u16,
    theme: Theme,
}

impl Viewer {
    pub fn new(content: String, theme: Theme) -> Result<Self> {
        Ok(Self {
            content,
            scroll: 0,
            total_lines: 0,
            theme,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;

        stdout().execute(EnterAlternateScreen);
        stdout().execute(event::EnableMouseCapture);
        stdout().execute(cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        // Clean up
        let _ = stdout().execute(cursor::Show);
        let _ = stdout().execute(event::DisableMouseCapture);
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();

        result
    }

    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        // Parse and render once (re-render on resize)
        let doc = parse_markdown(&self.content);
        let mut lines = render_document(&doc.blocks, &self.theme);
        self.total_lines = lines.len() as u16;

        loop {
            // Draw
            terminal.draw(|frame| {
                let area = frame.area();
                let paragraph = Paragraph::new(lines.clone())
                    .block(Block::default().borders(Borders::NONE))
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll, 0));
                frame.render_widget(paragraph, area);
            })?;

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(k) => {
                        if k.modifiers.contains(KeyModifiers::CONTROL)
                            && k.code == KeyCode::Char('c')
                        {
                            break;
                        }
                        match k.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Down | KeyCode::Char('j') => {
                                self.scroll_by(1);
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                self.scroll_by(-1);
                            }
                            KeyCode::PageDown | KeyCode::Char(' ') => {
                                let h = terminal.size()?.height;
                                self.scroll_by(h as i32 - 2);
                            }
                            KeyCode::PageUp => {
                                let h = terminal.size()?.height;
                                self.scroll_by(-(h as i32 - 2));
                            }
                            KeyCode::Home | KeyCode::Char('g') => {
                                self.scroll = 0;
                            }
                            KeyCode::End | KeyCode::Char('G') => {
                                let h = terminal.size()?.height;
                                self.scroll = self.total_lines.saturating_sub(h);
                            }
                            _ => {}
                        }
                    }
                    Event::Mouse(m) => match m.kind {
                        MouseEventKind::ScrollDown => {
                            self.scroll_by(3);
                        }
                        MouseEventKind::ScrollUp => {
                            self.scroll_by(-3);
                        }
                        _ => {}
                    },
                    Event::Resize(_, _) => {}

                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn scroll_by(&mut self, delta: i32) {
        let new = self.scroll as i32 + delta;
        self.scroll = new.max(0).min(self.total_lines.saturating_sub(1) as i32) as u16;
    }
}
