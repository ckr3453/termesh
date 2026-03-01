//! Renderable cell grid extracted from alacritty_terminal.

use crate::color::{resolve_color, Rgba, DEFAULT_BG, DEFAULT_FG};
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::Color as AnsiColor;

/// A single renderable cell for the GPU renderer.
#[derive(Debug, Clone, Copy)]
pub struct RenderableCell {
    /// Row position (0-based from top).
    pub row: usize,
    /// Column position (0-based from left).
    pub col: usize,
    /// The character to display.
    pub c: char,
    /// Foreground color.
    pub fg: Rgba,
    /// Background color.
    pub bg: Rgba,
    /// Whether the cell is bold.
    pub bold: bool,
    /// Whether the cell is italic.
    pub italic: bool,
    /// Whether the cell is underlined.
    pub underline: bool,
    /// Whether the cell is strikethrough.
    pub strikethrough: bool,
    /// Whether the cell has inverse colors.
    pub inverse: bool,
    /// Whether this is a wide character.
    pub wide: bool,
}

impl Default for RenderableCell {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            c: ' ',
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            inverse: false,
            wide: false,
        }
    }
}

/// Cursor position and style for rendering.
#[derive(Debug, Clone, Copy)]
pub struct CursorState {
    pub row: usize,
    pub col: usize,
    pub visible: bool,
}

/// A range of selected text (row, col) coordinates.
#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionRange {
    /// Start row (inclusive).
    pub start_row: usize,
    /// Start column (inclusive).
    pub start_col: usize,
    /// End row (inclusive).
    pub end_row: usize,
    /// End column (inclusive).
    pub end_col: usize,
}

/// A snapshot of the terminal grid ready for rendering.
#[derive(Debug, Clone)]
pub struct GridSnapshot {
    /// All renderable cells in row-major order.
    pub cells: Vec<RenderableCell>,
    /// Number of rows in the grid.
    pub rows: usize,
    /// Number of columns in the grid.
    pub cols: usize,
    /// Cursor state.
    pub cursor: CursorState,
    /// Currently selected range (if any).
    pub selection: Option<SelectionRange>,
}

impl GridSnapshot {
    /// Get a cell at the given position.
    pub fn cell_at(&self, row: usize, col: usize) -> Option<&RenderableCell> {
        if row < self.rows && col < self.cols {
            Some(&self.cells[row * self.cols + col])
        } else {
            None
        }
    }
}

/// Build a `RenderableCell` from alacritty cell data.
pub(crate) fn build_renderable_cell(
    row: usize,
    col: usize,
    c: char,
    fg: &AnsiColor,
    bg: &AnsiColor,
    flags: CellFlags,
) -> RenderableCell {
    let mut fg_color = resolve_color(fg);
    let mut bg_color = resolve_color(bg);

    let inverse = flags.contains(CellFlags::INVERSE);
    if inverse {
        std::mem::swap(&mut fg_color, &mut bg_color);
    }

    // Bold brightens foreground for named colors
    if flags.contains(CellFlags::BOLD) {
        if let AnsiColor::Named(named) = fg {
            // Convert to bright variant if applicable
            fg_color = resolve_color(&AnsiColor::Named(to_bright(*named)));
        }
    }

    RenderableCell {
        row,
        col,
        c,
        fg: fg_color,
        bg: bg_color,
        bold: flags.contains(CellFlags::BOLD),
        italic: flags.contains(CellFlags::ITALIC),
        underline: flags.intersects(CellFlags::ALL_UNDERLINES),
        strikethrough: flags.contains(CellFlags::STRIKEOUT),
        inverse,
        wide: flags.contains(CellFlags::WIDE_CHAR),
    }
}

/// Convert a named color to its bright variant.
fn to_bright(
    color: alacritty_terminal::vte::ansi::NamedColor,
) -> alacritty_terminal::vte::ansi::NamedColor {
    use alacritty_terminal::vte::ansi::NamedColor::*;
    match color {
        Black => BrightBlack,
        Red => BrightRed,
        Green => BrightGreen,
        Yellow => BrightYellow,
        Blue => BrightBlue,
        Magenta => BrightMagenta,
        Cyan => BrightCyan,
        White => BrightWhite,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};

    #[test]
    fn test_default_renderable_cell() {
        let cell = RenderableCell::default();
        assert_eq!(cell.c, ' ');
        assert_eq!(cell.fg, DEFAULT_FG);
        assert_eq!(cell.bg, DEFAULT_BG);
        assert!(!cell.bold);
    }

    #[test]
    fn test_build_basic_cell() {
        let cell = build_renderable_cell(
            0,
            0,
            'A',
            &AnsiColor::Named(NamedColor::Foreground),
            &AnsiColor::Named(NamedColor::Background),
            CellFlags::empty(),
        );
        assert_eq!(cell.c, 'A');
        assert_eq!(cell.fg, DEFAULT_FG);
        assert_eq!(cell.bg, DEFAULT_BG);
    }

    #[test]
    fn test_build_bold_cell() {
        let cell = build_renderable_cell(
            0,
            0,
            'B',
            &AnsiColor::Named(NamedColor::Red),
            &AnsiColor::Named(NamedColor::Background),
            CellFlags::BOLD,
        );
        assert!(cell.bold);
        // Bold red should use BrightRed color
        let bright_red = resolve_color(&AnsiColor::Named(NamedColor::BrightRed));
        assert_eq!(cell.fg, bright_red);
    }

    #[test]
    fn test_build_inverse_cell() {
        let cell = build_renderable_cell(
            0,
            0,
            'I',
            &AnsiColor::Named(NamedColor::Foreground),
            &AnsiColor::Named(NamedColor::Background),
            CellFlags::INVERSE,
        );
        assert!(cell.inverse);
        // Colors should be swapped
        assert_eq!(cell.fg, DEFAULT_BG);
        assert_eq!(cell.bg, DEFAULT_FG);
    }

    #[test]
    fn test_grid_snapshot_cell_at() {
        let cells = vec![
            RenderableCell {
                row: 0,
                col: 0,
                c: 'A',
                ..Default::default()
            },
            RenderableCell {
                row: 0,
                col: 1,
                c: 'B',
                ..Default::default()
            },
            RenderableCell {
                row: 1,
                col: 0,
                c: 'C',
                ..Default::default()
            },
            RenderableCell {
                row: 1,
                col: 1,
                c: 'D',
                ..Default::default()
            },
        ];
        let snapshot = GridSnapshot {
            cells,
            rows: 2,
            cols: 2,
            cursor: CursorState {
                row: 0,
                col: 0,
                visible: true,
            },
            selection: None,
        };
        assert_eq!(snapshot.cell_at(0, 0).unwrap().c, 'A');
        assert_eq!(snapshot.cell_at(1, 1).unwrap().c, 'D');
        assert!(snapshot.cell_at(2, 0).is_none());
    }
}
