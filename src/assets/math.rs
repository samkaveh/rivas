use std::sync::OnceLock;

use anyhow::Result;
use typst::{
    Library, LibraryExt, compile,
    foundations::Bytes,
    layout::PagedDocument,
    syntax::{FileId, Source},
    text::{Font, FontBook},
    utils::LazyHash,
};

use crate::assets::svg::rasterize_svg_to_png;

pub fn render_math(
    latex: &str,
    display: bool,
    max_width: u32,
    dark_theme: bool,
) -> Result<(Vec<u8>, u32, u32)> {
    // Convert LaTeX → Typst math
    let typst_math =
        mitex::convert_text(latex, None).map_err(|e| anyhow::anyhow!("mitex error: {:?}", e))?;

    let source = build_typst_source(&typst_math, display, dark_theme);

    let document = compile_typst(&source)?;

    let svg = typst_svg::svg(&document.pages[0]);
    rasterize_svg_to_png(&svg, max_width)
}

fn build_typst_source(typst_math: &str, display: bool, dark_theme: bool) -> String {
    let color = if dark_theme { "white" } else { "black" };
    let size = if display { "16pt" } else { "14pt" };
    let margin = if display { "12pt" } else { "4pt" };

    format!(
        "#set page(width: auto, height: auto, margin: {margin})\n\
         #set text(fill: {color}, size: {size})\n\
         ${typst_math}$"
    )
}

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

    let book = FontBook::from_fonts(&fonts);
    (book, fonts)
}
