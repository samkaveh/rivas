use std::{
    collections::{HashMap, hash_map},
    hash::{Hash, Hasher},
};

use crate::assets::math::render_math;

pub struct AssetCache {
    mermaid: HashMap<u64, (Vec<u8>, u32, u32)>,
    mermaid_errors: HashMap<u64, String>,
    images: HashMap<String, (Vec<u8>, u32, u32)>,
    image_errors: HashMap<String, String>,
    math: HashMap<u64, (Vec<u8>, u32, u32)>,
    math_errors: HashMap<u64, String>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            mermaid: HashMap::new(),
            mermaid_errors: HashMap::new(),
            images: HashMap::new(),
            image_errors: HashMap::new(),
            math: HashMap::new(),
            math_errors: HashMap::new(),
        }
    }

    pub fn get_or_load_image(
        &mut self,
        url: &str,
        base: Option<&std::path::Path>,
        max_width: u32,
    ) -> Result<&(Vec<u8>, u32, u32), &str> {
        let key = url.to_string();
        if self.image_errors.contains_key(&key) {
            return Err(self.image_errors.get(&key).unwrap());
        }
        if !self.images.contains_key(&key) {
            match super::images::load_image_to_png(url, base, max_width) {
                Ok(data) => {
                    self.images.insert(key.clone(), data);
                }
                Err(e) => {
                    let msg = format!("{e}");
                    self.image_errors.insert(key.clone(), msg);
                    return Err(self.image_errors.get(&key).unwrap());
                }
            }
        }
        Ok(self.images.get(&key).unwrap())
    }

    pub fn get_or_render_mermaid(
        &mut self,
        source: &str,
        max_width: u32,
    ) -> Result<&(Vec<u8>, u32, u32), &str> {
        let hash = Self::hash_str(source);
        if self.mermaid_errors.contains_key(&hash) {
            return Err(self.mermaid_errors.get(&hash).unwrap());
        }
        if !self.mermaid.contains_key(&hash) {
            match super::mermaid::render_mermaid_to_png(source, max_width) {
                Ok(data) => {
                    self.mermaid.insert(hash, data);
                }
                Err(e) => {
                    let msg = format!("{e}");
                    self.mermaid_errors.insert(hash, msg);
                    return Err(self.mermaid_errors.get(&hash).unwrap());
                }
            }
        }
        Ok(self.mermaid.get(&hash).unwrap())
    }

    pub fn get_or_render_math(
        &mut self,
        latex: &str,
        display: bool,
        max_width: u32,
        dark_theme: bool,
    ) -> Result<&(Vec<u8>, u32, u32), &str> {
        let hash = Self::hash_str(latex);
        if self.math_errors.contains_key(&hash) {
            return Err(self.math_errors.get(&hash).unwrap());
        }
        if !self.math.contains_key(&hash) {
            match render_math(latex, display, max_width, dark_theme) {
                Ok(data) => {
                    self.math.insert(hash, data);
                }
                Err(e) => {
                    let msg = format!("{e}");
                    self.math_errors.insert(hash, msg);
                    return Err(self.math_errors.get(&hash).unwrap());
                }
            }
        }
        Ok(self.math.get(&hash).unwrap())
    }

    pub fn clear_errors(&mut self) {
        self.mermaid_errors.clear();
        self.image_errors.clear();
        self.math_errors.clear();
    }

    fn hash_str(s: &str) -> u64 {
        let mut hasher = hash_map::DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
}
