use super::layout::*;
use super::theme::Theme;
use cosmic_text::{Buffer, Color, FontSystem, SwashCache, SwashContent, SwashImage};
use tiny_skia::Pixmap;

pub fn pain_document(
    layout: &[LayoutBlock],
    theme: &Theme,
    width: u32,
    height: u32,
    scroll_y: f32,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(theme.bg.to_skia());

    for block in layout {
        let screen_y = block.y - scroll_y;
        if screen_y + block.height < 0.0 {
            continue;
        }
        if screen_y > height as f32 {
            break;
        }

        paint_block(
            &mut pixmap,
            block,
            scroll_y,
            theme,
            font_system,
            swash_cache,
        );
    }

    pixmap
}

fn paint_block(
    pixmap: &mut Pixmap,
    block: &LayoutBlock,
    scroll_y: f32,
    theme: &Theme,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
) {
    let dy = -scroll_y;
    match &block.content {
        LayoutContent::Text { buffer, x_offset } => paint_buffer(
            pixmap,
            buffer,
            *x_offset,
            block.y + dy,
            theme.text.to_cosmic(),
            font_system,
            swash_cache,
        ),
        _ => (),
    }
}

fn paint_buffer(
    pixmap: &mut Pixmap,
    buffer: &Buffer,
    offset_x: f32,
    offset_y: f32,
    default_color: Color,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
) {
    let pixmap_w = pixmap.width() as i32;
    let pixmap_h = pixmap.height() as i32;

    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            // Convert phsyical pixel coordinates
            let phsyical = glyph.physical((offset_x, offset_y), 1.0);
            let color = glyph.color_opt.unwrap_or(default_color);

            // Rasterize the glyph (or fetch from cache)
            let Some(image) = swash_cache.get_image_uncached(font_system, phsyical.cache_key)
            else {
                continue;
            };

            // Top-left corner of the glyph bitmap in the target pixmap
            let gx = phsyical.x + image.placement.left;
            let gy = phsyical.y - image.placement.top;

            match image.content {
                SwashContent::Mask => {
                    // Grayscale alpha mask - colorize with glyph color
                    paint_mask(pixmap, &image, gx, gy, color, pixmap_w, pixmap_w);
                }
                SwashContent::Color => {
                    println!("{:?}", image.content);
                    // Pre-render color glyph (emoji)
                    paint_color_glyph(pixmap, &image, gx, gy, pixmap_w, pixmap_h);
                }
                SwashContent::SubpixelMask => {
                    // LCD sub-pixel - treat as grayscale mask for simplicity
                    paint_mask(pixmap, &image, gx, gy, color, pixmap_w, pixmap_h);
                }
            }
        }
    }
}

/// Blend a grayscale alpha mask glyph onto the pixmap with the given color
fn paint_mask(
    pixmap: &mut Pixmap,
    image: &SwashImage,
    gx: i32,
    gy: i32,
    color: Color,
    pw: i32,
    ph: i32,
) {
    let r = color.r();
    let g = color.g();
    let b = color.b();
    let iw = image.placement.width as i32;

    for dy in 0..image.placement.height as i32 {
        let py = gy + dy;
        if py < 0 || py >= ph {
            continue;
        }
        for dx in 0..iw {
            let px = gx + dx;
            if px < 0 || px >= pw {
                continue;
            }

            let alpha = image.data[(dy * iw + dx) as usize];
            if alpha == 0 {
                continue;
            }

            let idx = (py as usize * pw as usize + px as usize) * 4;
            let data = pixmap.data_mut();
            let a = alpha as f32 / 255.0;

            // Source-over compositioning (premultiplied alpha)
            data[idx + 0] = ((r as f32 * a) + data[idx + 0] as f32 * (1.0 - a)) as u8;
            data[idx + 1] = ((g as f32 * a) + data[idx + 1] as f32 * (1.0 - a)) as u8;
            data[idx + 2] = ((b as f32 * a) + data[idx + 2] as f32 * (1.0 - a)) as u8;
            data[idx + 3] =
                ((alpha as f32 * a) + data[idx + 3] as f32 * (1.0 - a)).min(255.0) as u8;
        }
    }
}

/// Blen a pre-rendered color glyph (emoji, color font) onto pixmap
fn paint_color_glyph(pixmap: &mut Pixmap, image: &SwashImage, gx: i32, gy: i32, pw: i32, ph: i32) {
    let iw = image.placement.width as i32;

    for dy in 0..image.placement.height as i32 {
        let py = gy + dy;
        if py < 0 || py >= ph {
            continue;
        }
        for dx in 0..iw {
            let px = gx + dx;
            if px < 0 || px >= pw {
                continue;
            }

            let si = (dy * iw + dx) as usize * 4;
            let di = (py as usize * pw as usize + px as usize) * 4;
            let sa = image.data[si + 3];
            if sa == 0 {
                continue;
            }

            let a = sa as f32 / 255.0;
            let data = pixmap.data_mut();
            data[di + 0] = ((image.data[si] as f32 * a) + data[di + 0] as f32 * (1.0 - a)) as u8;
            data[di + 1] =
                ((image.data[si + 1] as f32 * a) + data[di + 1] as f32 * (1.0 - a)) as u8;
            data[di + 2] =
                ((image.data[si + 2] as f32 * a) + data[di + 2] as f32 * (1.0 - a)) as u8;
            data[di + 3] = ((image.data[si + 3] as f32 * a) + data[di + 3] as f32 * (1.0 - a))
                .min(255.0) as u8;
        }
    }
}
