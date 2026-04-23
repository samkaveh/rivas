use ratatui::style::{Color, Modifier, Style};
pub struct Theme {
    pub heading: Style,
    pub heading_h1: Style,
    pub heading_h2: Style,
    pub heading_h3: Style,
    pub text: Style,
    pub bold: Modifier,
    pub italic: Modifier,
    pub strikethrough: Modifier,
    pub link: Style,
    pub inline_code: Style,
    pub code_block_bg: Color,
    pub code_block_text: Style,
    pub blockquote_bar: Style,
    pub blockquote_text: Style,
    pub list_marker: Style,
    pub rule: Style,
    pub table_border: Style,
    pub table_header: Style,
    pub placeholder: Style,
    pub blanck_line: Style,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            heading: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            heading_h1: Style::default()
                .fg(Color::Rgb(88, 166, 255))
                .add_modifier(Modifier::BOLD),
            heading_h2: Style::default()
                .fg(Color::Rgb(139, 233, 253))
                .add_modifier(Modifier::BOLD),
            heading_h3: Style::default()
                .fg(Color::Rgb(80, 250, 123))
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::Rgb(230, 237, 243)),
            bold: Modifier::BOLD,
            italic: Modifier::ITALIC,
            strikethrough: Modifier::CROSSED_OUT,
            link: Style::default()
                .fg(Color::Rgb(88, 166, 255))
                .add_modifier(Modifier::UNDERLINED),
            inline_code: Style::default()
                .fg(Color::Rgb(230, 237, 243))
                .bg(Color::Rgb(30, 36, 44)),
            code_block_bg: Color::Rgb(22, 27, 34),
            code_block_text: Style::default().fg(Color::Rgb(230, 237, 243)),
            blockquote_bar: Style::default().fg(Color::Rgb(48, 54, 61)),
            blockquote_text: Style::default().fg(Color::Rgb(139, 148, 158)),
            list_marker: Style::default().fg(Color::Rgb(139, 148, 158)),
            rule: Style::default().fg(Color::Rgb(48, 54, 61)),
            table_border: Style::default().fg(Color::Rgb(48, 54, 61)),
            table_header: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            placeholder: Style::default()
                .fg(Color::Rgb(88, 166, 255))
                .add_modifier(Modifier::ITALIC),
            blanck_line: Style::default(),
        }
    }
    pub fn light() -> Self {
        Self {
            heading: Style::default()
                .fg(Color::Rgb(31, 35, 40))
                .add_modifier(Modifier::BOLD),
            heading_h1: Style::default()
                .fg(Color::Rgb(9, 105, 218))
                .add_modifier(Modifier::BOLD),
            heading_h2: Style::default()
                .fg(Color::Rgb(31, 35, 40))
                .add_modifier(Modifier::BOLD),
            heading_h3: Style::default()
                .fg(Color::Rgb(31, 35, 40))
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::Rgb(31, 35, 40)),
            link: Style::default()
                .fg(Color::Rgb(9, 105, 218))
                .add_modifier(Modifier::UNDERLINED),
            inline_code: Style::default()
                .fg(Color::Rgb(31, 35, 40))
                .bg(Color::Rgb(235, 238, 242)),
            code_block_bg: Color::Rgb(246, 248, 250),
            code_block_text: Style::default().fg(Color::Rgb(31, 35, 40)),
            blockquote_bar: Style::default().fg(Color::Rgb(208, 215, 222)),
            blockquote_text: Style::default().fg(Color::Rgb(101, 109, 118)),
            table_border: Style::default().fg(Color::Rgb(208, 215, 222)),
            ..Self::dark()
        }
    }

    pub fn heading_style(&self, level: u8) -> Style {
        match level {
            1 => self.heading_h1,
            2 => self.heading_h2,
            3 => self.heading_h3,
            _ => self.heading,
        }
    }
}
