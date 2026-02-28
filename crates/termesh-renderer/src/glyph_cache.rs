//! Glyph cache with texture atlas for GPU rendering.

use crate::font::LoadedFont;
use std::collections::HashMap;

/// Maximum texture atlas size (2048x2048).
const ATLAS_SIZE: u32 = 2048;

/// Information about a cached glyph in the texture atlas.
#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    /// X offset in atlas texture (pixels).
    pub atlas_x: u32,
    /// Y offset in atlas texture (pixels).
    pub atlas_y: u32,
    /// Glyph bitmap width.
    pub width: u32,
    /// Glyph bitmap height.
    pub height: u32,
    /// X offset from cell origin to glyph origin.
    pub bearing_x: f32,
    /// Y offset from baseline to glyph top.
    pub bearing_y: f32,
}

/// Texture atlas for glyph caching.
///
/// Packs glyph bitmaps into a single texture using a simple
/// shelf-packing algorithm.
pub struct GlyphCache {
    /// Raw RGBA pixel data for the atlas.
    atlas_data: Vec<u8>,
    /// Atlas width in pixels.
    pub atlas_width: u32,
    /// Atlas height in pixels.
    pub atlas_height: u32,
    /// Cached glyph info indexed by character.
    glyphs: HashMap<char, GlyphInfo>,
    /// Current packing cursor X.
    cursor_x: u32,
    /// Current packing cursor Y.
    cursor_y: u32,
    /// Current shelf height (max glyph height in current row).
    shelf_height: u32,
    /// Whether the atlas data has changed since last upload.
    pub dirty: bool,
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphCache {
    /// Create a new empty glyph cache.
    pub fn new() -> Self {
        let atlas_width = ATLAS_SIZE;
        let atlas_height = ATLAS_SIZE;
        Self {
            atlas_data: vec![0u8; (atlas_width * atlas_height * 4) as usize],
            atlas_width,
            atlas_height,
            glyphs: HashMap::new(),
            cursor_x: 0,
            cursor_y: 0,
            shelf_height: 0,
            dirty: false,
        }
    }

    /// Get cached glyph info, or rasterize and cache it.
    pub fn get_or_insert(&mut self, c: char, font: &LoadedFont) -> Option<GlyphInfo> {
        if let Some(&info) = self.glyphs.get(&c) {
            return Some(info);
        }

        let (metrics, bitmap) = font.rasterize(c);

        if metrics.width == 0 || metrics.height == 0 {
            // Space or zero-size glyph — store with zero dimensions
            let info = GlyphInfo {
                atlas_x: 0,
                atlas_y: 0,
                width: 0,
                height: 0,
                bearing_x: metrics.xmin as f32,
                bearing_y: metrics.ymin as f32,
            };
            self.glyphs.insert(c, info);
            return Some(info);
        }

        let w = metrics.width as u32;
        let h = metrics.height as u32;

        // Check if we need to move to next shelf
        if self.cursor_x + w > self.atlas_width {
            self.cursor_x = 0;
            self.cursor_y += self.shelf_height + 1; // +1 padding
            self.shelf_height = 0;
        }

        // Check if atlas is full
        if self.cursor_y + h > self.atlas_height {
            log::warn!("glyph atlas full, cannot cache '{c}'");
            return None;
        }

        // Copy bitmap into atlas (convert grayscale to RGBA)
        for row in 0..h {
            for col in 0..w {
                let src_idx = (row * w + col) as usize;
                let dst_x = self.cursor_x + col;
                let dst_y = self.cursor_y + row;
                let dst_idx = ((dst_y * self.atlas_width + dst_x) * 4) as usize;

                let alpha = bitmap[src_idx];
                self.atlas_data[dst_idx] = 255; // R
                self.atlas_data[dst_idx + 1] = 255; // G
                self.atlas_data[dst_idx + 2] = 255; // B
                self.atlas_data[dst_idx + 3] = alpha; // A
            }
        }

        let info = GlyphInfo {
            atlas_x: self.cursor_x,
            atlas_y: self.cursor_y,
            width: w,
            height: h,
            bearing_x: metrics.xmin as f32,
            bearing_y: metrics.ymin as f32,
        };

        self.glyphs.insert(c, info);
        self.cursor_x += w + 1; // +1 padding
        self.shelf_height = self.shelf_height.max(h);
        self.dirty = true;

        Some(info)
    }

    /// Get the raw RGBA atlas data for GPU upload.
    pub fn atlas_data(&self) -> &[u8] {
        &self.atlas_data
    }

    /// Mark the atlas as uploaded (no longer dirty).
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Number of cached glyphs.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::load_builtin_font;

    #[test]
    fn test_cache_single_glyph() {
        let font = load_builtin_font(14.0).unwrap();
        let mut cache = GlyphCache::new();

        let info = cache.get_or_insert('A', &font);
        assert!(info.is_some());

        let info = info.unwrap();
        assert!(info.width > 0);
        assert!(info.height > 0);
        assert_eq!(cache.glyph_count(), 1);
        assert!(cache.dirty);
    }

    #[test]
    fn test_cache_hit() {
        let font = load_builtin_font(14.0).unwrap();
        let mut cache = GlyphCache::new();

        let info1 = cache.get_or_insert('B', &font).unwrap();
        let info2 = cache.get_or_insert('B', &font).unwrap();

        // Same atlas position on cache hit
        assert_eq!(info1.atlas_x, info2.atlas_x);
        assert_eq!(info1.atlas_y, info2.atlas_y);
        assert_eq!(cache.glyph_count(), 1);
    }

    #[test]
    fn test_cache_multiple_glyphs() {
        let font = load_builtin_font(14.0).unwrap();
        let mut cache = GlyphCache::new();

        for c in 'A'..='Z' {
            assert!(cache.get_or_insert(c, &font).is_some());
        }
        assert_eq!(cache.glyph_count(), 26);
    }

    #[test]
    fn test_cache_space() {
        let font = load_builtin_font(14.0).unwrap();
        let mut cache = GlyphCache::new();

        let info = cache.get_or_insert(' ', &font);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.width, 0);
        assert_eq!(info.height, 0);
    }

    #[test]
    fn test_atlas_data_size() {
        let cache = GlyphCache::new();
        assert_eq!(
            cache.atlas_data().len(),
            (ATLAS_SIZE * ATLAS_SIZE * 4) as usize
        );
    }

    #[test]
    fn test_mark_clean() {
        let font = load_builtin_font(14.0).unwrap();
        let mut cache = GlyphCache::new();
        cache.get_or_insert('X', &font);
        assert!(cache.dirty);
        cache.mark_clean();
        assert!(!cache.dirty);
    }
}
