//! Terminal emulation wrapper around alacritty_terminal.

use crate::grid::{build_renderable_cell, CursorState, GridSnapshot, SelectionRange};
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags as CellFlags;
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
    /// Selection anchor point (where drag started).
    selection_anchor: Option<(usize, usize)>,
    /// Selection endpoint (current drag position).
    selection_end: Option<(usize, usize)>,
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
            selection_anchor: None,
            selection_end: None,
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
    /// When the viewport is scrolled (display_offset > 0), cells are read
    /// from the scrollback history region using negative line indices.
    pub fn render_grid(&self) -> GridSnapshot {
        let grid = self.term.grid();
        let cols = grid.columns();
        let rows = grid.screen_lines();
        let display_offset = grid.display_offset();

        let mut cells = Vec::with_capacity(rows * cols);

        for row_idx in 0..rows {
            // Apply display_offset: shift line index into scrollback history.
            // display_offset=0 → Line(0..rows) (current screen)
            // display_offset=N → Line(-N .. rows-N) (scrolled up N lines)
            let line = Line(row_idx as i32 - display_offset as i32);
            for col_idx in 0..cols {
                let point = Point::new(line, Column(col_idx));
                let cell = &grid[point];

                let is_spacer = cell.flags.contains(CellFlags::WIDE_CHAR_SPACER);
                let mut renderable =
                    build_renderable_cell(row_idx, col_idx, cell.c, &cell.fg, &cell.bg, cell.flags);
                renderable.spacer = is_spacer;
                cells.push(renderable);
            }
        }

        let cursor_point = self.term.grid().cursor.point;
        // Adjust cursor position for display offset. Hide cursor when scrolled.
        let cursor = if display_offset == 0 {
            CursorState {
                row: cursor_point.line.0 as usize,
                col: cursor_point.column.0,
                visible: true,
            }
        } else {
            CursorState {
                row: 0,
                col: 0,
                visible: false,
            }
        };

        let selection = self.selection_range();

        GridSnapshot {
            cells,
            rows,
            cols,
            cursor,
            selection,
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

    /// Start a text selection at the given grid coordinate.
    pub fn selection_start(&mut self, row: usize, col: usize) {
        self.selection_anchor = Some((row, col));
        self.selection_end = Some((row, col));
    }

    /// Update the selection endpoint as the mouse drags.
    pub fn selection_update(&mut self, row: usize, col: usize) {
        if self.selection_anchor.is_some() {
            self.selection_end = Some((row, col));
        }
    }

    /// Clear the current selection.
    pub fn selection_clear(&mut self) {
        self.selection_anchor = None;
        self.selection_end = None;
    }

    /// Check if there is an active selection.
    pub fn has_selection(&self) -> bool {
        self.selection_anchor.is_some() && self.selection_end.is_some()
    }

    /// Get the normalized selection range (start <= end).
    fn selection_range(&self) -> Option<SelectionRange> {
        let (ar, ac) = self.selection_anchor?;
        let (er, ec) = self.selection_end?;

        // Normalize so start <= end
        let (start_row, start_col, end_row, end_col) = if (ar, ac) <= (er, ec) {
            (ar, ac, er, ec)
        } else {
            (er, ec, ar, ac)
        };

        Some(SelectionRange {
            start_row,
            start_col,
            end_row,
            end_col,
        })
    }

    /// Extract the selected text from the grid.
    pub fn selected_text(&self) -> Option<String> {
        let range = self.selection_range()?;
        let grid = self.term.grid();
        let cols = grid.columns();
        let rows = grid.screen_lines();

        let mut text = String::new();

        for row_idx in range.start_row..=range.end_row.min(rows.saturating_sub(1)) {
            let col_start = if row_idx == range.start_row {
                range.start_col
            } else {
                0
            };
            let col_end = if row_idx == range.end_row {
                range.end_col.min(cols.saturating_sub(1))
            } else {
                cols.saturating_sub(1)
            };

            let mut line = String::new();
            for col_idx in col_start..=col_end {
                let point = Point::new(Line(row_idx as i32), Column(col_idx));
                let cell = &grid[point];
                if cell.c != '\0' {
                    line.push(cell.c);
                }
            }

            // Trim trailing spaces from each line
            let trimmed = line.trim_end();
            text.push_str(trimmed);

            if row_idx < range.end_row {
                text.push('\n');
            }
        }

        if text.is_empty() {
            None
        } else {
            Some(text)
        }
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

    #[test]
    fn test_selection_start_and_update() {
        let mut term = Terminal::new(24, 80, 10000);
        assert!(!term.has_selection());

        term.selection_start(0, 5);
        assert!(term.has_selection());

        term.selection_update(0, 10);
        let grid = term.render_grid();
        let sel = grid.selection.unwrap();
        assert_eq!(sel.start_row, 0);
        assert_eq!(sel.start_col, 5);
        assert_eq!(sel.end_row, 0);
        assert_eq!(sel.end_col, 10);
    }

    #[test]
    fn test_selection_backward_normalized() {
        let mut term = Terminal::new(24, 80, 10000);
        // Drag from (2,10) to (0,5) — backward selection
        term.selection_start(2, 10);
        term.selection_update(0, 5);

        let grid = term.render_grid();
        let sel = grid.selection.unwrap();
        // Should be normalized: start <= end
        assert_eq!(sel.start_row, 0);
        assert_eq!(sel.start_col, 5);
        assert_eq!(sel.end_row, 2);
        assert_eq!(sel.end_col, 10);
    }

    #[test]
    fn test_selection_clear() {
        let mut term = Terminal::new(24, 80, 10000);
        term.selection_start(0, 0);
        term.selection_update(0, 5);
        assert!(term.has_selection());

        term.selection_clear();
        assert!(!term.has_selection());
        assert!(term.render_grid().selection.is_none());
    }

    #[test]
    fn test_selected_text_single_line() {
        let mut term = Terminal::new(24, 80, 10000);
        term.feed_bytes(b"Hello World");

        term.selection_start(0, 0);
        term.selection_update(0, 4);
        assert_eq!(term.selected_text(), Some("Hello".to_string()));
    }

    #[test]
    fn test_selected_text_multi_line() {
        let mut term = Terminal::new(24, 80, 10000);
        term.feed_bytes(b"Line1\r\nLine2\r\nLine3");

        term.selection_start(0, 0);
        term.selection_update(1, 4);
        assert_eq!(term.selected_text(), Some("Line1\nLine2".to_string()));
    }

    #[test]
    fn test_selected_text_no_selection() {
        let term = Terminal::new(24, 80, 10000);
        assert_eq!(term.selected_text(), None);
    }

    #[test]
    fn test_selection_in_grid_snapshot() {
        let mut term = Terminal::new(24, 80, 10000);
        let grid = term.render_grid();
        assert!(grid.selection.is_none());

        term.selection_start(1, 3);
        term.selection_update(2, 7);
        let grid = term.render_grid();
        assert!(grid.selection.is_some());
    }

    #[test]
    fn test_scroll_up_shows_history() {
        // Use a small terminal (4 rows) so scrollback is triggered quickly.
        let mut term = Terminal::new(4, 20, 100);
        // Write enough lines to push content into scrollback.
        for i in 0..10 {
            term.feed_bytes(format!("line-{i}\r\n").as_bytes());
        }

        // Before scrolling: bottom of output should be visible.
        let grid = term.render_grid();
        assert!(grid.cursor.visible);

        // Scroll up to see earlier lines.
        term.scroll_up(6);

        let grid = term.render_grid();
        // Cursor should be hidden when scrolled.
        assert!(!grid.cursor.visible);

        // First visible row should contain earlier content (not the latest).
        let first_row: String = (0..6).map(|col| grid.cell_at(0, col).unwrap().c).collect();
        assert!(
            first_row.starts_with("line-"),
            "expected scrollback content, got: {first_row:?}"
        );
    }

    #[test]
    fn test_scroll_to_bottom_restores_view() {
        let mut term = Terminal::new(4, 20, 100);
        for i in 0..10 {
            term.feed_bytes(format!("line-{i}\r\n").as_bytes());
        }

        term.scroll_up(5);
        assert!(!term.render_grid().cursor.visible);

        term.scroll_to_bottom();
        assert!(term.render_grid().cursor.visible);
    }
}
