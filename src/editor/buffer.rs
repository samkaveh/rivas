#[derive(Clone, Debug)]
pub struct Buffer {
    pub lines: Vec<String>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum CharClass {
    Word,
    Punct,
    Whitespace,
}

pub fn char_class(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Punct
    }
}

impl Buffer {
    pub fn new(text: &str) -> Self {
        let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self { lines }
    }

    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line(&self, row: usize) -> &str {
        &self.lines[row.min(self.lines.len().saturating_sub(1))]
    }

    pub fn char_count(&self, row: usize) -> usize {
        self.line(row).chars().count()
    }

    pub fn clamp_col(&self, row: usize, col: usize, insert: bool) -> usize {
        let len = self.char_count(row);
        if insert {
            col.min(len)
        } else if len == 0 {
            0
        } else {
            col.min(len - 1)
        }
    }

    pub fn byte_offset(&self, row: usize, col: usize) -> usize {
        self.line(row)
            .char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(self.line(row).len())
    }

    pub fn insert_char(&mut self, row: usize, col: usize, ch: char) {
        while row >= self.lines.len() {
            self.lines.push(String::new());
        }
        let byte = self.byte_offset(row, col);
        self.lines[row].insert(byte, ch);
    }

    pub fn insert_text(&mut self, row: usize, col: usize, text: &str) -> (usize, usize) {
        if text.is_empty() {
            return (row, col);
        }
        let start_byte = self.byte_offset(row, col);
        let line = &self.lines[row];
        let left = line[..start_byte].to_string();
        let right = line[start_byte..].to_string();

        let parts: Vec<&str> = text.split('\n').collect();
        if parts.len() == 1 {
            let new_line = format!("{}{}{}", left, parts[0], right);
            self.lines[row] = new_line;
            let end_col = col + parts[0].chars().count();
            (row, end_col.saturating_sub(1))
        } else {
            self.lines[row] = format!("{}{}", left, parts[0]);
            let num_parts = parts.len();
            for i in 1..(num_parts - 1) {
                self.lines.insert(row + i, parts[i].to_string());
            }
            let last_line = format!("{}{}", parts[num_parts - 1], right);
            self.lines.insert(row + num_parts - 1, last_line);
            let end_row = row + num_parts - 1;
            let end_col = parts[num_parts - 1].chars().count();
            (end_row, end_col.saturating_sub(1))
        }
    }

    pub fn delete_char(&mut self, row: usize, col: usize) -> Option<char> {
        if col >= self.char_count(row) {
            return None;
        }
        let byte = self.byte_offset(row, col);
        Some(self.lines[row].remove(byte))
    }

    pub fn split_line(&mut self, row: usize, col: usize) {
        let byte = self.byte_offset(row, col);
        let rest = self.lines[row].split_off(byte);
        self.lines.insert(row + 1, rest);
    }

    pub fn join_lines(&mut self, row: usize) {
        if row + 1 < self.lines.len() {
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next);
        }
    }

    pub fn delete_line(&mut self, row: usize) -> String {
        if self.lines.len() == 1 {
            let s = self.lines[0].clone();
            self.lines[0].clear();
            s
        } else {
            self.lines.remove(row)
        }
    }

    pub fn insert_line(&mut self, row: usize, content: String) {
        self.lines.insert(row, content);
    }

    pub fn replace_range_on_line(&mut self, row: usize, col_start: usize, col_end: usize, s: &str) {
        let start = self.byte_offset(row, col_start);
        let end = self.byte_offset(row, col_end);
        let mut new = self.lines[row][..start].to_string();
        new.push_str(s);
        new.push_str(&self.lines[row][end..]);
        self.lines[row] = new;
    }

    pub fn word_forward(&self, row: usize, col: usize) -> (usize, usize) {
        let chars: Vec<char> = self.line(row).chars().collect();
        if chars.is_empty() {
            if row + 1 < self.line_count() {
                return (row + 1, 0);
            }
            return (row, 0);
        }

        let mut c = col;
        let start_class = char_class(chars[c]);

        if start_class != CharClass::Whitespace {
            while c < chars.len() && char_class(chars[c]) == start_class {
                c += 1;
            }
        }

        while c < chars.len() && char_class(chars[c]) == CharClass::Whitespace {
            c += 1;
        }

        if c >= chars.len() {
            if row + 1 < self.line_count() {
                (row + 1, self.first_non_blank(row + 1))
            } else {
                (row, chars.len().saturating_sub(1))
            }
        } else {
            (row, c)
        }
    }

    pub fn word_backward(&self, row: usize, col: usize) -> (usize, usize) {
        if col == 0 {
            if row > 0 {
                let prev_row = row - 1;
                return (prev_row, self.char_count(prev_row).saturating_sub(1));
            }
            return (0, 0);
        }

        let chars: Vec<char> = self.line(row).chars().collect();
        let mut c = col as isize - 1;

        while c >= 0 && char_class(chars[c as usize]) == CharClass::Whitespace {
            c -= 1;
        }

        if c < 0 {
            if row > 0 {
                let prev_row = row - 1;
                return (prev_row, self.char_count(prev_row).saturating_sub(1));
            }
            return (row, 0);
        }

        let target_class = char_class(chars[c as usize]);
        while c > 0 && char_class(chars[(c - 1) as usize]) == target_class {
            c -= 1;
        }

        (row, c as usize)
    }

    pub fn word_end(&self, row: usize, col: usize) -> (usize, usize) {
        let chars: Vec<char> = self.line(row).chars().collect();
        if chars.is_empty() {
            if row + 1 < self.line_count() {
                return self.word_end(row + 1, 0);
            }
            return (row, 0);
        }

        let mut c = col + 1;
        while c < chars.len() && char_class(chars[c]) == CharClass::Whitespace {
            c += 1;
        }

        if c >= chars.len() {
            if row + 1 < self.line_count() {
                return self.word_end(row + 1, 0);
            }
            return (row, chars.len().saturating_sub(1));
        }

        let target_class = char_class(chars[c]);
        while c + 1 < chars.len() && char_class(chars[c + 1]) == target_class {
            c += 1;
        }

        (row, c)
    }

    pub fn find_forward(
        &self,
        row: usize,
        col: usize,
        target: char,
        before: bool,
    ) -> Option<usize> {
        let chars: Vec<char> = self.line(row).chars().collect();
        for i in (col + 1)..chars.len() {
            if chars[i] == target {
                return Some(if before { i.saturating_sub(1) } else { i });
            }
        }
        None
    }

    pub fn find_backward(
        &self,
        row: usize,
        col: usize,
        target: char,
        before: bool,
    ) -> Option<usize> {
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

    pub fn first_non_blank(&self, row: usize) -> usize {
        self.line(row)
            .chars()
            .take_while(|c| c.is_whitespace())
            .count()
    }

    pub fn search_forward(
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
            let from_byte = if offset == 0 {
                let here = self.byte_offset(row, start_col);
                line[here..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| here + i)
                    .unwrap_or(line.len())
            } else {
                0
            };

            if let Some(pos) = line[from_byte..].find(pat) {
                let match_byte = from_byte + pos;
                let col = line[..match_byte].chars().count();
                return Some((row, col));
            }
        }
        None
    }

    pub fn search_backward(
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
            let to_byte = if offset == 0 {
                self.byte_offset(row, start_col)
            } else {
                line.len()
            };

            if let Some(pos) = line[..to_byte].rfind(pat) {
                let col = line[..pos].chars().count();
                return Some((row, col));
            }
        }
        None
    }
}
