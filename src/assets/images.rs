use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, ImageDecoder, ImageEncoder};

const MAX_GIF_FRAMES: usize = 60;

pub use crate::assets::asset_cache::ImageData;
use crate::assets::asset_cache::AssetCache;

static IMAGE_CACHE: std::sync::LazyLock<AssetCache> = std::sync::LazyLock::new(AssetCache::new);

pub fn resolve_path(url: &str, base_dir: Option<&Path>) -> Result<PathBuf> {
    if url.starts_with("http://") || url.starts_with("https://") {
        anyhow::bail!("Remote images not yet supported: {url}")
    }
    let path = PathBuf::from(url);

    if path.is_absolute() {
        if path.exists() {
            return Ok(path);
        }

        anyhow::bail!("Image not found: {}", path.display());
    }

    if let Some(base) = base_dir {
        let resolved = base.join(&path);
        if resolved.exists() {
            return Ok(resolved);
        }
    }

    let cwd = std::env::current_dir()?;
    let resolved = cwd.join(&path);
    if resolved.exists() {
        return Ok(resolved);
    }

    anyhow::bail!("Image not found: {url}")
}

pub fn load_image(url: &str, base_dir: Option<&Path>, max_width: u32) -> Result<ImageData> {
    let path = resolve_path(url, base_dir)?;

    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    max_width.hash(&mut hasher);
    if let Ok(meta) = std::fs::metadata(&path) {
        if let Ok(mtime) = meta.modified() {
            mtime.hash(&mut hasher);
        }
    }
    let cache_key = hasher.finish();

    if let Some(cached) = IMAGE_CACHE.get(cache_key) {
        return Ok(cached);
    }

    let is_gif = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"));

    let image_data = if is_gif {
        load_gif_frames(&path, max_width)?
    } else {
        load_static_png(&path, max_width)?
    };

    IMAGE_CACHE.insert(cache_key, image_data.clone());
    Ok(image_data)
}

fn load_static_png(path: &Path, max_width: u32) -> Result<ImageData> {
    let img = image::open(path)?.to_rgba8();

    let img = if img.width() > max_width {
        let ratio = max_width as f32 / img.width() as f32;
        let new_h = (img.height() as f32 * ratio) as u32;
        image::imageops::resize(
            &img,
            max_width,
            new_h,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };
    let (w, h) = (img.width(), img.height());

    let mut png_buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
    encoder.write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgba8)?;

    Ok(ImageData::Png(png_buf, w, h))
}

fn load_gif_frames(path: &Path, max_width: u32) -> Result<ImageData> {
    let file = std::fs::File::open(path)?;
    let decoder = GifDecoder::new(BufReader::new(file))?;
    let (orig_w, orig_h) = decoder.dimensions();
    let raw_frames = decoder.into_frames().collect_frames()?;

    let total = raw_frames.len();

    // Sample evenly if more than MAX_GIF_FRAMES
    let selected: Vec<usize> = if total > MAX_GIF_FRAMES {
        (0..MAX_GIF_FRAMES)
            .map(|i| (i * total) / MAX_GIF_FRAMES)
            .collect()
    } else {
        (0..total).collect()
    };

    if selected.is_empty() {
        bail!("No frames in GIF");
    }

    let ((out_w, out_h), scale) = if orig_w > max_width {
        let s = max_width as f32 / orig_w as f32;
        ((max_width, (orig_h as f32 * s) as u32), s)
    } else {
        ((orig_w, orig_h), 1.0)
    };

    let mut frames = Vec::with_capacity(selected.len());
    for idx in &selected {
        let frame = &raw_frames[*idx];
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = if denom > 0 { numer / denom } else { 0 };
        let mut rgba = frame.clone().into_buffer();

        if scale != 1.0 {
            rgba = image::imageops::resize(&rgba, out_w, out_h, image::imageops::FilterType::Lanczos3);
        }

        let mut png_buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
        encoder.write_image(rgba.as_raw(), out_w, out_h, image::ExtendedColorType::Rgba8)?;

        frames.push((png_buf, delay_ms));
    }

    if frames.len() <= 1 {
        let (data, _) = frames.into_iter().next().unwrap_or_default();
        return Ok(ImageData::Png(data, out_w, out_h));
    }

    Ok(ImageData::Gif {
        frames,
        width: out_w,
        height: out_h,
    })
}
