use crate::assets::svg::rasterize_svg_to_png;
use anyhow::Result;

pub fn render_mermaid_to_png(source: &str, max_width: u32) -> Result<(Vec<u8>, u32, u32)> {
    let svg =
        mermaid_rs_renderer::render(source).map_err(|e| anyhow::anyhow!("Mermaid: {:?}", e))?;
    rasterize_svg_to_png(&svg, max_width)
}
