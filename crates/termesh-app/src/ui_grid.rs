//! Converts UI elements (session list, side panel) into renderable GridSnapshots.

use termesh_layout::session_list::SessionList;
use termesh_terminal::color::Rgba;
use termesh_terminal::grid::{CursorState, GridSnapshot, RenderableCell};

/// Background color for the session list panel.
const PANEL_BG: Rgba = Rgba::rgb(0x18, 0x18, 0x18);

/// Background color for the selected (active) session entry.
const SELECTED_BG: Rgba = Rgba::rgb(0x30, 0x50, 0x70);

/// Foreground color for labels.
const LABEL_FG: Rgba = Rgba::rgb(0xd0, 0xd0, 0xd0);

/// Foreground color for dimmed text (agent state icons for non-agent sessions).
const DIM_FG: Rgba = Rgba::rgb(0x60, 0x60, 0x60);

/// Render a session list into a GridSnapshot.
///
/// `rows` and `cols` are the grid dimensions computed from the panel's pixel
/// rect and font cell size.
pub fn render_session_list(list: &SessionList, rows: usize, cols: usize) -> GridSnapshot {
    let cols = cols.max(1);
    let rows = rows.max(1);

    let mut cells = Vec::with_capacity(rows * cols);

    for row in 0..rows {
        let entry = list.entries().get(row);
        let is_selected = entry.is_some() && row == list.selected_index();
        let bg = if is_selected { SELECTED_BG } else { PANEL_BG };

        if let Some(entry) = entry {
            // Format: "{icon} {label}"
            let icon = format!("{}", entry.state);
            let line = format!("{icon} {}", entry.label);
            let fg = if entry.is_agent { LABEL_FG } else { DIM_FG };

            for col_idx in 0..cols {
                let c = line.chars().nth(col_idx).unwrap_or(' ');
                // Use brighter fg for the icon portion (first few chars)
                let cell_fg = if col_idx < icon.chars().count() {
                    LABEL_FG
                } else {
                    fg
                };
                cells.push(RenderableCell {
                    row,
                    col: col_idx,
                    c,
                    fg: cell_fg,
                    bg,
                    ..Default::default()
                });
            }
        } else {
            // Empty row
            for col_idx in 0..cols {
                cells.push(RenderableCell {
                    row,
                    col: col_idx,
                    c: ' ',
                    fg: LABEL_FG,
                    bg,
                    ..Default::default()
                });
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use termesh_core::types::{AgentState, SessionId};
    use termesh_layout::session_list::SessionEntry;

    fn make_list() -> SessionList {
        let mut list = SessionList::new();
        list.add(SessionEntry {
            id: SessionId(1),
            label: "Backend".to_string(),
            is_agent: true,
            state: AgentState::Thinking,
        });
        list.add(SessionEntry {
            id: SessionId(2),
            label: "Shell".to_string(),
            is_agent: false,
            state: AgentState::None,
        });
        list
    }

    #[test]
    fn test_render_basic() {
        let list = make_list();
        let grid = render_session_list(&list, 10, 20);

        assert_eq!(grid.rows, 10);
        assert_eq!(grid.cols, 20);
        assert_eq!(grid.cells.len(), 10 * 20);
        assert!(!grid.cursor.visible);
    }

    #[test]
    fn test_first_row_has_content() {
        let list = make_list();
        let grid = render_session_list(&list, 10, 30);

        // First row should contain the first entry's label
        let first_row: String = grid.cells[..30].iter().map(|c| c.c).collect();
        let trimmed = first_row.trim_end();
        assert!(trimmed.contains("Backend"), "got: '{trimmed}'");
    }

    #[test]
    fn test_selected_row_highlighted() {
        let list = make_list();
        let grid = render_session_list(&list, 10, 20);

        // Row 0 is selected (default)
        let row0_bg = grid.cells[0].bg;
        assert_eq!(row0_bg, SELECTED_BG);

        // Row 1 is not selected
        let row1_bg = grid.cells[20].bg;
        assert_eq!(row1_bg, PANEL_BG);
    }

    #[test]
    fn test_empty_list() {
        let list = SessionList::new();
        let grid = render_session_list(&list, 5, 10);

        assert_eq!(grid.rows, 5);
        assert_eq!(grid.cols, 10);
        // All cells should be spaces with panel bg
        for cell in &grid.cells {
            assert_eq!(cell.c, ' ');
            assert_eq!(cell.bg, PANEL_BG);
        }
    }

    #[test]
    fn test_agent_vs_shell_colors() {
        let list = make_list();
        let grid = render_session_list(&list, 10, 30);

        // Find the label portion of each row (after icon)
        // Row 0 (agent): label chars should have LABEL_FG
        // Row 1 (shell): label chars should have DIM_FG
        let icon_len_0 = format!("{}", AgentState::Thinking).chars().count();
        let label_start_0 = icon_len_0 + 1; // after space
        if label_start_0 < 30 {
            assert_eq!(grid.cells[label_start_0].fg, LABEL_FG);
        }

        let icon_len_1 = format!("{}", AgentState::None).chars().count();
        let label_start_1 = 30 + icon_len_1 + 1;
        if label_start_1 < 60 {
            assert_eq!(grid.cells[label_start_1].fg, DIM_FG);
        }
    }

    #[test]
    fn test_narrow_cols() {
        let list = make_list();
        let grid = render_session_list(&list, 5, 3);

        assert_eq!(grid.cols, 3);
        // Should not panic, content is truncated
        assert_eq!(grid.cells.len(), 5 * 3);
    }
}
