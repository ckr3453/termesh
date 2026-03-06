//! Font loading and metrics using cosmic-text.
//!
//! Provides a `LoadedFont` that wraps `cosmic_text::FontSystem` for system
//! font discovery and fallback. Text rendering is handled by glyphon.

use glyphon::cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics as CosmicMetrics, Shaping,
    SwashCache as CosmicSwashCache,
};
use std::cell::RefCell;
use termesh_core::error::RenderError;

/// Monospace font metrics.
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Width of a single cell in pixels.
    pub cell_width: f32,
    /// Height of a single cell in pixels.
    pub cell_height: f32,
    /// Baseline offset from top of cell.
    pub baseline: f32,
    /// Font size in pixels.
    pub font_size: f32,
}

/// Loaded font with cosmic-text FontSystem for text layout.
///
/// Uses `RefCell` for interior mutability since cosmic-text APIs
/// require `&mut` but callers pass `&LoadedFont`.
pub struct LoadedFont {
    font_system: RefCell<FontSystem>,
    swash_cache: RefCell<CosmicSwashCache>,
    /// Computed monospace cell metrics.
    pub metrics: FontMetrics,
}

/// Embedded fallback monospace font.
static BUILTIN_FONT: &[u8] = include_bytes!("../fonts/CascadiaMono.ttf");

impl LoadedFont {
    /// Create a new font system with the built-in font and system fallbacks.
    ///
    /// CJK fallback fonts (Apple SD Gothic Neo, PingFang SC, Hiragino Sans)
    /// are automatically resolved by cosmic-text's `PlatformFallback`.
    fn new(font_size: f32) -> Result<Self, RenderError> {
        let mut font_system = FontSystem::new();
        font_system
            .db_mut()
            .load_font_data(BUILTIN_FONT.to_vec());
        // Override default monospace family ("Noto Sans Mono") with our built-in font,
        // which may not exist on all platforms.
        font_system
            .db_mut()
            .set_monospace_family("Cascadia Mono");

        // Register platform emoji fallback font for color emoji rendering.
        register_emoji_fallback(&mut font_system);

        let metrics = compute_metrics(&mut font_system, font_size);

        Ok(Self {
            font_system: RefCell::new(font_system),
            swash_cache: RefCell::new(CosmicSwashCache::new()),
            metrics,
        })
    }

    /// Borrow the inner FontSystem mutably (for glyphon prepare).
    pub fn font_system_mut(&self) -> std::cell::RefMut<'_, FontSystem> {
        self.font_system.borrow_mut()
    }

    /// Borrow the inner SwashCache mutably (for glyphon prepare).
    pub fn swash_cache_mut(&self) -> std::cell::RefMut<'_, CosmicSwashCache> {
        self.swash_cache.borrow_mut()
    }
}

/// Compute monospace cell metrics from the font.
///
/// Uses a two-pass approach: first measures the font to get real metrics,
/// then derives cell dimensions from ascent/descent/leading instead of
/// hardcoded multipliers.
fn compute_metrics(font_system: &mut FontSystem, font_size: f32) -> FontMetrics {
    // First pass: use an approximate line_height to identify the font and measure glyph width.
    let approx_line_height = font_size * 1.4;
    let cosmic_metrics = CosmicMetrics::new(font_size, approx_line_height);

    let mut buffer = Buffer::new(font_system, cosmic_metrics);
    buffer.set_size(
        font_system,
        Some(font_size * 10.0),
        Some(approx_line_height * 2.0),
    );
    let attrs = Attrs::new().family(Family::Monospace);
    // Measure advance widths from multiple representative glyphs.
    buffer.set_text(
        font_system,
        "M@W0",
        &attrs,
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(font_system, false);

    let mut cell_width = font_size * 0.6;
    let mut font_info = None;

    for run in buffer.layout_runs() {
        let mut width_sum = 0.0_f32;
        let mut count = 0u32;
        for glyph in run.glyphs.iter() {
            width_sum += glyph.w;
            count += 1;
            if font_info.is_none() {
                font_info = Some((glyph.font_id, glyph.font_weight));
            }
        }
        if count > 0 {
            cell_width = width_sum / count as f32;
        }
        break;
    }

    // Derive cell_height and baseline from actual font metrics (ascent/descent/leading).
    let (cell_height, baseline) = if let Some((id, weight)) = font_info {
        if let Some(font) = font_system.get_font(id, weight) {
            let m = font.metrics();
            let upem = m.units_per_em as f32;
            let ascent = m.ascent * font_size / upem;
            let descent = m.descent * font_size / upem;
            let leading = m.leading * font_size / upem;
            let height = (ascent - descent + leading).max(font_size * 1.2);
            (height, ascent)
        } else {
            (font_size * 1.4, font_size * 0.8)
        }
    } else {
        (font_size * 1.4, font_size * 0.8)
    };

    FontMetrics {
        cell_width,
        cell_height,
        baseline,
        font_size,
    }
}

/// Register a platform-specific color emoji fallback font.
///
/// On macOS, loads Apple Color Emoji so that emoji glyphs render
/// correctly when the primary monospace font lacks them.
fn register_emoji_fallback(font_system: &mut FontSystem) {
    #[cfg(target_os = "macos")]
    {
        let path = std::path::Path::new("/System/Library/Fonts/Apple Color Emoji.ttc");
        if path.exists() {
            if let Ok(data) = std::fs::read(path) {
                font_system.db_mut().load_font_data(data);
                log::info!("loaded Apple Color Emoji fallback font");
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // On Linux, Noto Color Emoji is typically discovered via system font paths.
        let _ = font_system;
    }
}

/// Load the built-in font with system fallbacks.
pub fn load_builtin_font(font_size: f32) -> Result<LoadedFont, RenderError> {
    LoadedFont::new(font_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_builtin_font() {
        let font = load_builtin_font(14.0).unwrap();
        assert!(font.metrics.cell_width > 0.0);
        assert!(font.metrics.cell_height > 0.0);
        assert!(font.metrics.baseline > 0.0);
    }

    #[test]
    fn test_metrics_reasonable() {
        let font = load_builtin_font(14.0).unwrap();
        let m = font.metrics;
        assert!(m.cell_width > 5.0 && m.cell_width < 20.0);
        assert!(m.cell_height > 14.0 && m.cell_height < 25.0);
        assert!(m.baseline > 0.0 && m.baseline < m.cell_height);
    }

    #[test]
    fn test_font_system_borrow() {
        let font = load_builtin_font(14.0).unwrap();
        let _fs = font.font_system_mut();
    }
}
