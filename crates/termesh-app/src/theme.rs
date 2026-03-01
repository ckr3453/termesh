//! Centralized color palette for the Termesh UI.
//!
//! All UI modules import colors from here to ensure visual consistency.

pub use termesh_terminal::color::Rgba;

// ── Surface backgrounds ──────────────────────────────────────────────────

/// Panel background (dark, blue-tinted).
pub const BG_SURFACE: Rgba = Rgba::rgb(0x18, 0x18, 0x1C);

/// Header / status bar background (slightly elevated).
pub const BG_ELEVATED: Rgba = Rgba::rgb(0x1E, 0x1E, 0x24);

/// Selected item background (subtle blue highlight).
pub const BG_SELECTED: Rgba = Rgba::rgb(0x24, 0x2C, 0x38);

// ── Foreground text ──────────────────────────────────────────────────────

/// Primary text color.
pub const FG_PRIMARY: Rgba = Rgba::rgb(0xCC, 0xCC, 0xD0);

/// Secondary / dimmed text color.
pub const FG_SECONDARY: Rgba = Rgba::rgb(0x80, 0x82, 0x8A);

/// Muted text / inactive borders.
pub const FG_MUTED: Rgba = Rgba::rgb(0x50, 0x52, 0x58);

// ── Accent & status ──────────────────────────────────────────────────────

/// Single accent color (focus, active state).
pub const ACCENT: Rgba = Rgba::rgb(0x5C, 0x9F, 0xE0);

/// Waiting-for-input status color.
pub const STATUS_WAITING: Rgba = Rgba::rgb(0xD4, 0xA5, 0x5A);

/// Success status color.
pub const STATUS_SUCCESS: Rgba = Rgba::rgb(0x6B, 0xB8, 0x7A);

/// Error status color.
pub const STATUS_ERROR: Rgba = Rgba::rgb(0xCF, 0x6B, 0x73);

// ── Diff ─────────────────────────────────────────────────────────────────

/// Diff insertion foreground.
pub const DIFF_ADD: Rgba = Rgba::rgb(0x5A, 0xB8, 0x6A);

/// Diff deletion foreground.
pub const DIFF_DEL: Rgba = Rgba::rgb(0xC5, 0x5A, 0x62);

// ── Borders ──────────────────────────────────────────────────────────────
// BORDER_COLOR is defined inline in main.rs dividers() as [f32; 4]
// for direct use with the GPU renderer.
