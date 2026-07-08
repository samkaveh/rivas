use super::action::{Mode, MotionType};
use super::buffer::Buffer;
use std::collections::{HashMap, VecDeque};

#[derive(Clone)]
pub struct HistoryEntry {
    pub buffer: Buffer,
    pub row: usize,
    pub col: usize,
}

pub struct EditorState {
    pub buf: Buffer,
    pub row: usize,
    pub col: usize,
    pub col_want: usize,
    pub scroll: usize,
    pub mode: Mode,
    pub cmd_buf: String,
    pub count_buf: String,
    pub operator: Option<char>,
    pub pending: Option<char>,
    pub last_find: Option<(char, bool)>,
    pub registers: HashMap<char, String>,
    pub visual_start: (usize, usize),
    pub undo_stack: VecDeque<HistoryEntry>,
    pub redo_stack: VecDeque<HistoryEntry>,
    pub filename: String,
    pub modified: bool,
    pub message: String,
    pub last_search: String,
    pub search_forward: bool,
    pub view_height: usize,
    pub view_width: usize,
}

impl EditorState {
    pub fn new(filename: String, content: &str) -> Self {
        Self {
            buf: Buffer::new(content),
            row: 0,
            col: 0,
            col_want: 0,
            scroll: 0,
            mode: Mode::Normal,
            cmd_buf: String::new(),
            count_buf: String::new(),
            operator: None,
            pending: None,
            last_find: None,
            registers: HashMap::new(),
            visual_start: (0, 0),
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            filename,
            modified: false,
            message: String::new(),
            last_search: String::new(),
            search_forward: true,
            view_height: 20,
            view_width: 80,
        }
    }

    pub fn count(&self) -> usize {
        self.count_buf.parse::<usize>().unwrap_or(1).max(1)
    }

    pub fn push_undo(&mut self) {
        self.undo_stack.push_back(HistoryEntry {
            buffer: self.buf.clone(),
            row: self.row,
            col: self.col,
        });
        if self.undo_stack.len() > 200 {
            self.undo_stack.pop_front();
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(e) = self.undo_stack.pop_back() {
            self.redo_stack.push_back(HistoryEntry {
                buffer: self.buf.clone(),
                row: self.row,
                col: self.col,
            });
            self.buf = e.buffer;
            self.row = e.row;
            self.col = e.col;
            self.modified = true;
            self.message = "Undo".into();
        } else {
            self.message = "Already at oldest change".into();
        }
    }

    pub fn redo(&mut self) {
        if let Some(e) = self.redo_stack.pop_back() {
            self.undo_stack.push_back(HistoryEntry {
                buffer: self.buf.clone(),
                row: self.row,
                col: self.col,
            });
            self.buf = e.buffer;
            self.row = e.row;
            self.col = e.col;
            self.modified = true;
            self.message = "Redo".into();
        } else {
            self.message = "Already at newest change".into();
        }
    }

    pub fn clamp(&mut self) {
        let n = self.buf.line_count();
        if self.row >= n {
            self.row = n - 1;
        }
        self.col = self
            .buf
            .clamp_col(self.row, self.col, self.mode == Mode::Insert);
    }

    pub fn absolute_byte_offset(&self) -> usize {
        let mut offset = 0;
        for i in 0..self.row {
            offset += self.buf.line(i).len() + 1;
        }
        offset += self.buf.byte_offset(self.row, self.col);
        offset
    }

    pub fn absolute_byte_offset_at(&self, row: usize, col: usize) -> usize {
        let mut offset = 0;
        for i in 0..row {
            offset += self.buf.line(i).len() + 1;
        }
        offset += self.buf.byte_offset(row, col);
        offset
    }

    pub fn scroll_to_cursor(&mut self) {
        if self.row < self.scroll {
            self.scroll = self.row;
        }
        if self.view_height > 0 && self.row >= self.scroll + self.view_height {
            self.scroll = self.row + 1 - self.view_height;
        }
    }

    pub fn yank(&mut self, reg: char, text: String) {
        self.registers.insert(reg, text);
    }

    pub fn resolve_paste_text(&self, reg: char) -> String {
        self.registers.get(&reg).cloned().unwrap_or_default()
    }

    pub fn paste_after(&mut self, reg: char) {
        let text = self.resolve_paste_text(reg);
        if text.is_empty() {
            return;
        }
        if text.ends_with('\n') {
            let lns: Vec<String> = text
                .trim_end_matches('\n')
                .split('\n')
                .map(|s| s.to_string())
                .collect();
            let at = self.row + 1;
            for (i, l) in lns.into_iter().enumerate() {
                self.buf.insert_line(at + i, l);
            }
            self.row = at;
            self.col = self.buf.first_non_blank(self.row);
        } else {
            let col = (self.col + 1).min(self.buf.char_count(self.row));
            let (r, c) = self.buf.insert_text(self.row, col, &text);
            self.row = r;
            self.col = c;
        }
        self.modified = true;
        self.clamp();
    }

    pub fn paste_before(&mut self, reg: char) {
        let text = self.resolve_paste_text(reg);
        if text.is_empty() {
            return;
        }
        if text.ends_with('\n') {
            let lns: Vec<String> = text
                .trim_end_matches('\n')
                .split('\n')
                .map(|s| s.to_string())
                .collect();
            for (i, l) in lns.into_iter().enumerate() {
                self.buf.insert_line(self.row + i, l);
            }
            self.col = self.buf.first_non_blank(self.row);
        } else {
            let (r, c) = self.buf.insert_text(self.row, self.col, &text);
            self.row = r;
            self.col = c;
        }
        self.modified = true;
        self.clamp();
    }

    pub fn apply_motion(&self, motion: char, target: Option<char>) -> Option<(usize, usize)> {
        let (r, c) = (self.row, self.col);
        let nlines = self.buf.line_count();
        let count = self.count();
        match motion {
            'h' => Some((r, c.saturating_sub(count))),
            'l' => Some((r, (c + count).min(self.buf.char_count(r).saturating_sub(1)))),
            'j' => Some(((r + count).min(nlines - 1), c)),
            'k' => Some((r.saturating_sub(count), c)),
            '0' => Some((r, 0)),
            '^' => Some((r, self.buf.first_non_blank(r))),
            '$' => Some((r, self.buf.char_count(r).saturating_sub(1))),
            'w' => {
                let mut p = (r, c);
                for _ in 0..count {
                    p = self.buf.word_forward(p.0, p.1);
                }
                Some(p)
            }
            'b' => {
                let mut p = (r, c);
                for _ in 0..count {
                    p = self.buf.word_backward(p.0, p.1);
                }
                Some(p)
            }
            'e' => {
                let mut p = (r, c);
                for _ in 0..count {
                    p = self.buf.word_end(p.0, p.1);
                }
                Some(p)
            }
            'G' => {
                let dr = if self.count_buf.is_empty() {
                    nlines - 1
                } else {
                    (self.count() - 1).min(nlines - 1)
                };
                Some((dr, self.buf.first_non_blank(dr)))
            }
            '{' => {
                let mut row = r.saturating_sub(1);
                while row > 0 && !self.buf.line(row).trim().is_empty() {
                    row -= 1;
                }
                Some((row, 0))
            }
            '}' => {
                let mut row = (r + 1).min(nlines - 1);
                while row < nlines - 1 && !self.buf.line(row).trim().is_empty() {
                    row += 1;
                }
                Some((row, 0))
            }
            'f' | 't' => target.and_then(|ch| {
                self.buf
                    .find_forward(r, c, ch, motion == 't')
                    .map(|nc| (r, nc))
            }),
            'F' | 'T' => target.and_then(|ch| {
                self.buf
                    .find_backward(r, c, ch, motion == 'T')
                    .map(|nc| (r, nc))
            }),
            _ => None,
        }
    }

    pub fn execute_operator(
        &mut self,
        op: char,
        dest: (usize, usize),
        motion_type: MotionType,
        reg: char,
    ) {
        let (r1, c1, r2, c2) = if (self.row, self.col) <= dest {
            (self.row, self.col, dest.0, dest.1)
        } else {
            (dest.0, dest.1, self.row, self.col)
        };
        if op != 'y' {
            self.push_undo();
        }
        if motion_type == MotionType::Line {
            let mut yanked = String::new();
            for row in r1..=r2 {
                yanked.push_str(self.buf.line(row));
                yanked.push('\n');
            }
            self.yank(reg, yanked);
            if op != 'y' {
                self.buf.lines.drain(r1..=r2);
                if self.buf.lines.is_empty() {
                    self.buf.lines.push(String::new());
                }
                self.row = r1.min(self.buf.line_count() - 1);
                self.col = self.buf.first_non_blank(self.row);
                if op == 'c' {
                    self.buf.insert_line(self.row, String::new());
                    self.col = 0;
                    self.mode = Mode::Insert;
                } else {
                    self.clamp();
                }
            }
        } else if r1 == r2 {
            let chars: Vec<char> = self.buf.line(r1).chars().collect();
            let end = if motion_type == MotionType::Exclusive {
                c2.min(chars.len())
            } else {
                (c2 + 1).min(chars.len())
            };
            let yanked: String = chars[c1..end].iter().collect();
            self.yank(reg, yanked);
            if op != 'y' {
                self.buf.replace_range_on_line(r1, c1, end, "");
                self.col = c1.min(self.buf.char_count(r1).saturating_sub(1));
                if op == 'c' {
                    self.mode = Mode::Insert;
                }
            }
        } else {
            let mut yanked = String::new();
            let h_byte = self.buf.byte_offset(r1, c1);
            yanked.push_str(&self.buf.line(r1)[h_byte..]);
            yanked.push('\n');
            for row in (r1 + 1)..r2 {
                yanked.push_str(self.buf.line(row));
                yanked.push('\n');
            }
            let end_c2 = if motion_type == MotionType::Exclusive {
                c2
            } else {
                c2 + 1
            };
            let t_byte = self
                .buf
                .byte_offset(r2, end_c2.min(self.buf.char_count(r2)));
            yanked.push_str(&self.buf.line(r2)[..t_byte]);
            self.yank(reg, yanked);
            if op != 'y' {
                let tail = self.buf.line(r2)[t_byte..].to_string();
                let h_byte2 = self.buf.byte_offset(r1, c1);
                let head = self.buf.line(r1)[..h_byte2].to_string();

                self.buf.lines.drain(r1..=r2);
                let merged_line = format!("{}{}", head, tail);
                if self.buf.lines.is_empty() {
                    self.buf.lines.push(merged_line);
                } else {
                    self.buf.lines.insert(r1, merged_line);
                }

                self.row = r1;
                self.col = c1;
                self.clamp();
                if op == 'c' {
                    self.mode = Mode::Insert;
                }
            }
        }
        if op != 'y' {
            self.modified = true;
        }
    }

    pub fn delete_lines(&mut self, count: usize, reg: char) {
        self.push_undo();
        let mut yanked = String::new();
        for _ in 0..count {
            let s = self
                .buf
                .delete_line(self.row.min(self.buf.line_count() - 1));
            yanked.push_str(&s);
            yanked.push('\n');
        }
        if self.row >= self.buf.line_count() {
            self.row = self.buf.line_count() - 1;
        }
        self.col = self.buf.first_non_blank(self.row);
        self.yank(reg, yanked);
        self.modified = true;
    }

    pub fn yank_lines(&mut self, count: usize, reg: char) {
        let mut yanked = String::new();
        for i in 0..count {
            let r = (self.row + i).min(self.buf.line_count() - 1);
            yanked.push_str(self.buf.line(r));
            yanked.push('\n');
        }
        self.yank(reg, yanked);
        self.message = format!("{} line{} yanked", count, if count != 1 { "s" } else { "" });
    }

    pub fn execute_command(&mut self) -> bool {
        let cmd = self.cmd_buf.clone();
        self.cmd_buf.clear();
        self.mode = Mode::Normal;
        if let Ok(n) = cmd.parse::<usize>() {
            self.row = (n.saturating_sub(1)).min(self.buf.line_count() - 1);
            self.col = self.buf.first_non_blank(self.row);
            return false;
        }
        match cmd.trim() {
            "w" | "write" => {
                match std::fs::write(&self.filename, self.buf.to_text()) {
                    Ok(_) => {
                        self.modified = false;
                        self.message = format!("\"{}\" written", self.filename);
                    }
                    Err(e) => {
                        self.message = format!("E: {}", e);
                    }
                }
                false
            }
            "q" => {
                if self.modified {
                    self.message = "No write since last change (use :q! to override)".into();
                    false
                } else {
                    true
                }
            }
            "q!" => true,
            "wq" | "x" => {
                let _ = std::fs::write(&self.filename, self.buf.to_text());
                true
            }
            "wq!" => {
                let _ = std::fs::write(&self.filename, self.buf.to_text());
                true
            }
            other => {
                self.message = format!("E: Not an editor command: {}", other);
                false
            }
        }
    }

    pub fn cursor_position(&self) -> super::position::Position {
        super::position::Position::new(self.row, self.col)
    }
}
