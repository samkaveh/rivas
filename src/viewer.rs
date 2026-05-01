use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::cursor;
use crossterm::event::KeyEventKind;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::CrosstermBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::fs;
use std::io::Write;
use std::io::{Stdout, stdout};
use std::path::PathBuf;
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
    file_path: Option<PathBuf>,
    content: String,
    editor_lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    preview_scroll: u16,
    editor_scroll: u16,
    prev_preview_scroll: u16,
    needs_image_redraw: bool,
    needs_rebuild: bool,
    dirty: bool,
    mode: ViewMode,
    input_mode: InputMode,
    pending_g: bool,
    pending_d: bool,
    status: String,
    theme: Theme,
    caps: TermCaps,
    asset_cache: AssetCache,
    kitty: KittyWriter<Stdout>,
    pending_images: Vec<PendingImage>,
    preview_lines: Vec<Line<'static>>,
    preview_area: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ViewMode {
    Preview,
    InPlaceEdit,
    SideBySide,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputMode {
    Normal,
    Insert,
}

impl Viewer {
    pub fn new(content: String, file_path: Option<PathBuf>, theme: Theme) -> Result<Self> {
        let caps = TermCaps::detect()?;
        let editor_lines = split_editor_lines(&content);
        Ok(Self {
            file_path,
            content,
            editor_lines,
            cursor_line: 0,
            cursor_col: 0,
            preview_scroll: 0,
            editor_scroll: 0,
            prev_preview_scroll: u16::MAX,
            needs_image_redraw: true,
            needs_rebuild: true,
            dirty: false,
            mode: ViewMode::Preview,
            input_mode: InputMode::Normal,
            pending_g: false,
            pending_d: false,
            status: String::from("Preview: e edit, s split, q quit"),
            theme,
            caps,
            asset_cache: AssetCache::new(),
            kitty: KittyWriter::new(stdout()),
            pending_images: Vec::new(),
            preview_lines: Vec::new(),
            preview_area: Rect::default(),
        })
    }

    fn rebuild(&mut self, width_cols: u16) {
        self.content = join_editor_lines(&self.editor_lines);
        let doc = parse_markdown(&self.content);
        let mut caps = self.caps.clone();
        caps.cols = width_cols.max(1);
        let base_dir = self.file_path.as_deref().and_then(|path| path.parent());
        let rendered = render_document(
            &doc.blocks,
            &self.theme,
            &mut self.asset_cache,
            &caps,
            base_dir,
            &mut self.kitty,
        );
        self.pending_images = rendered.images;
        self.preview_lines = rendered.lines;
        self.needs_image_redraw = true;
        self.needs_rebuild = false;
    }

    fn draw(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        let preview_scroll = self.preview_scroll;
        let editor_scroll = self.editor_scroll;
        let mode = self.mode;
        let status = self.status_line();
        let editor_lines = self.editor_view_lines();
        let preview_lines = self.preview_lines.clone();
        let cursor = self.editor_cursor_position();
        let theme_text = self.theme.text;
        let status_style = status_style(self.theme.is_dark);
        let border_style = border_style(self.theme.is_dark);
        let mut next_preview_area = Rect::default();

        terminal.draw(|frame| {
            let area = frame.area();
            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(area);

            match mode {
                ViewMode::Preview => {
                    next_preview_area = vertical[0];
                    let paragraph = Paragraph::new(preview_lines)
                        .style(Style::default())
                        .wrap(Wrap { trim: false })
                        .scroll((preview_scroll, 0));
                    frame.render_widget(paragraph, vertical[0]);
                }
                ViewMode::InPlaceEdit => {
                    let paragraph = Paragraph::new(editor_lines)
                        .style(theme_text)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(border_style)
                                .title(" Edit "),
                        )
                        .wrap(Wrap { trim: false })
                        .scroll((editor_scroll, 0));
                    frame.render_widget(paragraph, vertical[0]);
                    if let Some((x, y)) = cursor_in_area(cursor, vertical[0], editor_scroll) {
                        frame.set_cursor_position((x, y));
                    }
                }
                ViewMode::SideBySide => {
                    let columns = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(vertical[0]);
                    let editor = Paragraph::new(editor_lines)
                        .style(theme_text)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(border_style)
                                .title(" Edit "),
                        )
                        .wrap(Wrap { trim: false })
                        .scroll((editor_scroll, 0));
                    frame.render_widget(editor, columns[0]);

                    next_preview_area = columns[1];
                    let preview = Paragraph::new(preview_lines)
                        .style(Style::default())
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(border_style)
                                .title(" Preview "),
                        )
                        .wrap(Wrap { trim: false })
                        .scroll((preview_scroll, 0));
                    frame.render_widget(preview, columns[1]);
                    if let Some((x, y)) = cursor_in_area(cursor, columns[0], editor_scroll) {
                        frame.set_cursor_position((x, y));
                    }
                }
            }

            frame.render_widget(Paragraph::new(status).style(status_style), vertical[1]);
        })?;

        self.preview_area = next_preview_area;
        if self.mode == ViewMode::InPlaceEdit {
            let _ = self.kitty.delete_all();
            self.prev_preview_scroll = u16::MAX;
            return Ok(());
        }

        if self.preview_scroll != self.prev_preview_scroll || self.needs_image_redraw {
            self.place_visible_images()?;
            self.prev_preview_scroll = self.preview_scroll;
            self.needs_image_redraw = false;
        }
        Ok(())
    }

    fn place_visible_images(&mut self) -> Result<()> {
        if !self.caps.has_kitty || self.pending_images.is_empty() {
            return Ok(());
        };

        let _ = self.kitty.delete_all();
        let area = self.preview_area;
        if area.width == 0 || area.height == 0 {
            return Ok(());
        }
        let visible_rows = area.height;
        let content_col = if self.mode == ViewMode::SideBySide {
            area.x.saturating_add(1)
        } else {
            area.x
        };
        let content_row = if self.mode == ViewMode::SideBySide {
            area.y.saturating_add(1)
        } else {
            area.y
        };
        let content_width = if self.mode == ViewMode::SideBySide {
            area.width.saturating_sub(2)
        } else {
            area.width
        };
        let content_height = if self.mode == ViewMode::SideBySide {
            visible_rows.saturating_sub(2)
        } else {
            visible_rows
        };

        for img in &self.pending_images {
            let screen_row = (img.row as i32) - (self.preview_scroll as i32);
            if screen_row < 0 || screen_row >= content_height as i32 {
                continue;
            }

            let image_cols = ((img.width_px as f32) / (self.caps.cell_w_px as f32)).ceil() as u16;
            let display_cols = image_cols.min(content_width);
            let display_rows = img
                .rows
                .min((content_height as i32 - screen_row.max(0)) as u16);
            let _ = self
                .kitty
                .move_cursor(content_col, content_row + screen_row as u16);
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

        let _ = stdout().execute(EnterAlternateScreen);
        let _ = stdout().execute(event::EnableMouseCapture);
        stdout().execute(cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        self.rebuild(self.caps.cols);
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
        loop {
            if self.needs_rebuild {
                let width = self.preview_width_for_mode();
                self.rebuild(width);
            }
            self.sync_editor_scroll();
            self.sync_preview_scroll_to_cursor();
            self.draw(terminal)?;

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        if self.handle_key(k)? {
                            break;
                        }
                        if self.needs_rebuild {
                            let width = self.preview_width_for_mode();
                            self.rebuild(width);
                            self.sync_preview_scroll_to_cursor();
                        }
                    }
                    Event::Mouse(m) => match m.kind {
                        MouseEventKind::ScrollDown => {
                            self.preview_scroll = self.preview_scroll.saturating_add(3);
                        }
                        MouseEventKind::ScrollUp => {
                            self.preview_scroll = self.preview_scroll.saturating_sub(3);
                        }
                        _ => {}
                    },
                    Event::Resize(_, _) => {
                        self.caps = TermCaps::detect()?;
                        self.needs_rebuild = true;
                        self.needs_image_redraw = true;
                    }

                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') | KeyCode::Char('q') => return Ok(true),
                KeyCode::Char('s') => {
                    self.save()?;
                    return Ok(false);
                }
                _ => {}
            }
        }

        match self.mode {
            ViewMode::Preview => self.handle_preview_key(key),
            ViewMode::InPlaceEdit | ViewMode::SideBySide => self.handle_edit_key(key),
        }
    }

    fn handle_preview_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            KeyCode::Char('e') | KeyCode::Enter => self.set_mode(ViewMode::InPlaceEdit),
            KeyCode::Char('s') => self.set_mode(ViewMode::SideBySide),
            KeyCode::Down | KeyCode::Char('j') => {
                self.preview_scroll = self.preview_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                let page = self.caps.rows.saturating_sub(2);
                self.preview_scroll = self.preview_scroll.saturating_add(page);
            }
            KeyCode::PageUp => {
                let page = self.caps.rows.saturating_sub(2);
                self.preview_scroll = self.preview_scroll.saturating_sub(page);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.preview_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.preview_scroll =
                    (self.preview_lines.len() as u16).saturating_sub(self.caps.rows);
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_edit_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Insert => self.handle_insert_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        if key.code != KeyCode::Char('g') {
            self.pending_g = false;
        }
        if key.code != KeyCode::Char('d') {
            self.pending_d = false;
        }

        match key.code {
            KeyCode::Esc => self.set_mode(ViewMode::Preview),
            KeyCode::F(2) => self.set_mode(ViewMode::InPlaceEdit),
            KeyCode::F(3) => self.set_mode(ViewMode::SideBySide),
            KeyCode::Char('i') => self.input_mode = InputMode::Insert,
            KeyCode::Char('a') => {
                if self.cursor_col < self.current_line_len() {
                    self.move_right();
                }
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('o') => {
                self.cursor_col = self.current_line_len();
                self.insert_newline();
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('O') => {
                self.cursor_col = 0;
                self.editor_lines.insert(self.cursor_line, String::new());
                self.input_mode = InputMode::Insert;
                self.mark_edited();
            }
            KeyCode::Char('h') | KeyCode::Left => self.move_left(),
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('l') | KeyCode::Right => self.move_right(),
            KeyCode::Char('0') | KeyCode::Home => self.cursor_col = 0,
            KeyCode::Char('$') | KeyCode::End => self.cursor_col = self.current_line_len(),
            KeyCode::Char('G') => {
                self.cursor_line = self.editor_lines.len().saturating_sub(1);
                self.clamp_cursor_col();
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.cursor_line = 0;
                    self.clamp_cursor_col();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => self.delete(),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.half_page_down();
            }
            KeyCode::Char('d') => {
                if self.pending_d {
                    self.delete_current_line();
                    self.pending_d = false;
                } else {
                    self.pending_d = true;
                }
            }
            KeyCode::Backspace => self.move_left(),
            KeyCode::PageUp => self.page_up(),
            KeyCode::PageDown => self.page_down(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.half_page_up();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_insert_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        self.pending_g = false;
        self.pending_d = false;
        match key.code {
            KeyCode::Esc => self.input_mode = InputMode::Normal,
            KeyCode::F(2) => self.set_mode(ViewMode::InPlaceEdit),
            KeyCode::F(3) => self.set_mode(ViewMode::SideBySide),
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert_char(c);
            }
            KeyCode::Enter => self.insert_newline(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Home => self.cursor_col = 0,
            KeyCode::End => self.cursor_col = self.current_line_len(),
            KeyCode::PageUp => self.page_up(),
            KeyCode::PageDown => self.page_down(),
            KeyCode::Tab => {
                for _ in 0..4 {
                    self.insert_char(' ');
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn set_mode(&mut self, mode: ViewMode) {
        self.mode = mode;
        if mode == ViewMode::Preview {
            self.input_mode = InputMode::Normal;
        }
        self.pending_g = false;
        self.pending_d = false;
        self.needs_image_redraw = true;
        self.prev_preview_scroll = u16::MAX;
        self.status = match mode {
            ViewMode::Preview => "Preview: e edit, s split, q quit".to_string(),
            ViewMode::InPlaceEdit => {
                "Edit: i insert, Esc normal/preview, F3 split, Ctrl-S save, Ctrl-Q quit".to_string()
            }
            ViewMode::SideBySide => {
                "Split: preview follows cursor, i insert, Esc normal/preview, F2 edit, Ctrl-S save"
                    .to_string()
            }
        };
        self.needs_rebuild = true;
    }

    fn save(&mut self) -> Result<()> {
        self.content = join_editor_lines(&self.editor_lines);
        let Some(path) = &self.file_path else {
            self.status = "Cannot save stdin input: open a file path to save edits".to_string();
            return Ok(());
        };
        fs::write(path, &self.content)?;
        self.dirty = false;
        self.status = format!("Saved {}", path.display());
        Ok(())
    }

    fn mark_edited(&mut self) {
        self.dirty = true;
        self.needs_rebuild = true;
        self.needs_image_redraw = true;
    }

    fn insert_char(&mut self, c: char) {
        let line = &mut self.editor_lines[self.cursor_line];
        line.insert(self.cursor_col, c);
        self.cursor_col += c.len_utf8();
        self.mark_edited();
    }

    fn insert_newline(&mut self) {
        let rest = self.editor_lines[self.cursor_line].split_off(self.cursor_col);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.editor_lines.insert(self.cursor_line, rest);
        self.mark_edited();
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.editor_lines[self.cursor_line];
            let prev = previous_boundary(line, self.cursor_col);
            line.drain(prev..self.cursor_col);
            self.cursor_col = prev;
            self.mark_edited();
        } else if self.cursor_line > 0 {
            let removed = self.editor_lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.editor_lines[self.cursor_line].len();
            self.editor_lines[self.cursor_line].push_str(&removed);
            self.mark_edited();
        }
    }

    fn delete(&mut self) {
        if self.cursor_col < self.current_line_len() {
            let line = &mut self.editor_lines[self.cursor_line];
            let next = next_boundary(line, self.cursor_col);
            line.drain(self.cursor_col..next);
            self.mark_edited();
        } else if self.cursor_line + 1 < self.editor_lines.len() {
            let next = self.editor_lines.remove(self.cursor_line + 1);
            self.editor_lines[self.cursor_line].push_str(&next);
            self.mark_edited();
        }
    }

    fn delete_current_line(&mut self) {
        if self.editor_lines.len() == 1 {
            self.editor_lines[0].clear();
            self.cursor_col = 0;
        } else {
            self.editor_lines.remove(self.cursor_line);
            if self.cursor_line >= self.editor_lines.len() {
                self.cursor_line = self.editor_lines.len() - 1;
            }
            self.clamp_cursor_col();
        }
        self.mark_edited();
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col =
                previous_boundary(&self.editor_lines[self.cursor_line], self.cursor_col);
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.editor_lines[self.cursor_line].len();
        }
    }

    fn move_right(&mut self) {
        if self.cursor_col < self.current_line_len() {
            self.cursor_col = next_boundary(&self.editor_lines[self.cursor_line], self.cursor_col);
        } else if self.cursor_line + 1 < self.editor_lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.clamp_cursor_col();
        }
    }

    fn move_down(&mut self) {
        if self.cursor_line + 1 < self.editor_lines.len() {
            self.cursor_line += 1;
            self.clamp_cursor_col();
        }
    }

    fn page_up(&mut self) {
        let page = self.editor_page_height() as usize;
        self.cursor_line = self.cursor_line.saturating_sub(page);
        self.clamp_cursor_col();
    }

    fn page_down(&mut self) {
        let page = self.editor_page_height() as usize;
        self.cursor_line = (self.cursor_line + page).min(self.editor_lines.len() - 1);
        self.clamp_cursor_col();
    }

    fn half_page_up(&mut self) {
        let page = (self.editor_page_height() / 2).max(1) as usize;
        self.cursor_line = self.cursor_line.saturating_sub(page);
        self.clamp_cursor_col();
    }

    fn half_page_down(&mut self) {
        let page = (self.editor_page_height() / 2).max(1) as usize;
        self.cursor_line = (self.cursor_line + page).min(self.editor_lines.len() - 1);
        self.clamp_cursor_col();
    }

    fn clamp_cursor_col(&mut self) {
        let len = self.current_line_len();
        if self.cursor_col > len {
            self.cursor_col = len;
        }
        while !self.editor_lines[self.cursor_line].is_char_boundary(self.cursor_col) {
            self.cursor_col -= 1;
        }
    }

    fn current_line_len(&self) -> usize {
        self.editor_lines[self.cursor_line].len()
    }

    fn sync_editor_scroll(&mut self) {
        let height = self.editor_page_height();
        if self.cursor_line < self.editor_scroll as usize {
            self.editor_scroll = self.cursor_line as u16;
        } else if self.cursor_line >= self.editor_scroll as usize + height as usize {
            self.editor_scroll = (self.cursor_line + 1).saturating_sub(height as usize) as u16;
        }
    }

    fn sync_preview_scroll_to_cursor(&mut self) {
        if self.mode != ViewMode::SideBySide || self.preview_lines.is_empty() {
            return;
        }
        let source_max = self.editor_lines.len().saturating_sub(1).max(1);
        let preview_height = self.preview_page_height();
        let preview_max = (self.preview_lines.len() as u16).saturating_sub(preview_height);
        let ratio = self.cursor_line as f32 / source_max as f32;
        self.preview_scroll = ((preview_max as f32) * ratio).round() as u16;
    }

    fn editor_page_height(&self) -> u16 {
        self.caps.rows.saturating_sub(3).max(1)
    }

    fn preview_width_for_mode(&self) -> u16 {
        match self.mode {
            ViewMode::SideBySide => self.caps.cols.saturating_sub(2) / 2,
            _ => self.caps.cols,
        }
        .max(1)
    }

    fn preview_page_height(&self) -> u16 {
        match self.mode {
            ViewMode::SideBySide => self.caps.rows.saturating_sub(3).max(1),
            _ => self.caps.rows.saturating_sub(1).max(1),
        }
    }

    fn editor_view_lines(&self) -> Vec<Line<'static>> {
        self.editor_lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                Line::from(vec![
                    Span::styled(
                        format!("{:>4} ", idx + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(line.clone()),
                ])
            })
            .collect()
    }

    fn editor_cursor_position(&self) -> (u16, u16) {
        let col = byte_col_to_char_col(&self.editor_lines[self.cursor_line], self.cursor_col);
        ((col + 5) as u16, self.cursor_line as u16)
    }

    fn status_line(&self) -> Line<'static> {
        let dirty = if self.dirty { "modified" } else { "saved" };
        let input_mode = match self.mode {
            ViewMode::Preview => "PREVIEW",
            ViewMode::InPlaceEdit | ViewMode::SideBySide => match self.input_mode {
                InputMode::Normal => "NORMAL",
                InputMode::Insert => "INSERT",
            },
        };
        let file = self
            .file_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "stdin".to_string());
        Line::from(vec![
            Span::styled(
                format!(" {} {} ", input_mode, dirty),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{} | {}", file, self.status)),
        ])
    }
}

fn split_editor_lines(content: &str) -> Vec<String> {
    let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    if content.ends_with('\n') {
        lines.push(String::new());
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn join_editor_lines(lines: &[String]) -> String {
    lines.join("\n")
}

fn previous_boundary(s: &str, index: usize) -> usize {
    s[..index]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_boundary(s: &str, index: usize) -> usize {
    s[index..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| index + offset)
        .unwrap_or_else(|| s.len())
}

fn byte_col_to_char_col(s: &str, byte_col: usize) -> usize {
    s[..byte_col].chars().count()
}

fn cursor_in_area(cursor: (u16, u16), area: Rect, scroll: u16) -> Option<(u16, u16)> {
    let inner_x = area.x.saturating_add(1);
    let inner_y = area.y.saturating_add(1);
    let inner_width = area.width.saturating_sub(2);
    let inner_height = area.height.saturating_sub(2);
    let y = cursor.1.checked_sub(scroll)?;
    if y >= inner_height || cursor.0 >= inner_width {
        return None;
    }
    Some((inner_x + cursor.0, inner_y + y))
}

fn status_style(is_dark: bool) -> Style {
    if is_dark {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Rgb(139, 233, 253))
    } else {
        Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(9, 105, 218))
    }
}

fn border_style(is_dark: bool) -> Style {
    if is_dark {
        Style::default().fg(Color::Rgb(48, 54, 61))
    } else {
        Style::default().fg(Color::Rgb(208, 215, 222))
    }
}
