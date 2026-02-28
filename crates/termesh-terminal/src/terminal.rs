//! Terminal emulation wrapper around alacritty_terminal.

use crate::grid::{build_renderable_cell, CursorState, GridSnapshot};
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;

/// No-op event listener for the terminal emulator.
///
/// Terminal events (title changes, clipboard, bell, etc.) are
/// collected and can be polled by the caller.
#[derive(Clone)]
struct TermEventListener {
    events: std::sync::Arc<std::sync::Mutex<Vec<TermEvent>>>,
}

impl TermEventListener {
    fn new() -> Self {
        Self {
            events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl EventListener for TermEventListener {
    fn send_event(&self, event: Event) {
        let term_event = match event {
            Event::Title(title) => Some(TermEvent::TitleChanged(title)),
            Event::Bell => Some(TermEvent::Bell),
            Event::Exit => Some(TermEvent::Exit),
            Event::PtyWrite(text) => Some(TermEvent::PtyWrite(text)),
            _ => None,
        };
        if let Some(e) = term_event {
            if let Ok(mut events) = self.events.lock() {
                events.push(e);
            }
        }
    }
}

/// Events emitted by the terminal emulator.
#[derive(Debug, Clone)]
pub enum TermEvent {
    /// The terminal title has changed.
    TitleChanged(String),
    /// The terminal bell was triggered.
    Bell,
    /// The terminal process requested exit.
    Exit,
    /// The terminal wants to write data back to the PTY.
    PtyWrite(String),
}

/// Terminal size dimensions.
struct TermSize {
    cols: usize,
    rows: usize,
}

impl TermSize {
    fn new(cols: usize, rows: usize) -> Self {
        Self { cols, rows }
    }
}

impl Dimensions for TermSize {
    fn columns(&self) -> usize {
        self.cols
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn total_lines(&self) -> usize {
        self.rows
    }
}

/// Terminal emulator wrapping alacritty_terminal.
///
/// Processes raw PTY output bytes and produces a renderable grid.
pub struct Terminal {
    term: Term<TermEventListener>,
    parser: vte::ansi::Processor,
    listener: TermEventListener,
    rows: usize,
    cols: usize,
}

impl Terminal {
    /// Create a new terminal emulator with the given dimensions.
    ///
    /// # Arguments
    /// - `rows`: Number of rows (default: 24).
    /// - `cols`: Number of columns (default: 80).
    /// - `scrollback`: Number of scrollback lines (default: 10000).
    pub fn new(rows: usize, cols: usize, scrollback: usize) -> Self {
        let size = TermSize::new(cols, rows);
        let config = TermConfig {
            scrolling_history: scrollback,
            ..TermConfig::default()
        };

        let listener = TermEventListener::new();
        let term = Term::new(config, &size, listener.clone());
        let parser = vte::ansi::Processor::new();

        Self {
            term,
            parser,
            listener,
            rows,
            cols,
        }
    }

    /// Feed raw PTY output bytes into the terminal emulator.
    ///
    /// This processes VT100/ANSI escape sequences and updates
    /// the internal terminal state (grid, cursor, colors, etc.).
    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    /// Take a snapshot of the current grid for rendering.
    ///
    /// Returns a `GridSnapshot` with all renderable cells and cursor state.
    pub fn render_grid(&self) -> GridSnapshot {
        let grid = self.term.grid();
        let cols = grid.columns();
        let rows = grid.screen_lines();

        let mut cells = Vec::with_capacity(rows * cols);

        for row_idx in 0..rows {
            for col_idx in 0..cols {
                let point = Point::new(Line(row_idx as i32), Column(col_idx));
                let cell = &grid[point];

                let renderable =
                    build_renderable_cell(row_idx, col_idx, cell.c, &cell.fg, &cell.bg, cell.flags);
                cells.push(renderable);
            }
        }

        let cursor_point = self.term.grid().cursor.point;
        let cursor = CursorState {
            row: cursor_point.line.0 as usize,
            col: cursor_point.column.0,
            visible: true,
        };

        GridSnapshot {
            cells,
            rows,
            cols,
            cursor,
        }
    }

    /// Resize the terminal.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.rows = rows;
        self.cols = cols;
        let size = TermSize::new(cols, rows);
        self.term.resize(size);
    }

    /// Drain any pending terminal events.
    pub fn drain_events(&self) -> Vec<TermEvent> {
        if let Ok(mut events) = self.listener.events.lock() {
            events.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Get the current number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Get the current number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Scroll the viewport up by the given number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        use alacritty_terminal::grid::Scroll;
        self.term.scroll_display(Scroll::Delta(lines as i32));
    }

    /// Scroll the viewport down by the given number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        use alacritty_terminal::grid::Scroll;
        self.term.scroll_display(Scroll::Delta(-(lines as i32)));
    }

    /// Reset the viewport to the bottom (latest output).
    pub fn scroll_to_bottom(&mut self) {
        use alacritty_terminal::grid::Scroll;
        self.term.scroll_display(Scroll::Bottom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_terminal() {
        let term = Terminal::new(24, 80, 10000);
        assert_eq!(term.rows(), 24);
        assert_eq!(term.cols(), 80);
    }

    #[test]
    fn test_feed_plain_text() {
        let mut term = Terminal::new(24, 80, 10000);
        term.feed_bytes(b"Hello World");

        let grid = term.render_grid();
        assert_eq!(grid.rows, 24);
        assert_eq!(grid.cols, 80);

        // Check that "Hello World" appears in the first row
        let text: String = (0..11).map(|col| grid.cell_at(0, col).unwrap().c).collect();
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_feed_with_newline() {
        let mut term = Terminal::new(24, 80, 10000);
        term.feed_bytes(b"Line1\r\nLine2");

        let grid = term.render_grid();

        let line1: String = (0..5).map(|col| grid.cell_at(0, col).unwrap().c).collect();
        let line2: String = (0..5).map(|col| grid.cell_at(1, col).unwrap().c).collect();
        assert_eq!(line1, "Line1");
        assert_eq!(line2, "Line2");
    }

    #[test]
    fn test_feed_ansi_bold() {
        let mut term = Terminal::new(24, 80, 10000);
        // ESC[1m = bold on, ESC[0m = reset
        term.feed_bytes(b"\x1b[1mBold\x1b[0m Normal");

        let grid = term.render_grid();
        let bold_cell = grid.cell_at(0, 0).unwrap();
        assert_eq!(bold_cell.c, 'B');
        assert!(bold_cell.bold);

        let normal_cell = grid.cell_at(0, 5).unwrap();
        assert_eq!(normal_cell.c, 'N');
        assert!(!normal_cell.bold);
    }

    #[test]
    fn test_feed_ansi_color() {
        let mut term = Terminal::new(24, 80, 10000);
        // ESC[31m = red foreground
        term.feed_bytes(b"\x1b[31mRed\x1b[0m");

        let grid = term.render_grid();
        let cell = grid.cell_at(0, 0).unwrap();
        assert_eq!(cell.c, 'R');
        // Should not be default foreground
        assert_ne!(cell.fg, crate::color::DEFAULT_FG);
    }

    #[test]
    fn test_feed_true_color() {
        let mut term = Terminal::new(24, 80, 10000);
        // ESC[38;2;100;200;50m = set fg to RGB(100, 200, 50)
        term.feed_bytes(b"\x1b[38;2;100;200;50mX\x1b[0m");

        let grid = term.render_grid();
        let cell = grid.cell_at(0, 0).unwrap();
        assert_eq!(cell.c, 'X');
        assert_eq!(cell.fg, crate::color::Rgba::rgb(100, 200, 50));
    }

    #[test]
    fn test_resize() {
        let mut term = Terminal::new(24, 80, 10000);
        term.resize(40, 120);
        assert_eq!(term.rows(), 40);
        assert_eq!(term.cols(), 120);

        let grid = term.render_grid();
        assert_eq!(grid.rows, 40);
        assert_eq!(grid.cols, 120);
    }

    #[test]
    fn test_cursor_position() {
        let mut term = Terminal::new(24, 80, 10000);
        term.feed_bytes(b"AB");

        let grid = term.render_grid();
        // Cursor should be after 'B' at column 2
        assert_eq!(grid.cursor.col, 2);
        assert_eq!(grid.cursor.row, 0);
    }

    #[test]
    fn test_drain_events_empty() {
        let term = Terminal::new(24, 80, 10000);
        let events = term.drain_events();
        assert!(events.is_empty());
    }

    #[test]
    fn test_title_change_event() {
        let mut term = Terminal::new(24, 80, 10000);
        // OSC 0 ; title ST = set window title
        term.feed_bytes(b"\x1b]0;My Title\x07");

        let events = term.drain_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, TermEvent::TitleChanged(t) if t == "My Title")));
    }

    #[test]
    fn test_bell_event() {
        let mut term = Terminal::new(24, 80, 10000);
        term.feed_bytes(b"\x07"); // BEL character

        let events = term.drain_events();
        assert!(events.iter().any(|e| matches!(e, TermEvent::Bell)));
    }

    #[test]
    fn test_grid_snapshot_full_size() {
        let term = Terminal::new(24, 80, 10000);
        let grid = term.render_grid();
        assert_eq!(grid.cells.len(), 24 * 80);
    }
}
