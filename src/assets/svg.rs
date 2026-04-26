use anyhow::Result;
use resvg::usvg;

pub fn rasterize_svg_to_png(svg: &str, max_width: u32) -> Result<(Vec<u8>, u32, u32)> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())?;
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
