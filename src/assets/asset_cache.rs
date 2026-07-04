use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum ImageData {
    Png(Vec<u8>, u32, u32),
    Gif {
        frames: Vec<(Vec<u8>, u32)>,
        width: u32,
        height: u32,
    },
}

impl ImageData {
    pub fn width(&self) -> u32 {
        match self {
            ImageData::Png(_, w, _) => *w,
            ImageData::Gif { width, .. } => *width,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            ImageData::Png(_, _, h) => *h,
            ImageData::Gif { height, .. } => *height,
        }
    }
}

struct AssetEntry {
    image: ImageData,
}

pub struct AssetCache {
    cache: Arc<Mutex<HashMap<u64, AssetEntry>>>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: u64) -> Option<ImageData> {
        let cache = self.cache.lock().ok()?;
        cache.get(&key).map(|e| e.image.clone())
    }

    pub fn insert(&self, key: u64, image: ImageData) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, AssetEntry { image });
            if cache.len() > 64 {
                let to_remove = cache.len() / 2;
                let keys: Vec<u64> = cache.keys().copied().collect();
                for key in keys.iter().take(to_remove) {
                    cache.remove(key);
                }
            }
        }
    }
}
