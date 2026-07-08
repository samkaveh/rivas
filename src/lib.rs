pub mod editor;
pub mod protocol;

use editor::{EditorState, Mode, Position};

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
    pub cursor: Position,
    pub modified: bool,
}

pub struct Editor {
    state: EditorState,
}

impl Editor {
    pub fn new(filename: &str, content: &str) -> Self {
        Self {
            state: EditorState::new(filename.to_string(), content),
        }
    }

    pub fn execute_vim(&mut self, cmd: &str) -> CommandResult {
        let mut chars = cmd.chars().peekable();
        let mut count_buf = String::new();

        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                count_buf.push(c);
                chars.next();
            } else {
                break;
            }
        }

        let count: usize = if count_buf.is_empty() {
            1
        } else {
            count_buf.parse().unwrap_or(1)
        };

        let Some(c) = chars.next() else {
            return CommandResult {
                success: false,
                message: "Empty command".to_string(),
                cursor: self.state.cursor_position(),
                modified: self.state.modified,
            };
        };

        match c {
            'h' => {
                for _ in 0..count {
                    if self.state.col > 0 {
                        self.state.col -= 1;
                    }
                }
            }
            'l' => {
                for _ in 0..count {
                    let limit = self.state.buf.char_count(self.state.row).saturating_sub(1);
                    if self.state.col < limit {
                        self.state.col += 1;
                    }
                }
            }
            'j' => {
                for _ in 0..count {
                    if self.state.row + 1 < self.state.buf.line_count() {
                        self.state.row += 1;
                        self.state.col = self
                            .state
                            .buf
                            .clamp_col(self.state.row, self.state.col_want, false);
                    }
                }
            }
            'k' => {
                for _ in 0..count {
                    if self.state.row > 0 {
                        self.state.row -= 1;
                        self.state.col = self
                            .state
                            .buf
                            .clamp_col(self.state.row, self.state.col_want, false);
                    }
                }
            }
            'w' => {
                for _ in 0..count {
                    let pos = self.state.buf.word_forward(self.state.row, self.state.col);
                    self.state.row = pos.0;
                    self.state.col = pos.1;
                    self.state.col_want = pos.1;
                }
            }
            'b' => {
                for _ in 0..count {
                    let pos = self.state.buf.word_backward(self.state.row, self.state.col);
                    self.state.row = pos.0;
                    self.state.col = pos.1;
                    self.state.col_want = pos.1;
                }
            }
            'e' => {
                for _ in 0..count {
                    let pos = self.state.buf.word_end(self.state.row, self.state.col);
                    self.state.row = pos.0;
                    self.state.col = pos.1;
                    self.state.col_want = pos.1;
                }
            }
            '0' => {
                self.state.col = 0;
                self.state.col_want = 0;
            }
            '^' => {
                self.state.col = self.state.buf.first_non_blank(self.state.row);
                self.state.col_want = self.state.col;
            }
            '$' => {
                self.state.col = self.state.buf.char_count(self.state.row).saturating_sub(1);
                self.state.col_want = usize::MAX;
            }
            'G' => {
                let target = if count == 1 {
                    self.state.buf.line_count() - 1
                } else {
                    (count - 1).min(self.state.buf.line_count() - 1)
                };
                self.state.row = target;
                self.state.col = self.state.buf.first_non_blank(target);
                self.state.col_want = self.state.col;
            }
            'g' => {
                if let Some('g') = chars.next() {
                    self.state.row = 0;
                    self.state.col = self.state.buf.first_non_blank(0);
                    self.state.col_want = self.state.col;
                }
            }
            '{' => {
                for _ in 0..count {
                    let mut row = self.state.row.saturating_sub(1);
                    while row > 0 && !self.state.buf.line(row).trim().is_empty() {
                        row -= 1;
                    }
                    self.state.row = row;
                    self.state.col = 0;
                    self.state.col_want = 0;
                }
            }
            '}' => {
                for _ in 0..count {
                    let mut row = (self.state.row + 1).min(self.state.buf.line_count() - 1);
                    while row < self.state.buf.line_count() - 1
                        && !self.state.buf.line(row).trim().is_empty()
                    {
                        row += 1;
                    }
                    self.state.row = row;
                    self.state.col = 0;
                    self.state.col_want = 0;
                }
            }
            'd' => {
                if let Some('d') = chars.peek() {
                    chars.next();
                    for _ in 0..count {
                        self.state
                            .delete_lines(1, '"');
                    }
                }
            }
            'y' => {
                if let Some('y') = chars.peek() {
                    chars.next();
                    self.state.yank_lines(count, '"');
                }
            }
            'x' => {
                self.state.push_undo();
                let mut cut = String::new();
                for _ in 0..count {
                    if self.state.col < self.state.buf.char_count(self.state.row) {
                        if let Some(c) = self.state.buf.delete_char(self.state.row, self.state.col) {
                            cut.push(c);
                        }
                    }
                }
                if !cut.is_empty() {
                    self.state.yank('"', cut);
                }
                self.state.clamp();
                self.state.modified = true;
            }
            'p' => {
                self.state.push_undo();
                for _ in 0..count {
                    self.state.paste_after('"');
                }
            }
            'P' => {
                self.state.push_undo();
                for _ in 0..count {
                    self.state.paste_before('"');
                }
            }
            '"' => {
                // Register selection: "x where x is a register name
                if let Some(reg) = chars.next() {
                    // Parse the next command with the specified register
                    let remaining: String = chars.collect();
                    if !remaining.is_empty() {
                        // Re-parse with register context
                        let mut editor = Editor::new(&self.state.filename, &self.state.buf.to_text());
                        editor.state.registers = self.state.registers.clone();
                        editor.state.mode = self.state.mode.clone();
                        let result = editor.execute_vim(&remaining);
                        self.state = editor.state;
                        return result;
                    }
                }
            }
            'v' => {
                self.state.mode = Mode::Visual;
                self.state.visual_start = (self.state.row, self.state.col);
            }
            'u' => {
                for _ in 0..count {
                    self.state.undo();
                }
                self.state.clamp();
            }
            ':' => {
                let cmd: String = chars.collect();
                self.state.cmd_buf = cmd;
                self.state.execute_command();
            }
            'i' => {
                self.state.mode = Mode::Insert;
            }
            'a' => {
                self.state.mode = Mode::Insert;
                let l = self.state.buf.char_count(self.state.row);
                if l > 0 {
                    self.state.col = (self.state.col + 1).min(l);
                }
            }
            'I' => {
                self.state.col = self.state.buf.first_non_blank(self.state.row);
                self.state.mode = Mode::Insert;
            }
            'A' => {
                self.state.col = self.state.buf.char_count(self.state.row);
                self.state.mode = Mode::Insert;
            }
            'o' => {
                self.state.push_undo();
                self.state.buf.insert_line(self.state.row + 1, String::new());
                self.state.row += 1;
                self.state.col = 0;
                self.state.mode = Mode::Insert;
                self.state.modified = true;
            }
            'O' => {
                self.state.push_undo();
                self.state.buf.insert_line(self.state.row, String::new());
                self.state.col = 0;
                self.state.mode = Mode::Insert;
                self.state.modified = true;
            }
            'J' => {
                self.state.push_undo();
                let c = count.saturating_sub(1).max(1);
                for _ in 0..c {
                    if self.state.row + 1 < self.state.buf.line_count() {
                        let next = self.state.buf.lines.remove(self.state.row + 1);
                        let trimmed_next = next.trim_start();
                        if !self.state.buf.lines[self.state.row].is_empty()
                            && !self.state.buf.lines[self.state.row].ends_with(' ')
                            && !trimmed_next.is_empty()
                        {
                            self.state.buf.lines[self.state.row].push(' ');
                        }
                        self.state.buf.lines[self.state.row].push_str(trimmed_next);
                    }
                }
                self.state.modified = true;
            }
            '~' => {
                self.state.push_undo();
                if let Some(c) = self.state.buf.line(self.state.row).chars().nth(self.state.col) {
                    let tog: String = if c.is_uppercase() {
                        c.to_lowercase().collect()
                    } else {
                        c.to_uppercase().collect()
                    };
                    self.state
                        .buf
                        .replace_range_on_line(self.state.row, self.state.col, self.state.col + 1, &tog);
                    self.state.col = (self.state.col + 1)
                        .min(self.state.buf.char_count(self.state.row).saturating_sub(1));
                    self.state.modified = true;
                }
            }
            'r' => {
                if let Some(target) = chars.next() {
                    self.state.push_undo();
                    self.state.buf.delete_char(self.state.row, self.state.col);
                    self.state.buf.insert_char(self.state.row, self.state.col, target);
                    self.state.modified = true;
                }
            }
            'f' | 'F' | 't' | 'T' => {
                if let Some(target) = chars.next() {
                    let backward = c == 'F' || c == 'T';
                    let before = c == 't' || c == 'T';
                    self.state.last_find = Some((target, backward));
                    let result = if backward {
                        self.state
                            .buf
                            .find_backward(self.state.row, self.state.col, target, before)
                    } else {
                        self.state
                            .buf
                            .find_forward(self.state.row, self.state.col, target, before)
                    };
                    if let Some(col) = result {
                        self.state.col = col;
                        self.state.col_want = col;
                    }
                }
            }
            '/' => {
                let pattern: String = chars.collect();
                self.state.last_search = pattern.clone();
                self.state.search_forward = true;
                if let Some((r, c)) = self.state.buf.search_forward(
                    &pattern,
                    self.state.row,
                    self.state.col,
                ) {
                    self.state.row = r;
                    self.state.col = c;
                } else {
                    self.state.message = format!("Pattern not found: {}", pattern);
                }
            }
            '?' => {
                let pattern: String = chars.collect();
                self.state.last_search = pattern.clone();
                self.state.search_forward = false;
                if let Some((r, c)) = self.state.buf.search_backward(
                    &pattern,
                    self.state.row,
                    self.state.col,
                ) {
                    self.state.row = r;
                    self.state.col = c;
                } else {
                    self.state.message = format!("Pattern not found: {}", pattern);
                }
            }
            'n' => {
                let pattern = self.state.last_search.clone();
                let forward = self.state.search_forward;
                if forward {
                    if let Some((r, c)) = self.state.buf.search_forward(
                        &pattern,
                        self.state.row,
                        self.state.col,
                    ) {
                        self.state.row = r;
                        self.state.col = c;
                    }
                } else if let Some((r, c)) = self.state.buf.search_backward(
                    &pattern,
                    self.state.row,
                    self.state.col,
                ) {
                    self.state.row = r;
                    self.state.col = c;
                }
            }
            'N' => {
                let pattern = self.state.last_search.clone();
                let forward = !self.state.search_forward;
                if forward {
                    if let Some((r, c)) = self.state.buf.search_forward(
                        &pattern,
                        self.state.row,
                        self.state.col,
                    ) {
                        self.state.row = r;
                        self.state.col = c;
                    }
                } else if let Some((r, c)) = self.state.buf.search_backward(
                    &pattern,
                    self.state.row,
                    self.state.col,
                ) {
                    self.state.row = r;
                    self.state.col = c;
                }
            }
            _ => {
                return CommandResult {
                    success: false,
                    message: format!("Unknown command: {}", c),
                    cursor: self.state.cursor_position(),
                    modified: self.state.modified,
                };
            }
        }

        self.state.clamp();

        CommandResult {
            success: true,
            message: self.state.message.clone(),
            cursor: self.state.cursor_position(),
            modified: self.state.modified,
        }
    }

    pub fn content(&self) -> String {
        self.state.buf.to_text()
    }

    pub fn cursor(&self) -> Position {
        self.state.cursor_position()
    }

    pub fn is_modified(&self) -> bool {
        self.state.modified
    }

    pub fn mode(&self) -> Mode {
        self.state.mode.clone()
    }

    pub fn save(&mut self) -> Result<(), String> {
        std::fs::write(&self.state.filename, self.state.buf.to_text())
            .map_err(|e| e.to_string())
    }

    pub fn set_content(&mut self, content: &str) {
        self.state.buf = editor::Buffer::new(content);
    }

    pub fn state(&self) -> &EditorState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut EditorState {
        &mut self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_new() {
        let editor = Editor::new("test.md", "hello\nworld");
        assert_eq!(editor.content(), "hello\nworld");
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn test_execute_vim_move() {
        let mut editor = Editor::new("test.md", "hello\nworld");
        let result = editor.execute_vim("l");
        assert!(result.success);
        assert_eq!(result.cursor, Position::new(0, 1));

        let result = editor.execute_vim("j");
        assert!(result.success);
        // After moving down, cursor stays at column 1 (or clamped to end of line if shorter)
        // "world" has 5 chars, so col 1 is valid
        assert_eq!(result.cursor.row, 1);
    }

    #[test]
    fn test_execute_vim_count() {
        let mut editor = Editor::new("test.md", "hello\nworld\nfoo");
        let result = editor.execute_vim("2j");
        assert!(result.success);
        assert_eq!(result.cursor, Position::new(2, 0));
    }

    #[test]
    fn test_execute_vim_delete_line() {
        let mut editor = Editor::new("test.md", "line1\nline2\nline3");
        let result = editor.execute_vim("dd");
        assert!(result.success);
        assert_eq!(editor.content(), "line2\nline3");
    }

    #[test]
    fn test_execute_vim_insert() {
        let mut editor = Editor::new("test.md", "hello");
        let result = editor.execute_vim("i");
        assert!(result.success);
        assert_eq!(editor.mode(), Mode::Insert);
    }

    #[test]
    fn test_execute_vim_save() {
        let mut editor = Editor::new("test.md", "hello");
        let result = editor.execute_vim(":w");
        assert!(result.success);
    }

    #[test]
    fn test_execute_vim_visual_mode() {
        let mut editor = Editor::new("test.md", "hello");
        let result = editor.execute_vim("v");
        assert!(result.success);
        assert_eq!(editor.mode(), Mode::Visual);
    }

    #[test]
    fn test_execute_vim_register() {
        let mut editor = Editor::new("test.md", "hello\nworld");
        let result = editor.execute_vim("yy");
        assert!(result.success);
        assert!(editor.state().registers.contains_key(&'"'));
    }

    #[test]
    fn test_execute_vim_complex_command() {
        let mut editor = Editor::new("test.md", "line1\nline2\nline3");
        let result = editor.execute_vim("2dd");
        assert!(result.success);
        assert_eq!(editor.content(), "line3");
    }

    #[test]
    fn test_execute_vim_word_motion() {
        let mut editor = Editor::new("test.md", "hello world foo");
        let result = editor.execute_vim("w");
        assert!(result.success);
        assert_eq!(result.cursor, Position::new(0, 6));
    }
}
