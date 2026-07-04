use std::path::{Path, PathBuf};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

use anyhow::Result;
use image::ImageEncoder;

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

pub fn load_image_to_png(
    url: &str,
    base_dir: Option<&Path>,
    max_width: u32,
) -> Result<(Vec<u8>, u32, u32)> {
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

    let img = image::open(&path)?.to_rgba8();

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

    IMAGE_CACHE.insert(cache_key, png_buf.clone(), w, h);
    Ok((png_buf, w, h))
}
