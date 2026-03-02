//! Session picker overlay for swapping sessions in a split pane.
//!
//! Shown when the user presses Primary+S in Split mode.
//! Displays all sessions except the current pane's session, allowing
//! the user to select one to swap into the focused pane.

use crate::theme::*;
use crate::ui_grid;
use termesh_core::types::SessionId;
use termesh_terminal::grid::{CursorState, GridSnapshot};

/// A single entry in the session picker list.
#[derive(Debug, Clone)]
pub struct SessionPickerEntry {
    /// Session ID.
    pub id: SessionId,
    /// Display label (e.g., "a1b2c3d4").
    pub label: String,
    /// Agent kind (e.g., "claude", "shell").
    pub agent_kind: String,
    /// Which pane this session is bound to, if any (0-based display index).
    pub pane_index: Option<usize>,
}

/// Session picker overlay state.
pub struct SessionPicker {
    /// All sessions available for selection.
    entries: Vec<SessionPickerEntry>,
    /// Currently highlighted index.
    selected: usize,
    /// The session currently bound to the focused pane (for marking).
    current_session: Option<SessionId>,
}

impl SessionPicker {
    /// Create a new session picker.
    ///
    /// `entries` should contain all sessions.
    /// `current_session` is the session currently in the focused pane (shown as "current").
    pub fn new(entries: Vec<SessionPickerEntry>, current_session: Option<SessionId>) -> Self {
        // Start selection at the first non-current entry
        let selected = entries
            .iter()
            .position(|e| Some(e.id) != current_session)
            .unwrap_or(0);
        Self {
            entries,
            selected,
            current_session,
        }
    }

    /// Move selection up (wraps around).
    pub fn select_prev(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.entries.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Move selection down (wraps around).
    pub fn select_next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.entries.len();
    }

    /// Confirm selection and return the selected session ID.
    pub fn confirm(&self) -> Option<SessionId> {
        self.entries.get(self.selected).map(|e| e.id)
    }

    /// Whether there are any entries to pick from.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Render the picker into a `GridSnapshot`.
    pub fn render(&self, rows: usize, cols: usize) -> GridSnapshot {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let mut cells = Vec::with_capacity(rows * cols);

        let title = "Swap session";
        let separator = "\u{2500}".repeat(20.min(cols));
        let hint = "\u{2191}\u{2193} select, Enter swap, Esc cancel";

        // Content: title + separator + entries + blank + hint
        let content_height = 2 + self.entries.len() + 1 + 1;
        let start_row = if rows > content_height {
            (rows - content_height) / 2
        } else {
            0
        };

        for row in 0..rows {
            let rel = row.wrapping_sub(start_row);

            if rel == 0 {
                ui_grid::push_centered_row(&mut cells, row, cols, title, FG_PRIMARY, BG_SURFACE);
            } else if rel == 1 {
                ui_grid::push_centered_row(&mut cells, row, cols, &separator, FG_MUTED, BG_SURFACE);
            } else if rel >= 2 && rel < 2 + self.entries.len() {
                let idx = rel - 2;
                let entry = &self.entries[idx];
                let is_selected = idx == self.selected;
                let is_current = Some(entry.id) == self.current_session;

                let pane_info = match entry.pane_index {
                    Some(pi) => format!("(Pane {})", pi + 1),
                    None => "(unbound)".to_string(),
                };

                let marker = if is_selected { "\u{25B8}" } else { " " };
                let current_marker = if is_current { " \u{2190} current" } else { "" };

                let line = format!(
                    "  {marker} {idx}: {label}  {kind}  {pane}{cur}",
                    idx = idx + 1,
                    label = entry.label,
                    kind = entry.agent_kind,
                    pane = pane_info,
                    cur = current_marker,
                );

                let bg = if is_selected { BG_SELECTED } else { BG_SURFACE };
                let fg = if is_current { FG_MUTED } else { FG_PRIMARY };
                ui_grid::push_centered_row(&mut cells, row, cols, &line, fg, bg);
            } else if rel == 2 + self.entries.len() + 1 {
                ui_grid::push_centered_row(&mut cells, row, cols, hint, FG_MUTED, BG_SURFACE);
            } else {
                ui_grid::fill_row(&mut cells, row, cols, ' ', FG_MUTED, BG_SURFACE);
            }
        }

        GridSnapshot {
            cells,
            rows,
            cols,
            cursor: CursorState {
                row: 0,
                col: 0,
                visible: false,
            },
            selection: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entries() -> Vec<SessionPickerEntry> {
        vec![
            SessionPickerEntry {
                id: SessionId(1),
                label: "a1b2c3d4".to_string(),
                agent_kind: "Claude".to_string(),
                pane_index: Some(0),
            },
            SessionPickerEntry {
                id: SessionId(2),
                label: "e5f6g7h8".to_string(),
                agent_kind: "Shell".to_string(),
                pane_index: None,
            },
            SessionPickerEntry {
                id: SessionId(3),
                label: "i9j0k1l2".to_string(),
                agent_kind: "Claude".to_string(),
                pane_index: Some(1),
            },
        ]
    }

    #[test]
    fn test_new_skips_current_session() {
        let entries = make_entries();
        let picker = SessionPicker::new(entries, Some(SessionId(1)));
        // Should skip index 0 (current) and start at index 1
        assert_eq!(picker.selected, 1);
    }

    #[test]
    fn test_new_no_current_starts_at_zero() {
        let entries = make_entries();
        let picker = SessionPicker::new(entries, None);
        assert_eq!(picker.selected, 0);
    }

    #[test]
    fn test_select_next_wraps() {
        let entries = make_entries();
        let mut picker = SessionPicker::new(entries, None);
        picker.selected = 2;
        picker.select_next();
        assert_eq!(picker.selected, 0);
    }

    #[test]
    fn test_select_prev_wraps() {
        let entries = make_entries();
        let mut picker = SessionPicker::new(entries, None);
        picker.selected = 0;
        picker.select_prev();
        assert_eq!(picker.selected, 2);
    }

    #[test]
    fn test_confirm_returns_selected() {
        let entries = make_entries();
        let mut picker = SessionPicker::new(entries, None);
        picker.selected = 1;
        assert_eq!(picker.confirm(), Some(SessionId(2)));
    }

    #[test]
    fn test_confirm_empty() {
        let picker = SessionPicker::new(Vec::new(), None);
        assert_eq!(picker.confirm(), None);
    }

    #[test]
    fn test_render_dimensions() {
        let entries = make_entries();
        let picker = SessionPicker::new(entries, Some(SessionId(3)));
        let grid = picker.render(20, 60);
        assert_eq!(grid.rows, 20);
        assert_eq!(grid.cols, 60);
        assert_eq!(grid.cells.len(), 20 * 60);
        assert!(!grid.cursor.visible);
    }

    #[test]
    fn test_is_empty() {
        let picker = SessionPicker::new(Vec::new(), None);
        assert!(picker.is_empty());

        let entries = make_entries();
        let picker = SessionPicker::new(entries, None);
        assert!(!picker.is_empty());
    }
}
