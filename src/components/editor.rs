/// iocraft neovim-style modal editor
///
/// Rewritten to use the declarative element!/component macro API instead of
/// raw Component::draw(), since CanvasTextStyle does not carry color/background
/// — those are View/Text props in the element tree.
///
/// Modes:  Normal, Insert, Visual (char), Command, Search
/// Motions: h j k l  w b e  0 ^ $  gg G  { }  f/t/F/T  ; ,
/// Operators: d c y  (+ dd cc yy)
/// Insert: i I a A o O  s S
/// Visual: v + motions + d/c/y
/// Command: :w :q :wq :q! :wq! :<n>  ZZ ZQ
/// Undo/Redo: u  Ctrl-r
/// Paste: p P
/// Search: /pat  ?pat  n N
/// Misc: x X J ~ >> <<  Ctrl-d/u/f/b  PageUp/Down
use iocraft::prelude::*;
use std::collections::{HashMap, VecDeque};

// ─────────────────────────────────────────────────────────────────────────────
// Buffer
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Buffer {
    lines: Vec<String>,
}

impl Buffer {
    fn new(text: &str) -> Self {
        let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self { lines }
    }

    fn to_text(&self) -> String {
        self.lines.join("\n")
    }
    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn line(&self, row: usize) -> &str {
        &self.lines[row.min(self.lines.len().saturating_sub(1))]
    }

    fn char_count(&self, row: usize) -> usize {
        self.line(row).chars().count()
    }

    fn clamp_col(&self, row: usize, col: usize, insert: bool) -> usize {
        let len = self.char_count(row);
        if insert {
            col.min(len)
        } else if len == 0 {
            0
        } else {
            col.min(len - 1)
        }
    }

    fn byte_offset(&self, row: usize, col: usize) -> usize {
        self.line(row)
            .char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(self.line(row).len())
    }

    fn insert_char(&mut self, row: usize, col: usize, ch: char) {
        while row >= self.lines.len() {
            self.lines.push(String::new());
        }
        let byte = self.byte_offset(row, col);
        self.lines[row].insert(byte, ch);
    }

    fn delete_char(&mut self, row: usize, col: usize) -> Option<char> {
        if col >= self.char_count(row) {
            return None;
        }
        let byte = self.byte_offset(row, col);
        Some(self.lines[row].remove(byte))
    }

    fn split_line(&mut self, row: usize, col: usize) {
        let byte = self.byte_offset(row, col);
        let rest = self.lines[row].split_off(byte);
        self.lines.insert(row + 1, rest);
    }

    fn join_lines(&mut self, row: usize) {
        if row + 1 < self.lines.len() {
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next);
        }
    }

    fn delete_line(&mut self, row: usize) -> String {
        if self.lines.len() == 1 {
            let s = self.lines[0].clone();
            self.lines[0].clear();
            s
        } else {
            self.lines.remove(row)
        }
    }

    fn insert_line(&mut self, row: usize, content: String) {
        self.lines.insert(row, content);
    }

    fn replace_range_on_line(&mut self, row: usize, col_start: usize, col_end: usize, s: &str) {
        let start = self.byte_offset(row, col_start);
        let end = self.byte_offset(row, col_end);
        let mut new = self.lines[row][..start].to_string();
        new.push_str(s);
        new.push_str(&self.lines[row][end..]);
        self.lines[row] = new;
    }

    fn word_forward(&self, row: usize, col: usize) -> (usize, usize) {
        let chars: Vec<char> = self.line(row).chars().collect();
        let mut c = col;
        while c < chars.len() && is_word(chars[c]) {
            c += 1;
        }
        while c < chars.len() && chars[c].is_whitespace() {
            c += 1;
        }
        if c >= chars.len() && row + 1 < self.line_count() {
            (row + 1, 0)
        } else {
            (row, c.min(chars.len().saturating_sub(1)))
        }
    }

    fn word_backward(&self, row: usize, col: usize) -> (usize, usize) {
        let chars: Vec<char> = self.line(row).chars().collect();
        let mut c = col as isize - 1;
        while c >= 0 && chars[c as usize].is_whitespace() {
            c -= 1;
        }
        while c > 0 && is_word(chars[(c - 1) as usize]) {
            c -= 1;
        }
        if c < 0 {
            if row > 0 {
                (row - 1, self.char_count(row - 1).saturating_sub(1))
            } else {
                (0, 0)
            }
        } else {
            (row, c as usize)
        }
    }

    fn word_end(&self, row: usize, col: usize) -> (usize, usize) {
        let chars: Vec<char> = self.line(row).chars().collect();
        let mut c = col + 1;
        while c < chars.len() && chars[c].is_whitespace() {
            c += 1;
        }
        while c + 1 < chars.len() && is_word(chars[c + 1]) {
            c += 1;
        }
        (row, c.min(chars.len().saturating_sub(1)))
    }

    fn find_forward(&self, row: usize, col: usize, target: char, before: bool) -> Option<usize> {
        let chars: Vec<char> = self.line(row).chars().collect();
        for i in (col + 1)..chars.len() {
            if chars[i] == target {
                return Some(if before { i.saturating_sub(1) } else { i });
            }
        }
        None
    }

    fn find_backward(&self, row: usize, col: usize, target: char, before: bool) -> Option<usize> {
        if col == 0 {
            return None;
        }
        let chars: Vec<char> = self.line(row).chars().collect();
        for i in (0..col).rev() {
            if chars[i] == target {
                return Some(if before {
                    (i + 1).min(chars.len().saturating_sub(1))
                } else {
                    i
                });
            }
        }
        None
    }

    fn first_non_blank(&self, row: usize) -> usize {
        self.line(row)
            .chars()
            .take_while(|c| c.is_whitespace())
            .count()
    }

    fn search_forward(
        &self,
        pat: &str,
        start_row: usize,
        start_col: usize,
    ) -> Option<(usize, usize)> {
        if pat.is_empty() {
            return None;
        }
        let total = self.line_count();
        for offset in 0..total {
            let row = (start_row + offset) % total;
            let line = self.line(row);
            let from = if offset == 0 {
                (start_col + 1).min(line.len())
            } else {
                0
            };
            if let Some(pos) = line[from..].find(pat) {
                return Some((row, from + pos));
            }
        }
        None
    }

    fn search_backward(
        &self,
        pat: &str,
        start_row: usize,
        start_col: usize,
    ) -> Option<(usize, usize)> {
        if pat.is_empty() {
            return None;
        }
        let total = self.line_count();
        for offset in 0..total {
            let row = if start_row >= offset {
                start_row - offset
            } else {
                total - (offset - start_row)
            };
            let line = self.line(row);
            let to = if offset == 0 {
                start_col.min(line.len())
            } else {
                line.len()
            };
            if let Some(pos) = line[..to].rfind(pat) {
                return Some((row, pos));
            }
        }
        None
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

// ─────────────────────────────────────────────────────────────────────────────
// Mode
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
    Search { forward: bool },
}

impl Mode {
    fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
            Mode::Command => "COMMAND",
            Mode::Search { forward: true } => "SEARCH↓",
            Mode::Search { forward: false } => "SEARCH↑",
        }
    }
    fn color(&self) -> Color {
        match self {
            Mode::Normal => Color::Blue,
            Mode::Insert => Color::Green,
            Mode::Visual => Color::Magenta,
            Mode::Command | Mode::Search { .. } => Color::Yellow,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorState — pure logic, no iocraft types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct HistoryEntry {
    buffer: Buffer,
    row: usize,
    col: usize,
}

#[derive(Clone)]
struct EditorState {
    buf: Buffer,
    row: usize,
    col: usize,
    col_want: usize,
    scroll: usize,
    mode: Mode,
    cmd_buf: String,
    count_buf: String,
    operator: Option<char>,
    pending: Option<char>,
    last_find: Option<(char, bool)>,
    registers: HashMap<char, String>,
    visual_start: (usize, usize),
    undo_stack: VecDeque<HistoryEntry>,
    redo_stack: VecDeque<HistoryEntry>,
    pub filename: String,
    pub modified: bool,
    pub message: String,
    last_search: String,
    search_forward: bool,
    pub view_height: usize,
    pub view_width: usize,
}

impl EditorState {
    fn new(filename: String, content: &str) -> Self {
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

    fn count(&self) -> usize {
        self.count_buf.parse::<usize>().unwrap_or(1).max(1)
    }

    fn push_undo(&mut self) {
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

    fn undo(&mut self) {
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

    fn redo(&mut self) {
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

    fn clamp(&mut self) {
        let n = self.buf.line_count();
        if self.row >= n {
            self.row = n - 1;
        }
        self.col = self
            .buf
            .clamp_col(self.row, self.col, self.mode == Mode::Insert);
    }

    fn scroll_to_cursor(&mut self) {
        if self.row < self.scroll {
            self.scroll = self.row;
        }
        if self.view_height > 0 && self.row >= self.scroll + self.view_height {
            self.scroll = self.row + 1 - self.view_height;
        }
    }

    fn yank(&mut self, reg: char, text: String) {
        self.registers.insert(reg, text.clone());
        self.registers.insert('"', text);
    }

    fn paste_after(&mut self, reg: char) {
        let text = self.registers.get(&reg).cloned().unwrap_or_default();
        if text.is_empty() {
            return;
        }
        self.push_undo();
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
            for (i, ch) in text.chars().enumerate() {
                self.buf.insert_char(self.row, col + i, ch);
            }
            self.col = (col + text.chars().count()).saturating_sub(1);
        }
        self.modified = true;
        self.clamp();
    }

    fn paste_before(&mut self, reg: char) {
        let text = self.registers.get(&reg).cloned().unwrap_or_default();
        if text.is_empty() {
            return;
        }
        self.push_undo();
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
            for (i, ch) in text.chars().enumerate() {
                self.buf.insert_char(self.row, self.col + i, ch);
            }
        }
        self.modified = true;
        self.clamp();
    }

    fn apply_motion(&self, motion: char, target: Option<char>) -> Option<(usize, usize)> {
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

    fn execute_operator(&mut self, op: char, dest: (usize, usize), reg: char) {
        let (r1, c1, r2, c2) = if (self.row, self.col) <= dest {
            (self.row, self.col, dest.0, dest.1)
        } else {
            (dest.0, dest.1, self.row, self.col)
        };
        self.push_undo();
        if r1 == r2 {
            let chars: Vec<char> = self.buf.line(r1).chars().collect();
            let end = (c2 + 1).min(chars.len());
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
            let t_byte = self
                .buf
                .byte_offset(r2, (c2 + 1).min(self.buf.char_count(r2)));
            yanked.push_str(&self.buf.line(r2)[..t_byte]);
            self.yank(reg, yanked);
            if op != 'y' {
                let tail = self.buf.line(r2)[t_byte..].to_string();
                let h_byte2 = self.buf.byte_offset(r1, c1);
                let head = self.buf.line(r1)[..h_byte2].to_string();
                for _ in r1..=r2 {
                    self.buf.delete_line(r1);
                }
                self.buf.insert_line(r1, format!("{}{}", head, tail));
                self.row = r1;
                self.col = c1;
                self.clamp();
                if op == 'c' {
                    self.mode = Mode::Insert;
                }
            }
        }
        self.modified = true;
    }

    fn delete_lines(&mut self, count: usize, reg: char) {
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

    fn yank_lines(&mut self, count: usize, reg: char) {
        let mut yanked = String::new();
        for i in 0..count {
            let r = (self.row + i).min(self.buf.line_count() - 1);
            yanked.push_str(self.buf.line(r));
            yanked.push('\n');
        }
        self.yank(reg, yanked);
        self.message = format!("{} line{} yanked", count, if count != 1 { "s" } else { "" });
    }

    fn execute_command(&mut self) -> bool {
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

    // Called from the iocraft component each frame to produce render data
    // Produces one RenderedLine per visible row. Each line is a Vec<Run> — consecutive
    // chars with identical style are merged into one element, typically giving 2-4 runs
    // per line instead of one element per character. Cuts element count ~40x.
    fn render_lines(&self, view_height: usize, view_width: usize) -> Vec<RenderedLine> {
        let text_w = view_width.saturating_sub(5);
        let line_bg = Color::AnsiValue(236);
        let normal_bg = Color::AnsiValue(234);

        let (vr1, vc1, vr2, vc2) = if self.mode == Mode::Visual {
            let (ar, ac) = self.visual_start;
            let (br, bc) = (self.row, self.col);
            if (ar, ac) <= (br, bc) {
                (ar, ac, br, bc)
            } else {
                (br, bc, ar, ac)
            }
        } else {
            (0, 0, 0, 0)
        };

        (0..view_height)
            .map(|screen_row| {
                let buf_row = screen_row + self.scroll;
                let is_cur = buf_row == self.row;

                if buf_row >= self.buf.line_count() {
                    return RenderedLine::Tilde;
                }

                let bg = if is_cur { line_bg } else { normal_bg };
                let line = self.buf.line(buf_row);
                let chars: Vec<char> = line.chars().collect();
                let cursor_col = self.col.min(chars.len().max(1) - 1);
                let mut runs: Vec<Run> = Vec::new();

                for col in 0..text_w {
                    let ch = chars.get(col).copied().unwrap_or(' ');

                    let is_cursor = is_cur && col == cursor_col;
                    let in_visual = self.mode == Mode::Visual
                        && ((buf_row > vr1 && buf_row < vr2)
                            || (buf_row == vr1 && buf_row == vr2 && col >= vc1 && col <= vc2)
                            || (buf_row == vr1 && buf_row < vr2 && col >= vc1)
                            || (buf_row == vr2 && buf_row > vr1 && col <= vc2));
                    let in_search = !self.last_search.is_empty() && {
                        let byte = self.buf.byte_offset(buf_row, col);
                        line[byte..].starts_with(&self.last_search)
                    };

                    let (fg, cell_bg, bold) = if is_cursor {
                        match self.mode {
                            Mode::Insert => (Color::Black, Color::Green, false),
                            Mode::Visual => (Color::Black, Color::Yellow, false),
                            _ => (Color::Black, Color::White, true),
                        }
                    } else if in_visual {
                        (Color::White, Color::Magenta, false)
                    } else if in_search {
                        (Color::Black, Color::Yellow, false)
                    } else if is_cur {
                        (Color::AnsiValue(252), line_bg, false)
                    } else {
                        (Color::AnsiValue(252), normal_bg, false)
                    };

                    if let Some(last) = runs.last_mut() {
                        if last.fg == fg && last.bg == cell_bg && last.bold == bold {
                            last.text.push(ch);
                            continue;
                        }
                    }
                    runs.push(Run {
                        text: ch.to_string(),
                        fg,
                        bg: cell_bg,
                        bold,
                    });
                }

                RenderedLine::Text {
                    line_num: buf_row,
                    is_current: is_cur,
                    runs,
                    bg,
                }
            })
            .collect()
    }
}

// A styled run of consecutive chars with identical fg/bg/bold.
// Using runs instead of per-cell elements cuts element count from O(cols) to O(2-4) per line.
#[derive(Clone, Debug)]
struct Run {
    text: String,
    fg: Color,
    bg: Color,
    bold: bool,
}

#[derive(Clone, Debug)]
enum RenderedLine {
    Tilde,
    Text {
        line_num: usize,
        is_current: bool,
        runs: Vec<Run>,
        bg: Color,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Key handling (pure, no iocraft)
// ─────────────────────────────────────────────────────────────────────────────

fn handle_key(s: &mut EditorState, code: KeyCode, ctrl: bool) -> bool {
    s.message.clear();
    match s.mode.clone() {
        Mode::Insert => handle_insert(s, code, ctrl),
        Mode::Command => handle_cmdline(s, code),
        Mode::Search { forward } => handle_search(s, code, forward),
        Mode::Visual => handle_visual(s, code),
        Mode::Normal => handle_normal(s, code, ctrl),
    }
}

fn handle_insert(s: &mut EditorState, code: KeyCode, ctrl: bool) -> bool {
    match code {
        KeyCode::Esc => {
            s.mode = Mode::Normal;
            s.col = s.col.saturating_sub(1);
            s.clamp();
        }
        KeyCode::Char('c') if ctrl => {
            s.mode = Mode::Normal;
            s.col = s.col.saturating_sub(1);
            s.clamp();
        }
        KeyCode::Char(c) if !ctrl => {
            s.buf.insert_char(s.row, s.col, c);
            s.col += 1;
            s.modified = true;
        }
        KeyCode::Enter => {
            s.buf.split_line(s.row, s.col);
            s.row += 1;
            s.col = 0;
            s.modified = true;
        }
        KeyCode::Backspace => {
            if s.col > 0 {
                s.col -= 1;
                s.buf.delete_char(s.row, s.col);
                s.modified = true;
            } else if s.row > 0 {
                let prev = s.buf.char_count(s.row - 1);
                s.buf.join_lines(s.row - 1);
                s.row -= 1;
                s.col = prev;
                s.modified = true;
            }
        }
        KeyCode::Delete => {
            if s.col < s.buf.char_count(s.row) {
                s.buf.delete_char(s.row, s.col);
                s.modified = true;
            } else if s.row + 1 < s.buf.line_count() {
                s.buf.join_lines(s.row);
                s.modified = true;
            }
        }
        KeyCode::Left => {
            if s.col > 0 {
                s.col -= 1;
            }
        }
        KeyCode::Right => {
            let l = s.buf.char_count(s.row);
            if s.col < l {
                s.col += 1;
            }
        }
        KeyCode::Up => {
            if s.row > 0 {
                s.row -= 1;
                s.clamp();
            }
        }
        KeyCode::Down => {
            if s.row + 1 < s.buf.line_count() {
                s.row += 1;
                s.clamp();
            }
        }
        KeyCode::Home => {
            s.col = 0;
        }
        KeyCode::End => {
            s.col = s.buf.char_count(s.row);
        }
        _ => {}
    }
    false
}

fn handle_cmdline(s: &mut EditorState, code: KeyCode) -> bool {
    match code {
        KeyCode::Esc => {
            s.mode = Mode::Normal;
            s.cmd_buf.clear();
            false
        }
        KeyCode::Enter => s.execute_command(),
        KeyCode::Backspace => {
            if s.cmd_buf.is_empty() {
                s.mode = Mode::Normal;
            } else {
                s.cmd_buf.pop();
            }
            false
        }
        KeyCode::Char(c) => {
            s.cmd_buf.push(c);
            false
        }
        _ => false,
    }
}

fn handle_search(s: &mut EditorState, code: KeyCode, forward: bool) -> bool {
    match code {
        KeyCode::Esc => {
            s.mode = Mode::Normal;
            s.cmd_buf.clear();
        }
        KeyCode::Enter => {
            s.last_search = s.cmd_buf.clone();
            s.search_forward = forward;
            s.cmd_buf.clear();
            s.mode = Mode::Normal;
            do_search(s, forward);
        }
        KeyCode::Backspace => {
            s.cmd_buf.pop();
        }
        KeyCode::Char(c) => {
            s.cmd_buf.push(c);
        }
        _ => {}
    }
    false
}

fn do_search(s: &mut EditorState, forward: bool) {
    if s.last_search.is_empty() {
        return;
    }
    let res = if forward {
        s.buf.search_forward(&s.last_search, s.row, s.col)
    } else {
        s.buf.search_backward(&s.last_search, s.row, s.col)
    };
    match res {
        Some((r, c)) => {
            s.row = r;
            s.col = c;
        }
        None => {
            s.message = format!("Pattern not found: {}", s.last_search);
        }
    }
}

fn handle_visual(s: &mut EditorState, code: KeyCode) -> bool {
    match code {
        KeyCode::Esc | KeyCode::Char('v') => {
            s.mode = Mode::Normal;
        }
        KeyCode::Char('d') | KeyCode::Char('x') => {
            let d = (s.row, s.col);
            s.execute_operator('d', d, '"');
            s.mode = Mode::Normal;
        }
        KeyCode::Char('y') => {
            let d = (s.row, s.col);
            s.execute_operator('y', d, '"');
            s.mode = Mode::Normal;
        }
        KeyCode::Char('c') => {
            let d = (s.row, s.col);
            s.execute_operator('c', d, '"');
        }
        key => {
            if let Some(dest) = motion_from_key(s, key) {
                s.row = dest.0;
                s.col = dest.1;
                s.col_want = s.col;
            }
        }
    }
    false
}

fn motion_from_key(s: &EditorState, key: KeyCode) -> Option<(usize, usize)> {
    match key {
        KeyCode::Char(c) => s.apply_motion(c, None),
        KeyCode::Left => s.apply_motion('h', None),
        KeyCode::Right => s.apply_motion('l', None),
        KeyCode::Up => s.apply_motion('k', None),
        KeyCode::Down => s.apply_motion('j', None),
        _ => None,
    }
}

fn handle_normal(s: &mut EditorState, code: KeyCode, ctrl: bool) -> bool {
    if ctrl {
        match code {
            KeyCode::Char('r') => {
                let c = s.count();
                s.count_buf.clear();
                for _ in 0..c {
                    s.redo();
                }
                s.clamp();
                return false;
            }
            KeyCode::Char('d') => {
                let h = (s.view_height / 2).max(1);
                s.row = (s.row + h).min(s.buf.line_count() - 1);
                s.clamp();
                s.count_buf.clear();
                return false;
            }
            KeyCode::Char('u') => {
                let h = (s.view_height / 2).max(1);
                s.row = s.row.saturating_sub(h);
                s.clamp();
                s.count_buf.clear();
                return false;
            }
            KeyCode::Char('f') => {
                s.row = (s.row + s.view_height).min(s.buf.line_count() - 1);
                s.clamp();
                s.count_buf.clear();
                return false;
            }
            KeyCode::Char('b') => {
                s.row = s.row.saturating_sub(s.view_height);
                s.clamp();
                s.count_buf.clear();
                return false;
            }
            _ => {}
        }
    }

    // Resolve pending two-char sequences
    if let Some(pend) = s.pending {
        s.pending = None;
        match pend {
            'g' => {
                if code == KeyCode::Char('g') {
                    let dest = (0, s.buf.first_non_blank(0));
                    if let Some(op) = s.operator.take() {
                        s.execute_operator(op, dest, '"');
                    } else {
                        s.row = dest.0;
                        s.col = dest.1;
                        s.col_want = s.col;
                    }
                }
                s.count_buf.clear();
                return false;
            }
            'Z' => {
                if code == KeyCode::Char('Z') {
                    let _ = std::fs::write(&s.filename, s.buf.to_text());
                    return true;
                }
                if code == KeyCode::Char('Q') {
                    return true;
                }
                s.count_buf.clear();
                return false;
            }
            'r' => {
                if let KeyCode::Char(c) = code {
                    s.push_undo();
                    s.buf.delete_char(s.row, s.col);
                    s.buf.insert_char(s.row, s.col, c);
                    s.modified = true;
                }
                s.count_buf.clear();
                return false;
            }
            m @ ('f' | 't' | 'F' | 'T') => {
                if let KeyCode::Char(target) = code {
                    let backward = m == 'F' || m == 'T';
                    s.last_find = Some((target, backward));
                    if let Some(dest) = s.apply_motion(m, Some(target)) {
                        if let Some(op) = s.operator.take() {
                            s.execute_operator(op, dest, '"');
                        } else {
                            s.row = dest.0;
                            s.col = dest.1;
                            s.col_want = s.col;
                        }
                    }
                }
                s.count_buf.clear();
                s.clamp();
                return false;
            }
            _ => {
                s.count_buf.clear();
                return false;
            }
        }
    }

    match code {
        // Count digits
        KeyCode::Char(d @ '1'..='9') if s.operator.is_none() && s.count_buf.len() < 8 => {
            s.count_buf.push(d);
            return false;
        }
        KeyCode::Char('0') if !s.count_buf.is_empty() && s.operator.is_none() => {
            s.count_buf.push('0');
            return false;
        }

        // Enter insert
        KeyCode::Char('i') => {
            s.push_undo();
            s.mode = Mode::Insert;
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('I') => {
            s.push_undo();
            s.col = s.buf.first_non_blank(s.row);
            s.mode = Mode::Insert;
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('a') => {
            s.push_undo();
            let l = s.buf.char_count(s.row);
            if l > 0 {
                s.col = (s.col + 1).min(l);
            }
            s.mode = Mode::Insert;
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('A') => {
            s.push_undo();
            s.col = s.buf.char_count(s.row);
            s.mode = Mode::Insert;
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('o') => {
            s.push_undo();
            s.buf.insert_line(s.row + 1, String::new());
            s.row += 1;
            s.col = 0;
            s.mode = Mode::Insert;
            s.modified = true;
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('O') => {
            s.push_undo();
            s.buf.insert_line(s.row, String::new());
            s.col = 0;
            s.mode = Mode::Insert;
            s.modified = true;
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('s') => {
            s.push_undo();
            s.buf.delete_char(s.row, s.col);
            s.mode = Mode::Insert;
            s.modified = true;
            s.count_buf.clear();
            return false;
        }

        // Visual / command / search
        KeyCode::Char('v') => {
            s.visual_start = (s.row, s.col);
            s.mode = Mode::Visual;
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char(':') => {
            s.mode = Mode::Command;
            s.cmd_buf.clear();
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('/') => {
            s.mode = Mode::Search { forward: true };
            s.cmd_buf.clear();
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('?') => {
            s.mode = Mode::Search { forward: false };
            s.cmd_buf.clear();
            s.count_buf.clear();
            return false;
        }

        // Undo / search repeat
        KeyCode::Char('u') => {
            let c = s.count();
            s.count_buf.clear();
            for _ in 0..c {
                s.undo();
            }
            s.clamp();
            return false;
        }
        KeyCode::Char('n') => {
            do_search(s, s.search_forward);
            s.count_buf.clear();
            return false;
        }
        KeyCode::Char('N') => {
            let fwd = !s.search_forward;
            do_search(s, fwd);
            s.count_buf.clear();
            return false;
        }

        // Operators
        KeyCode::Char(op @ ('d' | 'c' | 'y')) => {
            if s.operator == Some(op) {
                let count = s.count();
                s.count_buf.clear();
                s.operator = None;
                match op {
                    'd' => s.delete_lines(count, '"'),
                    'y' => s.yank_lines(count, '"'),
                    'c' => {
                        s.push_undo();
                        s.buf.lines[s.row].clear();
                        s.col = 0;
                        s.mode = Mode::Insert;
                        s.modified = true;
                    }
                    _ => {}
                }
            } else {
                s.operator = Some(op);
                return false;
            }
        }

        // x X
        KeyCode::Char('x') | KeyCode::Delete => {
            s.push_undo();
            let count = s.count();
            s.count_buf.clear();
            for _ in 0..count {
                if s.col < s.buf.char_count(s.row) {
                    s.buf.delete_char(s.row, s.col);
                }
            }
            s.clamp();
            s.modified = true;
        }
        KeyCode::Char('X') => {
            s.push_undo();
            if s.col > 0 {
                s.col -= 1;
                s.buf.delete_char(s.row, s.col);
                s.modified = true;
            }
        }

        // r (replace)
        KeyCode::Char('r') => {
            s.pending = Some('r');
            return false;
        }

        // Paste
        KeyCode::Char('p') => {
            s.paste_after('"');
            s.count_buf.clear();
        }
        KeyCode::Char('P') => {
            s.paste_before('"');
            s.count_buf.clear();
        }

        // J ~ >> <<
        KeyCode::Char('J') => {
            s.push_undo();
            let c = s.count().max(1);
            s.count_buf.clear();
            for _ in 0..c {
                if s.row + 1 < s.buf.line_count() {
                    s.buf.join_lines(s.row);
                }
            }
            s.modified = true;
        }
        KeyCode::Char('~') => {
            s.push_undo();
            if let Some(c) = s.buf.line(s.row).chars().nth(s.col) {
                let tog: String = if c.is_uppercase() {
                    c.to_lowercase().collect()
                } else {
                    c.to_uppercase().collect()
                };
                s.buf.replace_range_on_line(s.row, s.col, s.col + 1, &tog);
                s.col = (s.col + 1).min(s.buf.char_count(s.row).saturating_sub(1));
                s.modified = true;
            }
        }
        KeyCode::Char('>') if s.operator == Some('>') => {
            s.push_undo();
            let c = s.count();
            s.operator = None;
            s.count_buf.clear();
            for i in 0..c {
                let r = (s.row + i).min(s.buf.line_count() - 1);
                s.buf.lines[r].insert_str(0, "    ");
            }
            s.modified = true;
        }
        KeyCode::Char('<') if s.operator == Some('<') => {
            s.push_undo();
            let c = s.count();
            s.operator = None;
            s.count_buf.clear();
            for i in 0..c {
                let r = (s.row + i).min(s.buf.line_count() - 1);
                let sp = s.buf.lines[r]
                    .chars()
                    .take_while(|&c| c == ' ')
                    .count()
                    .min(4);
                s.buf.lines[r] = s.buf.lines[r][sp..].to_string();
            }
            s.modified = true;
        }
        KeyCode::Char('>') => {
            s.operator = Some('>');
            return false;
        }
        KeyCode::Char('<') => {
            s.operator = Some('<');
            return false;
        }

        // Two-char sequences
        KeyCode::Char('g') => {
            s.pending = Some('g');
            return false;
        }
        KeyCode::Char('Z') => {
            s.pending = Some('Z');
            return false;
        }
        KeyCode::Char(m @ ('f' | 't' | 'F' | 'T')) => {
            s.pending = Some(m);
            return false;
        }

        // ; ,
        KeyCode::Char(';') | KeyCode::Char(',') => {
            if let Some((target, was_backward)) = s.last_find {
                let fwd = if code == KeyCode::Char(';') {
                    !was_backward
                } else {
                    was_backward
                };
                let nc = if fwd {
                    s.buf.find_forward(s.row, s.col, target, false)
                } else {
                    s.buf.find_backward(s.row, s.col, target, false)
                };
                if let Some(c) = nc {
                    s.col = c;
                    s.col_want = s.col;
                }
            }
        }

        // G
        KeyCode::Char('G') => {
            if let Some(dest) = s.apply_motion('G', None) {
                if let Some(op) = s.operator.take() {
                    s.execute_operator(op, dest, '"');
                } else {
                    s.row = dest.0;
                    s.col = dest.1;
                    s.col_want = s.col;
                }
            }
        }

        // j k (sticky col)
        KeyCode::Char('j') | KeyCode::Down => {
            let count = s.count();
            s.count_buf.clear();
            if let Some(op) = s.operator.take() {
                let dest = ((s.row + count).min(s.buf.line_count() - 1), s.col);
                s.execute_operator(op, dest, '"');
            } else {
                for _ in 0..count {
                    if s.row + 1 < s.buf.line_count() {
                        s.row += 1;
                        s.col = s.buf.clamp_col(s.row, s.col_want, false);
                    }
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let count = s.count();
            s.count_buf.clear();
            if let Some(op) = s.operator.take() {
                let dest = (s.row.saturating_sub(count), s.col);
                s.execute_operator(op, dest, '"');
            } else {
                for _ in 0..count {
                    if s.row > 0 {
                        s.row -= 1;
                        s.col = s.buf.clamp_col(s.row, s.col_want, false);
                    }
                }
            }
        }

        // All other motions (+ optional operator)
        KeyCode::Char('h')
        | KeyCode::Left
        | KeyCode::Char('l')
        | KeyCode::Right
        | KeyCode::Char('w')
        | KeyCode::Char('b')
        | KeyCode::Char('e')
        | KeyCode::Char('0')
        | KeyCode::Char('^')
        | KeyCode::Char('$')
        | KeyCode::Char('{')
        | KeyCode::Char('}') => {
            let ch = match code {
                KeyCode::Char(c) => c,
                KeyCode::Left => 'h',
                KeyCode::Right => 'l',
                _ => unreachable!(),
            };
            if let Some(dest) = s.apply_motion(ch, None) {
                if let Some(op) = s.operator.take() {
                    s.execute_operator(op, dest, '"');
                } else {
                    s.row = dest.0;
                    s.col = dest.1;
                    s.col_want = s.col;
                }
            }
        }

        KeyCode::PageDown => {
            s.row = (s.row + s.view_height).min(s.buf.line_count() - 1);
            s.clamp();
        }
        KeyCode::PageUp => {
            s.row = s.row.saturating_sub(s.view_height);
            s.clamp();
        }

        _ => {}
    }

    if s.operator.is_none() && s.pending.is_none() {
        s.count_buf.clear();
    }
    s.clamp();
    false
}

// ─────────────────────────────────────────────────────────────────────────────
// iocraft component — uses element!/Text/View for rendering
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default, Props)]
pub struct NvimEditorProps {
    pub filename: String,
    pub initial_content: String,
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub on_change: Handler<String>,
}

#[component]
pub fn NvimEditor(mut hooks: Hooks, props: &mut NvimEditorProps) -> impl Into<AnyElement<'static>> {
    // use_ref: holds EditorState directly — mutations don't trigger re-renders.
    let mut state_ref = hooks.use_ref(|| {
        Some(EditorState::new(
            props.filename.clone(),
            &props.initial_content,
        ))
    });

    // use_state: a simple counter — the ONLY thing that drives re-renders.
    let tick: State<u64> = hooks.use_state(|| 0u64);
    let should_quit: State<bool> = hooks.use_state(|| false);

    // use_async_handler: bridges sync terminal events into async tasks that
    // can safely call tick.set() to request a re-render.
    let on_change = props.on_change.clone();
    let on_event = hooks.use_async_handler({
        let state_ref = state_ref.clone();
        move |ev: TerminalEvent| {
            let mut state_ref = state_ref.clone();
            let mut tick = tick.clone();
            let mut should_quit = should_quit.clone();
            let on_change = on_change.clone();
            async move {
                if let TerminalEvent::Resize(w, h) = &ev {
                    {
                        if let Some(s) = state_ref.write().as_mut() {
                            s.view_width = *w as usize;
                            s.view_height = (*h as usize).saturating_sub(3);
                        }
                    }

                    tick.set(tick.get().wrapping_add(1));
                }
                if let TerminalEvent::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                }) = ev
                {
                    if kind == KeyEventKind::Press {
                        let ctrl = modifiers.contains(KeyModifiers::CONTROL);
                        let (quit, new_content) = if let Some(s) = state_ref.write().as_mut() {
                            let before_content = s.buf.to_text();
                            let q = handle_key(s, code, ctrl);
                            let after_content = s.buf.to_text();
                            let changed = before_content != after_content;
                            (q, changed.then_some(after_content))
                        } else {
                            (false, None)
                        };
                        if let Some(content) = new_content {
                            on_change(content);
                        }
                        if quit {
                            should_quit.set(true);
                        } else {
                            tick.set(tick.get().wrapping_add(1));
                        }
                    }
                }
            }
        }
    });

    hooks.use_terminal_events(move |ev| {
        on_event(ev);
    });

    let mut system = hooks.use_context_mut::<SystemContext>();
    if *should_quit.read() {
        system.exit();
    }

    // ── Snapshot state for rendering (read-only borrow of the ref) ────────
    {
        if let Some(s) = state_ref.write().as_mut() {
            s.view_width = props.viewport_width as usize;
            s.view_height = (props.viewport_height as usize).saturating_sub(3);
            s.scroll_to_cursor();
        }
    }
    let guard = state_ref.read();
    let s = guard.as_ref().unwrap();

    let view_h = s.view_height;
    let view_w = s.view_width;
    let lines = s.render_lines(view_h, view_w);
    let mode = s.mode.clone();
    let row = s.row;
    let col = s.col;
    let total = s.buf.line_count();
    let filename = s.filename.clone();
    let modified = s.modified;
    let message = s.message.clone();
    let cmd_buf = s.cmd_buf.clone();
    let count_buf = s.count_buf.clone();
    let operator = s.operator;
    drop(guard);

    // ── Status line strings ───────────────────────────────────────────────
    // ── Status line strings ───────────────────────────────────────────────
    let mode_str = format!(" {} ", mode.label());
    let pos_str = format!(" {}:{} ", row + 1, col + 1);
    let fname_str = format!("  {}{}  ", filename, if modified { " [+]" } else { "" });

    // ── Cmdline/message ───────────────────────────────────────────────────
    let cmdline_text = match mode {
        Mode::Command => format!(":{}", cmd_buf),
        Mode::Search { forward } => format!("{}{}", if forward { '/' } else { '?' }, cmd_buf),
        _ => message.clone(),
    };
    let count_hint = if !count_buf.is_empty() || operator.is_some() {
        format!(
            "{}{}",
            count_buf,
            operator.map(|c| c.to_string()).unwrap_or_default()
        )
    } else {
        String::new()
    };

    element! {
        View(
            width: 100pct,
            height: 100pct,
            flex_direction: FlexDirection::Column,
        ) {
            // ── Editor area ──────────────────────────────────────────────
            View(flex_grow: 1.0, flex_direction: FlexDirection::Row, overflow: Overflow::Hidden) {
                // Gutter
                View(width: 5, flex_direction: FlexDirection::Column, background_color: Color::AnsiValue(235)) {
                    #(lines.iter().map(|line| match line {
                        RenderedLine::Tilde => element! {
                            View(width: 100pct, justify_content: JustifyContent::FlexEnd, padding_right: 1) {
                                Text(content: "~", color: Color::DarkBlue)
                            }
                        },
                        RenderedLine::Text { line_num, is_current, .. } => element! {
                            View(width: 100pct, justify_content: JustifyContent::FlexEnd, padding_right: 1) {
                                Text(
                                    content: format!("{}", line_num + 1),
                                    color: if *is_current { Color::White } else { Color::AnsiValue(240) },
                                    weight: if *is_current { Weight::Bold } else { Weight::Normal },
                                )
                            }
                        },
                    }))
                }

                // Text area — one Text per styled run, not one per character.
                View(flex_grow: 1.0, flex_direction: FlexDirection::Column, overflow: Overflow::Hidden) {
                    #(lines.iter().map(|line| match line {
                        RenderedLine::Tilde => element! {
                            View(background_color: Color::AnsiValue(234)) {
                                Text(content: "~", color: Color::DarkBlue)
                            }
                        },
                        RenderedLine::Text { runs, bg, .. } => element! {
                            View(flex_direction: FlexDirection::Row, background_color: *bg) {
                                #(runs.iter().map(|run| element! {
                                    View(background_color: run.bg) {
                                        Text(
                                            content: run.text.clone(),
                                            color: run.fg,
                                            weight: if run.bold { Weight::Bold } else { Weight::Normal },
                                        )
                                    }
                                }))
                            }
                        },
                    }))
                }
            }

            // ── Status bar ───────────────────────────────────────────────
            View(
                width: 100pct,
                flex_direction: FlexDirection::Row,
                background_color: Color::AnsiValue(238),
            ) {
                View(background_color: mode.color(), padding_left: 1, padding_right: 1) {
                    Text(content: mode_str, color: Color::Black, weight: Weight::Bold)
                }
                View(flex_grow: 1.0, padding_left: 1) {
                    Text(content: fname_str, color: Color::AnsiValue(250))
                }
                View(padding_left: 1, padding_right: 1) {
                    Text(content: format!("{}:{} / {}", row + 1, col + 1, total), color: Color::AnsiValue(250))
                }
                View(padding_right: 1) {
                    Text(content: pos_str, color: Color::AnsiValue(250))
                }
            }

            // ── Cmdline / message ────────────────────────────────────────
            View(width: 100pct, flex_direction: FlexDirection::Row) {
                View(flex_grow: 1.0) {
                    Text(content: cmdline_text, color: Color::White)
                }
                #(if !count_hint.is_empty() {
                    Some(element! {
                        View(padding_right: 1) {
                            Text(content: count_hint.clone(), color: Color::Yellow)
                        }
                    })
                } else {
                    None
                }.into_iter())
            }

            // ── Help line ────────────────────────────────────────────────
            View(width: 100pct, background_color: Color::AnsiValue(234)) {
                Text(
                    content: " i/a=Insert  v=Visual  :w=Save  :q=Quit  u=Undo  C-r=Redo  /=Search  n=Next  ZZ=SaveQuit ",
                    color: Color::AnsiValue(242),
                )
            }
        }
    }
}
