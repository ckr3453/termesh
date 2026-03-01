//! Converts UI elements (session list, side panel) into renderable GridSnapshots.

use termesh_diff::diff_generator::{DiffLine, DiffTag};
use termesh_layout::session_list::SessionList;
use termesh_layout::side_panel::SidePanel;
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

/// Foreground color for diff insertions.
const DIFF_ADD_FG: Rgba = Rgba::rgb(0x50, 0xd0, 0x50);

/// Foreground color for diff deletions.
const DIFF_DEL_FG: Rgba = Rgba::rgb(0xd0, 0x50, 0x50);

/// Background color for the active tab header.
const TAB_ACTIVE_BG: Rgba = Rgba::rgb(0x28, 0x28, 0x38);

/// Foreground color for inactive tab headers.
const TAB_INACTIVE_FG: Rgba = Rgba::rgb(0x70, 0x70, 0x70);

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

/// Render the side panel (diff/preview/testlog) into a GridSnapshot.
///
/// `scroll_offset` is the number of content lines scrolled past.
pub fn render_side_panel(
    panel: &SidePanel,
    diff_lines: &[DiffLine],
    rows: usize,
    cols: usize,
    scroll_offset: usize,
) -> GridSnapshot {
    let cols = cols.max(1);
    let rows = rows.max(1);

    let mut cells = Vec::with_capacity(rows * cols);

    // Row 0: tab header bar
    let tabs = panel.tabs();
    let active_idx = panel.active_index();
    let mut header = String::new();
    for (i, tab) in tabs.iter().enumerate() {
        if i > 0 {
            header.push_str(" | ");
        }
        let name = match tab {
            termesh_core::types::SidePanelTab::Diff => "Diff",
            termesh_core::types::SidePanelTab::Preview => "Preview",
            termesh_core::types::SidePanelTab::TestLog => "TestLog",
        };
        header.push_str(name);
    }
    // Build tab position map: (start_col, end_col) for each tab
    let mut tab_ranges: Vec<(usize, usize)> = Vec::new();
    {
        let mut pos = 0;
        for (i, tab) in tabs.iter().enumerate() {
            if i > 0 {
                pos += 3; // " | "
            }
            let name = match tab {
                termesh_core::types::SidePanelTab::Diff => "Diff",
                termesh_core::types::SidePanelTab::Preview => "Preview",
                termesh_core::types::SidePanelTab::TestLog => "TestLog",
            };
            let start = pos;
            pos += name.len();
            tab_ranges.push((start, pos));
        }
    }

    // Render header row
    let header_chars: Vec<char> = header.chars().collect();
    for col_idx in 0..cols {
        let c = header_chars.get(col_idx).copied().unwrap_or(' ');
        // Determine if this column is inside the active tab's range
        let in_active = tab_ranges
            .get(active_idx)
            .is_some_and(|&(s, e)| col_idx >= s && col_idx < e);
        let (fg, bg) = if in_active {
            (LABEL_FG, TAB_ACTIVE_BG)
        } else {
            (TAB_INACTIVE_FG, PANEL_BG)
        };
        cells.push(RenderableCell {
            row: 0,
            col: col_idx,
            c,
            fg,
            bg,
            ..Default::default()
        });
    }

    // Row 1: separator line
    for col_idx in 0..cols {
        cells.push(RenderableCell {
            row: 1,
            col: col_idx,
            c: if col_idx < cols { '─' } else { ' ' },
            fg: DIM_FG,
            bg: PANEL_BG,
            ..Default::default()
        });
    }

    // Rows 2..rows: diff content (scrollable)
    let content_rows = rows.saturating_sub(2);
    for content_row in 0..content_rows {
        let row_idx = content_row + 2;
        let line_idx = scroll_offset + content_row;

        if let Some(diff_line) = diff_lines.get(line_idx) {
            let (prefix, fg) = match diff_line.tag {
                DiffTag::Insert => ('+', DIFF_ADD_FG),
                DiffTag::Delete => ('-', DIFF_DEL_FG),
                DiffTag::Equal => (' ', DIM_FG),
            };

            let line_text = format!("{prefix}{}", diff_line.content);
            let line_chars: Vec<char> = line_text.chars().collect();

            for col_idx in 0..cols {
                let c = line_chars.get(col_idx).copied().unwrap_or(' ');
                cells.push(RenderableCell {
                    row: row_idx,
                    col: col_idx,
                    c,
                    fg,
                    bg: PANEL_BG,
                    ..Default::default()
                });
            }
        } else {
            // Empty row past diff content
            for col_idx in 0..cols {
                cells.push(RenderableCell {
                    row: row_idx,
                    col: col_idx,
                    c: ' ',
                    fg: DIM_FG,
                    bg: PANEL_BG,
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
    use termesh_core::types::{AgentState, SessionId, SidePanelTab};
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

    // --- Side panel tests ---

    fn make_diff_lines() -> Vec<DiffLine> {
        vec![
            DiffLine {
                tag: DiffTag::Equal,
                content: "fn main() {".to_string(),
            },
            DiffLine {
                tag: DiffTag::Delete,
                content: "    old_line();".to_string(),
            },
            DiffLine {
                tag: DiffTag::Insert,
                content: "    new_line();".to_string(),
            },
            DiffLine {
                tag: DiffTag::Equal,
                content: "}".to_string(),
            },
        ]
    }

    fn make_panel() -> SidePanel {
        SidePanel::with_tabs(
            vec![
                SidePanelTab::Diff,
                SidePanelTab::Preview,
                SidePanelTab::TestLog,
            ],
            true,
        )
    }

    #[test]
    fn test_side_panel_basic() {
        let panel = make_panel();
        let diff = make_diff_lines();
        let grid = render_side_panel(&panel, &diff, 10, 40, 0);

        assert_eq!(grid.rows, 10);
        assert_eq!(grid.cols, 40);
        assert_eq!(grid.cells.len(), 10 * 40);
        assert!(!grid.cursor.visible);
    }

    #[test]
    fn test_side_panel_header_contains_tab_names() {
        let panel = make_panel();
        let grid = render_side_panel(&panel, &[], 5, 40, 0);

        // Row 0 should contain tab names
        let header: String = grid.cells[..40].iter().map(|c| c.c).collect();
        let trimmed = header.trim_end();
        assert!(trimmed.contains("Diff"), "header: '{trimmed}'");
        assert!(trimmed.contains("Preview"), "header: '{trimmed}'");
        assert!(trimmed.contains("TestLog"), "header: '{trimmed}'");
    }

    #[test]
    fn test_side_panel_active_tab_highlight() {
        let panel = make_panel();
        let grid = render_side_panel(&panel, &[], 5, 40, 0);

        // "Diff" is at index 0 (active). First char 'D' should have TAB_ACTIVE_BG
        assert_eq!(grid.cells[0].bg, TAB_ACTIVE_BG);
        assert_eq!(grid.cells[0].fg, LABEL_FG);
    }

    #[test]
    fn test_side_panel_separator_row() {
        let panel = make_panel();
        let grid = render_side_panel(&panel, &[], 5, 40, 0);

        // Row 1 is separator
        let sep_start = 40; // row 1, col 0
        assert_eq!(grid.cells[sep_start].c, '─');
        assert_eq!(grid.cells[sep_start].fg, DIM_FG);
    }

    #[test]
    fn test_side_panel_diff_colors() {
        let panel = make_panel();
        let diff = make_diff_lines();
        let grid = render_side_panel(&panel, &diff, 10, 40, 0);

        // Row 2 = first diff line (Equal), prefix ' '
        let row2_start = 2 * 40;
        assert_eq!(grid.cells[row2_start].c, ' ');
        assert_eq!(grid.cells[row2_start].fg, DIM_FG);

        // Row 3 = second diff line (Delete), prefix '-'
        let row3_start = 3 * 40;
        assert_eq!(grid.cells[row3_start].c, '-');
        assert_eq!(grid.cells[row3_start].fg, DIFF_DEL_FG);

        // Row 4 = third diff line (Insert), prefix '+'
        let row4_start = 4 * 40;
        assert_eq!(grid.cells[row4_start].c, '+');
        assert_eq!(grid.cells[row4_start].fg, DIFF_ADD_FG);
    }

    #[test]
    fn test_side_panel_scroll_offset() {
        let panel = make_panel();
        let diff = make_diff_lines();
        // Scroll past the first 2 lines
        let grid = render_side_panel(&panel, &diff, 10, 40, 2);

        // Row 2 should now show diff_lines[2] (Insert)
        let row2_start = 2 * 40;
        assert_eq!(grid.cells[row2_start].c, '+');
        assert_eq!(grid.cells[row2_start].fg, DIFF_ADD_FG);
    }

    #[test]
    fn test_side_panel_empty_diff() {
        let panel = make_panel();
        let grid = render_side_panel(&panel, &[], 5, 20, 0);

        // Content rows (2..5) should be spaces
        for row in 2..5 {
            for col in 0..20 {
                let idx = row * 20 + col;
                assert_eq!(grid.cells[idx].c, ' ');
            }
        }
    }

    #[test]
    fn test_side_panel_scroll_past_content() {
        let panel = make_panel();
        let diff = make_diff_lines(); // 4 lines
        let grid = render_side_panel(&panel, &diff, 10, 20, 100);

        // All content rows should be empty
        for row in 2..10 {
            assert_eq!(grid.cells[row * 20].c, ' ');
        }
    }

    #[test]
    fn test_side_panel_second_tab_active() {
        let mut panel = make_panel();
        panel.next_tab(); // Preview is now active
        let grid = render_side_panel(&panel, &[], 5, 40, 0);

        // "Preview" starts after "Diff | " (7 chars). Position 7 should be active.
        let preview_start = 7; // "Diff | " = 7 chars
        assert_eq!(grid.cells[preview_start].bg, TAB_ACTIVE_BG);

        // "Diff" chars should be inactive
        assert_eq!(grid.cells[0].fg, TAB_INACTIVE_FG);
    }
}
