//! Font loading and glyph rasterization using fontdue.
//!
//! Supports a primary monospace font with system fallback fonts
//! for CJK (Korean, Chinese, Japanese) and emoji characters.
//!
//! Primary font glyphs use MSDF (Multi-channel Signed Distance Field)
//! for resolution-independent rendering. Fallback fonts use bitmap rasterization.

use fdsm::generate::generate_msdf;
use fdsm::render::correct_sign_msdf;
use fdsm::shape::Shape;
use fdsm::transform::Transform;
use fontdue::{Font, FontSettings};
use image::RgbImage;
use nalgebra::{Affine2, Matrix3};
use termesh_core::error::RenderError;

/// MSDF atlas cell height in pixels.
const MSDF_CELL_SIZE: u32 = 48;

/// SDF range in MSDF pixels (controls the soft edge width).
const MSDF_RANGE: f64 = 4.0;

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

/// MSDF rasterization result for a single glyph.
pub struct MsdfGlyph {
    /// RGB distance field pixel data (3 bytes per pixel, row-major).
    pub pixels: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Loaded font with fallback chain for rasterization.
pub struct LoadedFont {
    pub font: Font,
    pub metrics: FontMetrics,
    /// Fallback fonts for characters not in the primary font.
    pub(crate) fallbacks: Vec<Font>,
    /// Raw font data for ttf-parser (needed for MSDF generation).
    font_data: Vec<u8>,
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
            font_data: data.to_vec(),
        })
    }

    /// Rasterize a single character, trying fallback fonts if needed.
    pub fn rasterize(&self, c: char) -> (fontdue::Metrics, Vec<u8>) {
        // Try primary font first (use has_glyph for accurate detection)
        if self.font.has_glyph(c) {
            return self.font.rasterize(c, self.metrics.font_size);
        }

        // Try fallback fonts
        for fallback in &self.fallbacks {
            if fallback.has_glyph(c) {
                return fallback.rasterize(c, self.metrics.font_size);
            }
        }

        // No font has this glyph — return primary font's .notdef
        self.font.rasterize(c, self.metrics.font_size)
    }

    /// Generate MSDF for a primary-font glyph.
    ///
    /// Returns `None` if the character is not in the primary font
    /// or has no outline (e.g., space).
    pub fn rasterize_msdf(&self, c: char) -> Option<MsdfGlyph> {
        if !self.font.has_glyph(c) {
            return None;
        }

        let face = ttf_parser::Face::parse(&self.font_data, 0).ok()?;
        let glyph_id = face.glyph_index(c)?;
        let shape = fdsm_ttf_parser::load_shape_from_face(&face, glyph_id)?;

        let upem = face.units_per_em() as f64;
        let ascent = face.ascender() as f64;
        let size = MSDF_CELL_SIZE as f64;
        let range = MSDF_RANGE;

        // Scale from font units to MSDF pixel space.
        // Maps the full EM box to the content area (size - 2*range).
        let scale = (size - 2.0 * range) / upem;

        // Affine transform: scale + flip Y + translate.
        // Font coords: y-up, baseline at y=0
        // Pixel coords: y-down, origin at top-left
        let transform = Affine2::from_matrix_unchecked(Matrix3::new(
            scale,
            0.0,
            range,
            0.0,
            -scale,
            range + ascent * scale,
            0.0,
            0.0,
            1.0,
        ));

        let mut shape = shape;
        shape.transform(&transform);

        let colored = Shape::edge_coloring_simple(shape, 3.0, 0);
        let prepared = colored.prepare();

        let mut img = RgbImage::new(MSDF_CELL_SIZE, MSDF_CELL_SIZE);
        generate_msdf(&prepared, range, &mut img);
        correct_sign_msdf(
            &mut img,
            &prepared,
            fdsm::bezier::scanline::FillRule::Nonzero,
        );

        // Convert to raw RGB bytes
        let pixels = img.into_raw();

        Some(MsdfGlyph {
            pixels,
            width: MSDF_CELL_SIZE,
            height: MSDF_CELL_SIZE,
        })
    }

    /// MSDF cell size in pixels.
    pub fn msdf_cell_size(&self) -> u32 {
        MSDF_CELL_SIZE
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
    fn test_fallback_loads_on_system() {
        let font = load_builtin_font(14.0).unwrap();
        // On any dev machine, at least some fallback fonts should exist
        // This test is informational — it won't fail on CI
        eprintln!("fallback fonts loaded: {}", font.fallbacks.len());
    }

    #[test]
    fn test_baseline_glyph_positions() {
        // Test at multiple DPI-scaled sizes
        for font_size in [14.0_f32, 21.0, 28.0] {
            let font = load_builtin_font(font_size).unwrap();
            let m = font.metrics;
            eprintln!("\n=== font_size={:.0} ===", font_size);
            eprintln!(
                "cell: {:.1}x{:.1}, baseline={:.1}",
                m.cell_width, m.cell_height, m.baseline
            );

            for c in ['A', 'a', 'g', 'p', 'y', '|', '_', '.'] {
                let (gm, _) = font.rasterize(c);
                let gh = gm.height as f32;
                let glyph_top = gm.ymin as f32 + gh;
                let baseline_y = (m.baseline - glyph_top).round();
                let center_y = ((m.cell_height - gh) / 2.0).round();
                eprintln!(
                    "  '{}': w={:2} h={:2} xmin={:3} ymin={:3} → baseline_y={:.0} center_y={:.0}",
                    c, gm.width, gm.height, gm.xmin, gm.ymin, baseline_y, center_y
                );
                // Baseline position must be within cell bounds
                assert!(
                    baseline_y >= -1.0,
                    "glyph '{}' at {:.0}pt: baseline_y={} is above cell",
                    c,
                    font_size,
                    baseline_y
                );
                assert!(
                    baseline_y + gh <= m.cell_height + 1.0,
                    "glyph '{}' at {:.0}pt: bottom={} exceeds cell_height={}",
                    c,
                    font_size,
                    baseline_y + gh,
                    m.cell_height
                );
            }

            // Korean from fallback
            if !font.fallbacks.is_empty() {
                let (gm, _) = font.rasterize('가');
                eprintln!(
                    "  '가': w={:2} h={:2} xmin={:3} ymin={:3} has_glyph={}",
                    gm.width,
                    gm.height,
                    gm.xmin,
                    gm.ymin,
                    font.font.has_glyph('가')
                );
            }
        }
    }

    #[test]
    fn test_msdf_generation_ascii() {
        let font = load_builtin_font(14.0).unwrap();
        let msdf = font.rasterize_msdf('A');
        assert!(msdf.is_some(), "MSDF should succeed for ASCII 'A'");

        let msdf = msdf.unwrap();
        assert_eq!(msdf.width, MSDF_CELL_SIZE);
        assert_eq!(msdf.height, MSDF_CELL_SIZE);
        assert_eq!(
            msdf.pixels.len(),
            (MSDF_CELL_SIZE * MSDF_CELL_SIZE * 3) as usize
        );

        // SDF should have some variation (not all zeros or all max)
        let has_low = msdf.pixels.iter().any(|&b| b < 64);
        let has_high = msdf.pixels.iter().any(|&b| b > 192);
        assert!(
            has_low && has_high,
            "MSDF should contain distance field variation"
        );
    }

    #[test]
    fn test_msdf_generation_multiple_chars() {
        let font = load_builtin_font(14.0).unwrap();
        for c in ['a', 'z', 'M', '0', '9', '@', '#'] {
            let msdf = font.rasterize_msdf(c);
            assert!(msdf.is_some(), "MSDF should succeed for '{c}'");
        }
    }

    #[test]
    fn test_msdf_none_for_space() {
        let font = load_builtin_font(14.0).unwrap();
        // Space has no outline — should return None
        let msdf = font.rasterize_msdf(' ');
        assert!(msdf.is_none(), "Space glyph has no outline for MSDF");
    }

    #[test]
    fn test_msdf_none_for_fallback_char() {
        let font = load_builtin_font(14.0).unwrap();
        // CJK character is not in primary font
        let msdf = font.rasterize_msdf('가');
        assert!(msdf.is_none(), "Fallback chars should not use MSDF");
    }

    #[test]
    fn test_korean_rendered_via_fallback() {
        let font = load_builtin_font(14.0).unwrap();
        // Skip on systems without Korean-capable fallback fonts (e.g., Ubuntu CI)
        let has_korean_fallback = font.fallbacks.iter().any(|f| f.has_glyph('가'));
        if !has_korean_fallback {
            eprintln!("skipping: no Korean fallback font on this system");
            return;
        }
        let (m, bmp) = font.rasterize('가');
        // Primary font (CascadiaMono) lacks Korean glyphs, so has_glyph()
        // should route to fallback (Malgun Gothic) which produces larger glyphs.
        assert!(
            m.width > 10,
            "Korean glyph should come from fallback font (width > 10), got {}",
            m.width
        );
        assert!(
            bmp.iter().any(|&b| b > 0),
            "Korean glyph should render via fallback"
        );
    }
}
