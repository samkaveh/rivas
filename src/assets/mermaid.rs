use crate::assets::svg::rasterize_svg_to_png;
use anyhow::Result;
use selkie::{RenderConfig, parse, render_with_config};

pub fn render_mermaid_to_png(source: &str, max_width: u32) -> Result<(Vec<u8>, u32, u32)> {
    let mut render_config = RenderConfig::default();
    render_config.theme.font_family = "DejaVu Sans".to_string();
    let diagram = parse(source)?;
    let svg = render_with_config(&diagram, &render_config)?;
    rasterize_svg_to_png(&svg, max_width)
}
