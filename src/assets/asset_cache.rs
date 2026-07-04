use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct AssetEntry {
    data: Vec<u8>,
    width: u32,
    height: u32,
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

    pub fn get(&self, key: u64) -> Option<(Vec<u8>, u32, u32)> {
        let cache = self.cache.lock().ok()?;
        cache.get(&key).map(|e| (e.data.clone(), e.width, e.height))
    }

    pub fn insert(&self, key: u64, data: Vec<u8>, width: u32, height: u32) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(
                key,
                AssetEntry {
                    data,
                    width,
                    height,
                },
            );
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
