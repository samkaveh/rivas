/// RGBA color (0-255 per channel)
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub fn to_cosmic(&self) -> cosmic_text::Color {
        cosmic_text::Color::rgba(self.r, self.g, self.b, self.a)
    }
    pub fn to_skia(&self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

pub struct Theme {
    // Page
    pub bg: Color,
    pub text: Color,
    pub max_content_width: f32,
    pub padding: f32,

    // Typography
    pub font_family: &'static str,
    pub code_font_family: &'static str,
    pub body_size: f32,
    pub h1_size: f32,
    pub h2_size: f32,
    pub h3_size: f32,
    pub h4_size: f32,
    pub line_height_factor: f32, // multiply by font_size to get line_height

    // Colors
    pub heading: Color,
    pub link: Color,
    pub code_bg: Color,
    pub code_text: Color,
    pub inline_code_bg: Color,
    pub blockquote_bar: Color,
    pub blockquote_text: Color,
    pub rule: Color,
    pub table_border: Color,
    pub table_header_bg: Color,

    // Spacing
    pub block_spacing: f32,
    pub heading_top_spacing: f32,
    pub list_indent: f32,
    pub blockquote_indent: f32,
    pub code_padding: f32,
    pub code_corner_radius: f32,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg: Color::rgb(13, 17, 23),
            text: Color::rgb(230, 237, 243),
            max_content_width: 900.0,
            padding: 24.0,

            font_family: "sans-serif",
            code_font_family: "monospace",
            body_size: 16.0,
            h1_size: 32.0,
            h2_size: 24.0,
            h3_size: 20.0,
            h4_size: 16.0,
            line_height_factor: 1.5,

            heading: Color::rgb(230, 237, 243),
            link: Color::rgb(88, 166, 255),
            code_bg: Color::rgb(22, 27, 34),
            code_text: Color::rgb(230, 237, 243),
            inline_code_bg: Color::rgb(30, 36, 44),
            blockquote_bar: Color::rgb(48, 54, 61),
            blockquote_text: Color::rgb(139, 148, 158),
            rule: Color::rgb(48, 54, 61),
            table_border: Color::rgb(48, 54, 61),
            table_header_bg: Color::rgb(22, 27, 34),

            block_spacing: 16.0,
            heading_top_spacing: 24.0,
            list_indent: 24.0,
            blockquote_indent: 16.0,
            code_padding: 12.0,
            code_corner_radius: 6.0,
        }
    }

    pub fn light() -> Self {
        Self {
            bg: Color::rgb(255, 255, 255),
            text: Color::rgb(31, 35, 40),
            heading: Color::rgb(31, 35, 40),
            link: Color::rgb(9, 105, 218),
            code_bg: Color::rgb(246, 248, 250),
            code_text: Color::rgb(31, 35, 40),
            inline_code_bg: Color::rgb(235, 238, 242),
            blockquote_bar: Color::rgb(208, 215, 222),
            blockquote_text: Color::rgb(101, 109, 118),
            rule: Color::rgb(208, 215, 222),
            table_border: Color::rgb(208, 215, 222),
            table_header_bg: Color::rgb(246, 248, 250),
            ..Self::dark()
        }
    }

    /// Font size for a heading level
    pub fn heading_size(&self, level: u8) -> f32 {
        match level {
            1 => self.h1_size,
            2 => self.h2_size,
            3 => self.h3_size,
            _ => self.h4_size,
        }
    }

    /// Metrics (font_size, line_height) for body text
    pub fn body_metrics(&self) -> cosmic_text::Metrics {
        cosmic_text::Metrics::new(self.body_size, self.body_size * self.line_height_factor)
    }

    /// Metrics for a heading
    pub fn heading_metrics(&self, level: u8) -> cosmic_text::Metrics {
        let s = self.heading_size(level);
        cosmic_text::Metrics::new(s, s * self.line_height_factor)
    }

    /// Metrics for code
    pub fn code_metrics(&self) -> cosmic_text::Metrics {
        cosmic_text::Metrics::new(
            self.body_size * 0.875,
            self.body_size * self.line_height_factor,
        )
    }
}
