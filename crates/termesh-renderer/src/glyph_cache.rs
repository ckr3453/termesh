//! Glyph cache with texture atlas for GPU rendering.

use crate::font::LoadedFont;
use std::collections::HashMap;

/// Maximum texture atlas size (2048x2048).
const ATLAS_SIZE: u32 = 2048;

/// Minimum cell height (pixels) for MSDF rendering.
/// Below this threshold, bitmap rasterization produces sharper results
/// because the MSDF px_range becomes too small for smooth edges.
const MSDF_MIN_CELL_HEIGHT: f32 = 32.0;

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
    /// Whether this glyph uses MSDF rendering (vs bitmap).
    pub is_msdf: bool,
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
    ///
    /// Tries MSDF for primary-font glyphs; falls back to bitmap for
    /// fallback fonts and glyphs without outlines.
    pub fn get_or_insert(&mut self, c: char, font: &LoadedFont) -> Option<GlyphInfo> {
        if let Some(&info) = self.glyphs.get(&c) {
            return Some(info);
        }

        // Try MSDF for primary font glyphs (only at large enough sizes)
        if font.metrics.cell_height >= MSDF_MIN_CELL_HEIGHT {
            if let Some(msdf) = font.rasterize_msdf(c) {
                return self.insert_msdf(c, &msdf);
            }
        }

        // Fallback: bitmap rasterization
        self.insert_bitmap(c, font)
    }

    /// Insert a bitmap-rasterized glyph into the atlas.
    fn insert_bitmap(&mut self, c: char, font: &LoadedFont) -> Option<GlyphInfo> {
        let (metrics, bitmap) = font.rasterize(c);

        if metrics.width == 0 || metrics.height == 0 {
            let info = GlyphInfo {
                atlas_x: 0,
                atlas_y: 0,
                width: 0,
                height: 0,
                bearing_x: metrics.xmin as f32,
                bearing_y: metrics.ymin as f32,
                is_msdf: false,
            };
            self.glyphs.insert(c, info);
            return Some(info);
        }

        let w = metrics.width as u32;
        let h = metrics.height as u32;

        self.advance_shelf_if_needed(w)?;

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
            is_msdf: false,
        };

        self.glyphs.insert(c, info);
        self.cursor_x += w + 1;
        self.shelf_height = self.shelf_height.max(h);
        self.dirty = true;

        Some(info)
    }

    /// Insert an MSDF glyph into the atlas.
    fn insert_msdf(&mut self, c: char, msdf: &crate::font::MsdfGlyph) -> Option<GlyphInfo> {
        let w = msdf.width;
        let h = msdf.height;

        self.advance_shelf_if_needed(w)?;

        if self.cursor_y + h > self.atlas_height {
            log::warn!("glyph atlas full (MSDF), cannot cache '{c}'");
            return None;
        }

        // Copy RGB MSDF data into RGBA atlas (A = 255)
        for row in 0..h {
            for col in 0..w {
                let src_idx = ((row * w + col) * 3) as usize;
                let dst_x = self.cursor_x + col;
                let dst_y = self.cursor_y + row;
                let dst_idx = ((dst_y * self.atlas_width + dst_x) * 4) as usize;

                self.atlas_data[dst_idx] = msdf.pixels[src_idx]; // R = SDF ch1
                self.atlas_data[dst_idx + 1] = msdf.pixels[src_idx + 1]; // G = SDF ch2
                self.atlas_data[dst_idx + 2] = msdf.pixels[src_idx + 2]; // B = SDF ch3
                self.atlas_data[dst_idx + 3] = 255; // A = opaque marker
            }
        }

        let info = GlyphInfo {
            atlas_x: self.cursor_x,
            atlas_y: self.cursor_y,
            width: w,
            height: h,
            bearing_x: 0.0,
            bearing_y: 0.0,
            is_msdf: true,
        };

        self.glyphs.insert(c, info);
        self.cursor_x += w + 1;
        self.shelf_height = self.shelf_height.max(h);
        self.dirty = true;

        Some(info)
    }

    /// Move to the next shelf row if the current glyph doesn't fit horizontally.
    /// Returns `None` if the atlas is too full for even a new row.
    fn advance_shelf_if_needed(&mut self, glyph_width: u32) -> Option<()> {
        if self.cursor_x + glyph_width > self.atlas_width {
            self.cursor_x = 0;
            self.cursor_y += self.shelf_height + 1;
            self.shelf_height = 0;
        }
        Some(())
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

    #[test]
    fn test_primary_font_glyphs_use_msdf_at_large_size() {
        // MSDF is only used when cell_height >= 32 (large font / HiDPI)
        let font = load_builtin_font(28.0).unwrap();
        assert!(
            font.metrics.cell_height >= 32.0,
            "28pt should have cell_height >= 32, got {}",
            font.metrics.cell_height
        );
        let mut cache = GlyphCache::new();

        let info = cache.get_or_insert('A', &font).unwrap();
        assert!(info.is_msdf, "large size primary font should use MSDF");
        assert_eq!(info.width, font.msdf_cell_size());
        assert_eq!(info.height, font.msdf_cell_size());
    }

    #[test]
    fn test_primary_font_uses_bitmap_at_small_size() {
        // At 14pt DPI=1.0, cell_height < 32 → bitmap fallback
        let font = load_builtin_font(14.0).unwrap();
        let mut cache = GlyphCache::new();

        let info = cache.get_or_insert('A', &font).unwrap();
        assert!(!info.is_msdf, "small size should use bitmap, not MSDF");
    }

    #[test]
    fn test_fallback_glyphs_use_bitmap() {
        let font = load_builtin_font(14.0).unwrap();
        let has_korean_fallback = font.fallbacks.iter().any(|f| f.has_glyph('가'));
        if !has_korean_fallback {
            eprintln!("skipping: no Korean fallback font");
            return;
        }

        let mut cache = GlyphCache::new();
        let info = cache.get_or_insert('가', &font).unwrap();
        assert!(!info.is_msdf, "fallback font should use bitmap");
    }

    #[test]
    fn test_msdf_atlas_data_has_rgb_channels() {
        let font = load_builtin_font(28.0).unwrap();
        let mut cache = GlyphCache::new();

        let info = cache.get_or_insert('M', &font).unwrap();
        assert!(info.is_msdf);

        // MSDF stores distance channels in R,G,B and sets A=255
        let px_idx = ((info.atlas_y * cache.atlas_width + info.atlas_x) * 4) as usize;
        assert_eq!(
            cache.atlas_data()[px_idx + 3],
            255,
            "MSDF alpha should be 255"
        );

        // SDF channels should have some variation (not all zeros)
        let mut has_nonzero = false;
        for row in 0..info.height {
            for col in 0..info.width {
                let idx =
                    (((info.atlas_y + row) * cache.atlas_width + info.atlas_x + col) * 4) as usize;
                if cache.atlas_data()[idx] != 0 || cache.atlas_data()[idx + 1] != 0 {
                    has_nonzero = true;
                    break;
                }
            }
        }
        assert!(has_nonzero, "MSDF should have non-zero distance values");
    }
}
