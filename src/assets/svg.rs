use anyhow::Result;
use resvg::usvg;

pub fn rasterize_svg_to_png(svg: &str, max_width: u32) -> Result<(Vec<u8>, u32, u32)> {
    let mut opt = usvg::Options::default();

    // Embed font at compile time
    let font_data = include_bytes!("../assets/fonts/DejaVuSans.ttf");

    // Load into usvg font database
    opt.fontdb_mut().load_font_data(font_data.to_vec());

    // Set fallback
    opt.font_family = "DejaVu Sans".to_string();

    let tree = usvg::Tree::from_str(svg, &opt)?;
    let size = tree.size();
    let scale = (max_width as f32 / size.width()).min(1.0);
    let w = (size.width() * scale).ceil() as u32;
    let h = (size.height() * scale).ceil() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w.max(1), h.max(1))
        .ok_or_else(|| anyhow::anyhow!("pixmap allocation failed ({}x{})", w, h))?;

    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let png = pixmap.encode_png()?;
    Ok((png, w, h))
}
