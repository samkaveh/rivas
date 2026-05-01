
use crate::assets::svg::rasterize_svg_to_png;
use anyhow::Result;
use mermaid_rs_renderer::{LayoutConfig, Theme, compute_layout, parse_mermaid, render_svg};

pub fn render_mermaid_to_png(source: &str, max_width: u32) -> Result<(Vec<u8>, u32, u32)> {
    let parsed = parse_mermaid(source).map_err(|e| anyhow::anyhow!("Mermaid: {:?}", e))?;
    let mut theme = Theme::mermaid_default();
    theme.font_family = "DejaVu Sans".to_string();
    let config = LayoutConfig::default();
    let layout = compute_layout(&parsed.graph, &theme, &config);
    let svg = render_svg(&layout, &theme, &config);
    rasterize_svg_to_png(&svg, max_width)
}
