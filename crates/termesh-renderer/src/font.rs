//! Font loading and glyph rasterization using fontdue.
//!
//! Supports a primary monospace font with system fallback fonts
//! for CJK (Korean, Chinese, Japanese) and emoji characters.

use fontdue::{Font, FontSettings};
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

/// Loaded font with fallback chain for rasterization.
pub struct LoadedFont {
    pub font: Font,
    pub metrics: FontMetrics,
    /// Fallback fonts for characters not in the primary font.
    fallbacks: Vec<Font>,
}

impl LoadedFont {
    /// Load a font from raw bytes.
    pub fn from_bytes(data: &[u8], font_size: f32) -> Result<Self, RenderError> {
        let font = Font::from_bytes(data, FontSettings::default()).map_err(|e| {
            RenderError::FontLoadFailed {
                path: format!("<embedded>: {e}").into(),
            }
        })?;

        let metrics = compute_metrics(&font, font_size);
        let fallbacks = load_system_fallbacks();

        Ok(Self {
            font,
            metrics,
            fallbacks,
        })
    }

    /// Rasterize a single character, trying fallback fonts if needed.
    pub fn rasterize(&self, c: char) -> (fontdue::Metrics, Vec<u8>) {
        // Try primary font first
        let (metrics, bitmap) = self.font.rasterize(c, self.metrics.font_size);
        if !is_notdef(&metrics, &bitmap, c) {
            return (metrics, bitmap);
        }

        // Try fallback fonts
        for fallback in &self.fallbacks {
            let (fb_metrics, fb_bitmap) = fallback.rasterize(c, self.metrics.font_size);
            if !is_notdef(&fb_metrics, &fb_bitmap, c) {
                return (fb_metrics, fb_bitmap);
            }
        }

        // No font has this glyph — return primary font's .notdef
        (metrics, bitmap)
    }
}

/// Check if a rasterized glyph is likely the .notdef (missing) glyph.
///
/// Heuristic: if the glyph has zero dimensions or its bitmap is entirely
/// empty (all zeros), it's probably .notdef.
fn is_notdef(metrics: &fontdue::Metrics, bitmap: &[u8], c: char) -> bool {
    // Space and control chars are expected to have zero dimensions
    if c.is_ascii_control() || c == ' ' {
        return false;
    }
    if metrics.width == 0 || metrics.height == 0 {
        return true;
    }
    // Check if bitmap is all zeros (blank glyph = likely .notdef)
    bitmap.iter().all(|&b| b == 0)
}

/// Compute monospace cell metrics from a font.
fn compute_metrics(font: &Font, font_size: f32) -> FontMetrics {
    // Use 'M' as reference for monospace width
    let (m_metrics, _) = font.rasterize('M', font_size);

    // Use line metrics for height/baseline
    let line_metrics = font
        .horizontal_line_metrics(font_size)
        .unwrap_or(fontdue::LineMetrics {
            ascent: font_size * 0.8,
            descent: font_size * -0.2,
            line_gap: 0.0,
            new_line_size: font_size,
        });

    let cell_width = m_metrics.advance_width.max(font_size * 0.6);
    let cell_height =
        (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).max(font_size * 1.2);
    let baseline = line_metrics.ascent;

    FontMetrics {
        cell_width,
        cell_height,
        baseline,
        font_size,
    }
}

/// Embedded fallback monospace font.
static BUILTIN_FONT: &[u8] = include_bytes!("../fonts/CascadiaMono.ttf");

/// Load the built-in fallback font.
pub fn load_builtin_font(font_size: f32) -> Result<LoadedFont, RenderError> {
    LoadedFont::from_bytes(BUILTIN_FONT, font_size)
}

/// Known system font paths for CJK/emoji fallback.
fn system_font_paths() -> Vec<&'static str> {
    let mut paths = Vec::new();

    if cfg!(target_os = "windows") {
        // Windows system fonts
        paths.extend_from_slice(&[
            "C:/Windows/Fonts/malgun.ttf",   // Malgun Gothic (한글)
            "C:/Windows/Fonts/msgothic.ttc", // MS Gothic (日本語)
            "C:/Windows/Fonts/msyh.ttc",     // Microsoft YaHei (中文)
            "C:/Windows/Fonts/seguiemj.ttf", // Segoe UI Emoji
        ]);
    } else if cfg!(target_os = "macos") {
        paths.extend_from_slice(&[
            "/System/Library/Fonts/AppleSDGothicNeo.ttc", // Korean
            "/System/Library/Fonts/Supplemental/AppleGothic.ttf", // Korean fallback
            "/System/Library/Fonts/Apple Color Emoji.ttc", // Emoji
            "/System/Library/Fonts/Hiragino Sans GB.ttc", // CJK
        ]);
    } else {
        // Linux
        paths.extend_from_slice(&[
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
        ]);
    }

    paths
}

/// Load system fallback fonts for CJK and emoji.
fn load_system_fallbacks() -> Vec<Font> {
    let mut fonts = Vec::new();

    for path in system_font_paths() {
        if let Ok(data) = std::fs::read(path) {
            match Font::from_bytes(data, FontSettings::default()) {
                Ok(font) => {
                    log::info!("loaded fallback font: {path}");
                    fonts.push(font);
                }
                Err(e) => {
                    log::debug!("failed to load fallback font {path}: {e}");
                }
            }
        }
    }

    if fonts.is_empty() {
        log::warn!("no system fallback fonts found for CJK/emoji");
    }

    fonts
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
    fn test_rasterize_char() {
        let font = load_builtin_font(14.0).unwrap();
        let (metrics, bitmap) = font.rasterize('A');
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);
        assert!(!bitmap.is_empty());
    }

    #[test]
    fn test_rasterize_space() {
        let font = load_builtin_font(14.0).unwrap();
        let (metrics, _bitmap) = font.rasterize(' ');
        // Space has advance width but may have zero height
        assert!(metrics.advance_width > 0.0);
    }

    #[test]
    fn test_metrics_consistency() {
        let font = load_builtin_font(14.0).unwrap();
        // Monospace: all printable chars should have same advance width
        let (m_a, _) = font.rasterize('A');
        let (m_w, _) = font.rasterize('W');
        let (m_i, _) = font.rasterize('i');
        // Allow small floating point differences
        assert!((m_a.advance_width - m_w.advance_width).abs() < 1.0);
        assert!((m_a.advance_width - m_i.advance_width).abs() < 1.0);
    }

    #[test]
    fn test_is_notdef() {
        assert!(!is_notdef(
            &fontdue::Metrics {
                width: 8,
                height: 10,
                ..Default::default()
            },
            &[1; 80],
            'A'
        ));
        // Zero-size glyph for non-space
        assert!(is_notdef(
            &fontdue::Metrics {
                width: 0,
                height: 0,
                ..Default::default()
            },
            &[],
            'A'
        ));
        // Space is not considered notdef even with zero dimensions
        assert!(!is_notdef(
            &fontdue::Metrics {
                width: 0,
                height: 0,
                ..Default::default()
            },
            &[],
            ' '
        ));
    }

    #[test]
    fn test_fallback_loads_on_system() {
        let font = load_builtin_font(14.0).unwrap();
        // On any dev machine, at least some fallback fonts should exist
        // This test is informational — it won't fail on CI
        eprintln!("fallback fonts loaded: {}", font.fallbacks.len());
    }

    #[test]
    fn test_korean_rendered_via_fallback() {
        let font = load_builtin_font(14.0).unwrap();
        if font.fallbacks.is_empty() {
            eprintln!("skipping: no fallback fonts on this system");
            return;
        }
        let (m, bmp) = font.rasterize('가');
        eprintln!("Korean '가' via fallback: w={}, h={}", m.width, m.height);
        // Should have non-zero bitmap (not .notdef)
        assert!(
            bmp.iter().any(|&b| b > 0),
            "Korean glyph should render via fallback"
        );
    }
}
