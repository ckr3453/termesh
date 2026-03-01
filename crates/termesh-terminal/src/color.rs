//! Color types and conversion utilities.

use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Rgb as AnsiRgb};

/// RGBA color representation for the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Convert to [f32; 4] for GPU shaders.
    pub fn to_f32_array(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
}

/// Default dark theme colors for named ANSI colors.
const NAMED_COLORS: [(u8, u8, u8); 18] = [
    // Normal colors
    (0x1e, 0x1e, 0x1e), // Black (background)
    (0xf4, 0x43, 0x36), // Red
    (0x4c, 0xaf, 0x50), // Green
    (0xff, 0xeb, 0x3b), // Yellow
    (0x21, 0x96, 0xf3), // Blue
    (0x9c, 0x27, 0xb0), // Magenta
    (0x00, 0xbc, 0xd4), // Cyan
    (0xe0, 0xe0, 0xe0), // White (foreground)
    // Bright colors
    (0x54, 0x54, 0x54), // BrightBlack
    (0xef, 0x53, 0x50), // BrightRed
    (0x66, 0xbb, 0x6a), // BrightGreen
    (0xff, 0xf1, 0x76), // BrightYellow
    (0x42, 0xa5, 0xf5), // BrightBlue
    (0xce, 0x93, 0xd8), // BrightMagenta
    (0x4d, 0xd0, 0xe1), // BrightCyan
    (0xff, 0xff, 0xff), // BrightWhite
    // Special
    (0xe0, 0xe0, 0xe0), // Foreground
    (0x1e, 0x1e, 0x1e), // Background
];

/// Resolve a named color index to Rgba.
fn named_color_to_rgba(color: NamedColor) -> Rgba {
    let idx = match color {
        NamedColor::Black => 0,
        NamedColor::Red => 1,
        NamedColor::Green => 2,
        NamedColor::Yellow => 3,
        NamedColor::Blue => 4,
        NamedColor::Magenta => 5,
        NamedColor::Cyan => 6,
        NamedColor::White => 7,
        NamedColor::BrightBlack => 8,
        NamedColor::BrightRed => 9,
        NamedColor::BrightGreen => 10,
        NamedColor::BrightYellow => 11,
        NamedColor::BrightBlue => 12,
        NamedColor::BrightMagenta => 13,
        NamedColor::BrightCyan => 14,
        NamedColor::BrightWhite => 15,
        NamedColor::Foreground => 16,
        NamedColor::Background => 17,
        _ => 16, // fallback to foreground
    };
    let (r, g, b) = NAMED_COLORS[idx];
    Rgba::rgb(r, g, b)
}

/// Convert a 256-color index to Rgba.
fn indexed_color_to_rgba(idx: u8) -> Rgba {
    match idx {
        0..=15 => {
            let (r, g, b) = NAMED_COLORS[idx as usize];
            Rgba::rgb(r, g, b)
        }
        // 216 color cube: indices 16-231
        16..=231 => {
            let n = idx - 16;
            let b = (n % 6) * 51;
            let g = ((n / 6) % 6) * 51;
            let r = (n / 36) * 51;
            Rgba::rgb(r, g, b)
        }
        // Grayscale ramp: indices 232-255
        232..=255 => {
            let v = 8 + (idx - 232) * 10;
            Rgba::rgb(v, v, v)
        }
    }
}

/// Resolve an alacritty AnsiColor to our Rgba type.
pub fn resolve_color(color: &AnsiColor) -> Rgba {
    match color {
        AnsiColor::Named(named) => named_color_to_rgba(*named),
        AnsiColor::Spec(AnsiRgb { r, g, b }) => Rgba::rgb(*r, *g, *b),
        AnsiColor::Indexed(idx) => indexed_color_to_rgba(*idx),
    }
}

/// Default foreground color.
pub const DEFAULT_FG: Rgba = Rgba::rgb(0xe0, 0xe0, 0xe0);

/// Default background color.
pub const DEFAULT_BG: Rgba = Rgba::rgb(0x1e, 0x1e, 0x1e);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba_to_f32() {
        let c = Rgba::rgb(255, 128, 0);
        let arr = c.to_f32_array();
        assert!((arr[0] - 1.0).abs() < 0.01);
        assert!((arr[1] - 0.502).abs() < 0.01);
        assert!((arr[2] - 0.0).abs() < 0.01);
        assert!((arr[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_named_color_resolution() {
        let fg = resolve_color(&AnsiColor::Named(NamedColor::Foreground));
        assert_eq!(fg, DEFAULT_FG);

        let bg = resolve_color(&AnsiColor::Named(NamedColor::Background));
        assert_eq!(bg, DEFAULT_BG);
    }

    #[test]
    fn test_spec_color_resolution() {
        let c = resolve_color(&AnsiColor::Spec(AnsiRgb {
            r: 100,
            g: 200,
            b: 50,
        }));
        assert_eq!(c, Rgba::rgb(100, 200, 50));
    }

    #[test]
    fn test_indexed_color_standard() {
        // Index 1 = Red
        let c = resolve_color(&AnsiColor::Indexed(1));
        assert_eq!(c, Rgba::rgb(0xf4, 0x43, 0x36));
    }

    #[test]
    fn test_indexed_color_grayscale() {
        // Index 232 = darkest gray
        let c = resolve_color(&AnsiColor::Indexed(232));
        assert_eq!(c, Rgba::rgb(8, 8, 8));
    }

    #[test]
    fn test_indexed_color_cube() {
        // Index 196 = bright red in 216-color cube
        let c = resolve_color(&AnsiColor::Indexed(196));
        // 196 - 16 = 180, r=180/36=5*51=255, g=0, b=0
        assert_eq!(c, Rgba::rgb(255, 0, 0));
    }
}
