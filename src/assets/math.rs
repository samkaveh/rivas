use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use crate::assets::asset_cache::AssetCache;
use crate::assets::svg::rasterize_svg_to_png;
use anyhow::Result;
use tylax::latex_to_typst;
use typst::{
    Library, LibraryExt, compile,
    foundations::Bytes,
    layout::PagedDocument,
    syntax::{FileId, Source},
    text::{Font, FontBook},
    utils::LazyHash,
};

static MATH_CACHE: std::sync::LazyLock<AssetCache> = std::sync::LazyLock::new(AssetCache::new);

pub fn render_math(
    latex: &str,
    display: bool,
    max_width: u32,
    dark_theme: bool,
) -> Result<(Vec<u8>, u32, u32)> {
    let mut hasher = DefaultHasher::new();
    latex.hash(&mut hasher);
    display.hash(&mut hasher);
    max_width.hash(&mut hasher);
    dark_theme.hash(&mut hasher);
    let cache_key = hasher.finish();

    if let Some(cached) = MATH_CACHE.get(cache_key) {
        return Ok(cached);
    }

    let typst_math = latex_to_typst(latex);

    let source = build_typst_source(&typst_math, display, dark_theme);

    let document = compile_typst(&source)?;

    let svg = typst_svg::svg(&document.pages[0]);
    let result = rasterize_svg_to_png(&svg, max_width)?;
    MATH_CACHE.insert(cache_key, result.0.clone(), result.1, result.2);
    Ok(result)
}

fn build_typst_source(typst_math: &str, display: bool, dark_theme: bool) -> String {
    let color = if dark_theme { "white" } else { "black" };
    let size = if display { "16pt" } else { "14pt" };
    let margin = if display { "12pt" } else { "4pt" };
    let block = if display { "true" } else { "false" };

    format!(
        "{MITEX_PRELUDE}\n\
         #set page(width: auto, height: auto, margin: {margin}, fill: none)\n\
         #set text(font: \"New Computer Modern\", fill: {color}, size: {size})\n\
         #math.equation(block: {block}, ${typst_math}$)"
    )
}

const MITEX_PRELUDE: &str = r#"
#let textmath(body) = text(body)
#let mitexdisplay(body) = body
#let mitexsqrt(first, second: none) = if second == none { math.sqrt(first) } else { math.root(first, second) }
#let mitexoverbrace(body) = math.overbrace(body)
#let mitexunderbrace(body) = math.underbrace(body)
#let mitexcolor(color, body) = body
#let colortext(color, body) = body
#let colorbox(color, body) = body
#let mitexmathbf(body) = math.bold(body)
#let zws = ""
"#;

static FONTS: OnceLock<(FontBook, Vec<Font>)> = OnceLock::new();

struct MathWorld {
    source: Source,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

impl MathWorld {
    fn new(source_text: &str) -> Self {
        let (book, fonts) = FONTS.get_or_init(load_bundled_fonts);

        Self {
            source: Source::detached(source_text),
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book.clone()),
            fonts: fonts.clone(),
        }
    }
}

impl typst::World for MathWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(typst::diag::FileError::NotFound(
                id.vpath().as_rootless_path().into(),
            ))
        }
    }

    fn file(&self, _: FileId) -> typst::diag::FileResult<Bytes> {
        Err(typst::diag::FileError::NotFound("not found".into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _: Option<i64>) -> Option<typst::foundations::Datetime> {
        None
    }
}

fn compile_typst(source: &str) -> Result<PagedDocument> {
    let world = MathWorld::new(source);
    let result = compile(&world);

    result.output.map_err(|errors| {
        let messages: Vec<String> = errors.iter().map(|e| e.message.to_string()).collect();
        anyhow::anyhow!("typst compilation failed: {}", messages.join("; "))
    })
}

//
// Embedded fonts (portable)
//

static FONT_DEJAVU_MATH: &[u8] = include_bytes!("../assets/fonts/DejaVuMathTeXGyre.ttf");
static FONT_DEJAVU_SANS: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
static FONT_DEJAVU_SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");
static FONT_DEJAVU_SANS_MONO: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono.ttf");

fn load_bundled_fonts() -> (FontBook, Vec<Font>) {
    let mut fonts = Vec::new();

    for font_data in [
        FONT_DEJAVU_MATH,
        FONT_DEJAVU_SANS,
        FONT_DEJAVU_SANS_BOLD,
        FONT_DEJAVU_SANS_MONO,
    ] {
        let bytes = Bytes::new(font_data);
        for font in Font::iter(bytes) {
            fonts.push(font);
        }
    }
    for font_data in typst_assets::fonts() {
        let bytes = Bytes::new(font_data);
        for font in Font::iter(bytes) {
            fonts.push(font);
        }
    }

    let book = FontBook::from_fonts(&fonts);
    (book, fonts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_png(data: &[u8], width: u32, height: u32) {
        assert!(data.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(width > 0);
        assert!(height > 0);
    }

    fn assert_has_non_white_or_transparent_pixels(data: &[u8]) {
        let image = image::load_from_memory(data).unwrap().to_rgba8();
        assert!(image.pixels().any(|pixel| {
            let [r, g, b, a] = pixel.0;
            a < 255 || r < 250 || g < 250 || b < 250
        }));
    }

    #[test]
    fn loads_bundled_fonts() {
        let (book, fonts) = load_bundled_fonts();
        let families: Vec<_> = book
            .families()
            .map(|(family, _)| family.to_string())
            .collect();

        assert!(!fonts.is_empty());
        assert!(
            families
                .iter()
                .any(|family| family == "DejaVu Math TeX Gyre")
        );
        assert!(
            families
                .iter()
                .any(|family| family == "New Computer Modern")
        );
    }

    #[test]
    fn renders_common_math_to_png() {
        let (png, width, height) =
            render_math(r"\frac{1}{2} + \sqrt{x}", true, 800, false).expect("math should render");

        assert_png(&png, width, height);
        assert_has_non_white_or_transparent_pixels(&png);
    }

    #[test]
    fn dark_theme_math_uses_transparent_page() {
        let (png, width, height) =
            render_math(r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", true, 800, true)
                .expect("math should render");

        assert_png(&png, width, height);
        assert_has_non_white_or_transparent_pixels(&png);
    }

    #[test]
    fn max_width_limits_rendered_png_width() {
        let (png, width, height) = render_math(r"x + y + z + \int_0^1 t^2 dt", true, 40, false)
            .expect("math should render");

        assert_png(&png, width, height);
        assert!(width <= 40);
    }
}
