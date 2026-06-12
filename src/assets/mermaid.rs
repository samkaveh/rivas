use crate::assets::svg::rasterize_svg_to_png;
use anyhow::Result;
use selkie::{RenderConfig, parse, render_with_config};

pub fn render_mermaid_to_png(source: &str, max_width: u32) -> Result<(Vec<u8>, u32, u32)> {
    let mut render_config = RenderConfig::default();
    render_config.theme.font_family = "DejaVu Sans".to_string();
    let diagram = parse(source)?;
    let mut svg = render_with_config(&diagram, &render_config)?;

    let style_override = r#"<defs><style>
    text, tspan, .label { fill: #A15EED !important; }
    .edgeLabel { color: #A15EED !important; }
    line, path { stroke: #CCCCCC !important; }
    </style></defs>"#;

    if let Some(svg_start) = svg.find("<svg") {
        if let Some(pos) = svg[svg_start..].find('>') {
            svg.insert_str(svg_start + pos + 1, style_override);
        }
    }

    rasterize_svg_to_png(&svg, max_width)
}
