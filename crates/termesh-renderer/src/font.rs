//! Font loading and glyph rasterization using fontdue.

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

/// Loaded font ready for rasterization.
pub struct LoadedFont {
    pub font: Font,
    pub metrics: FontMetrics,
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
        Ok(Self { font, metrics })
    }

    /// Rasterize a single character. Returns (metrics, bitmap).
    pub fn rasterize(&self, c: char) -> (fontdue::Metrics, Vec<u8>) {
        self.font.rasterize(c, self.metrics.font_size)
    }
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

/// Embedded fallback monospace font (JetBrains Mono or similar).
/// We include a minimal built-in font so the renderer works out of the box.
static BUILTIN_FONT: &[u8] = include_bytes!("../fonts/CascadiaMono.ttf");

/// Load the built-in fallback font.
pub fn load_builtin_font(font_size: f32) -> Result<LoadedFont, RenderError> {
    LoadedFont::from_bytes(BUILTIN_FONT, font_size)
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
}
