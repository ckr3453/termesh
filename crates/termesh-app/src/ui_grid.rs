//! Converts UI elements (session list, side panel, header bar, status bar) into renderable
//! GridSnapshots.

use crate::theme::*;
use termesh_core::types::{AgentState, ViewMode, SPINNER_FRAMES};
use termesh_diff::diff_generator::{DiffLine, DiffTag};
use termesh_layout::session_list::SessionList;
use termesh_layout::side_panel::SidePanel;
use termesh_terminal::grid::{CursorState, GridSnapshot, RenderableCell};

// ── Session list ───────────────────────────────────────────────────────────

/// Render a session list into a GridSnapshot (minimal design).
///
/// Layout: entries only, no header/footer chrome.
/// ```text
///   ⠋ Backend
///   · Frontend                 shell
/// ```
/// Selected entry uses `BG_SELECTED` background.
/// When editing, the selected row shows an inline text input.
pub fn render_session_list(
    list: &SessionList,
    rows: usize,
    cols: usize,
    spinner_frame: usize,
    agent_kinds: &[String],
) -> GridSnapshot {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let mut cells = Vec::with_capacity(rows * cols);

    let is_editing = list.is_editing();

    for row in 0..rows {
        let entry = list.entries().get(row);
        let is_selected = entry.is_some() && row == list.selected_index();
        let bg = if is_selected { BG_SELECTED } else { BG_SURFACE };

        if is_selected && is_editing {
            // Inline editing: render "  {buffer}|" with cursor
            if let Some(edit) = list.edit_state() {
                let buffer = edit.text();
                let cursor_pos = edit.cursor();
                let prefix = "  ";
                let prefix_chars: Vec<char> = prefix.chars().collect();
                let buf_chars: Vec<char> = buffer.chars().collect();

                for col_idx in 0..cols {
                    if col_idx < prefix_chars.len() {
                        cells.push(RenderableCell {
                            row,
                            col: col_idx,
                            c: prefix_chars[col_idx],
                            fg: FG_PRIMARY,
                            bg,
                            ..Default::default()
                        });
                    } else {
                        let buf_idx = col_idx - prefix_chars.len();
                        let is_cursor = buf_idx == cursor_pos;
                        if buf_idx < buf_chars.len() {
                            cells.push(RenderableCell {
                                row,
                                col: col_idx,
                                c: buf_chars[buf_idx],
                                // Cursor: inverted colors
                                fg: if is_cursor { BG_SURFACE } else { FG_PRIMARY },
                                bg: if is_cursor { FG_PRIMARY } else { bg },
                                ..Default::default()
                            });
                        } else if is_cursor {
                            // Cursor at end of buffer
                            cells.push(RenderableCell {
                                row,
                                col: col_idx,
                                c: ' ',
                                fg: BG_SURFACE,
                                bg: FG_PRIMARY,
                                ..Default::default()
                            });
                        } else {
                            cells.push(RenderableCell {
                                row,
                                col: col_idx,
                                c: ' ',
                                fg: FG_PRIMARY,
                                bg,
                                ..Default::default()
                            });
                        }
                    }
                }
            } else {
                fill_row(&mut cells, row, cols, ' ', FG_PRIMARY, bg);
            }
        } else if let Some(entry) = entry {
            // Normal entry: "  {icon} {label}                claude"
            let (state_icon, state_fg) = state_icon_and_color(entry.state, spinner_frame);
            let fg = if entry.is_agent {
                FG_PRIMARY
            } else {
                FG_SECONDARY
            };

            // Right-side label: agent kind from dynamic lookup
            let right_label = agent_kinds.get(row).map(|s| s.as_str()).unwrap_or("");
            let right_chars: Vec<char> = right_label.chars().collect();
            let right_start = if right_chars.is_empty() {
                cols
            } else {
                cols.saturating_sub(right_chars.len() + 1)
            };

            // "  {icon} {label}"
            let label_chars: Vec<char> = entry.label.chars().collect();
            let icon_col = 2;
            let label_start = 4; // "  X "

            for col_idx in 0..cols {
                if col_idx < 2 {
                    // Padding
                    cells.push(RenderableCell {
                        row,
                        col: col_idx,
                        c: ' ',
                        fg,
                        bg,
                        ..Default::default()
                    });
                } else if col_idx == icon_col {
                    cells.push(RenderableCell {
                        row,
                        col: col_idx,
                        c: state_icon,
                        fg: state_fg,
                        bg,
                        ..Default::default()
                    });
                } else if col_idx == 3 {
                    // Space after icon
                    cells.push(RenderableCell {
                        row,
                        col: col_idx,
                        c: ' ',
                        fg,
                        bg,
                        ..Default::default()
                    });
                } else if col_idx >= label_start
                    && col_idx - label_start < label_chars.len()
                    && col_idx < right_start
                {
                    cells.push(RenderableCell {
                        row,
                        col: col_idx,
                        c: label_chars[col_idx - label_start],
                        fg,
                        bg,
                        ..Default::default()
                    });
                } else if col_idx >= right_start && col_idx - right_start < right_chars.len() {
                    cells.push(RenderableCell {
                        row,
                        col: col_idx,
                        c: right_chars[col_idx - right_start],
                        fg: FG_MUTED,
                        bg,
                        ..Default::default()
                    });
                } else {
                    cells.push(RenderableCell {
                        row,
                        col: col_idx,
                        c: ' ',
                        fg,
                        bg,
                        ..Default::default()
                    });
                }
            }
        } else {
            // Empty row
            fill_row(&mut cells, row, cols, ' ', FG_PRIMARY, BG_SURFACE);
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

/// Return a human-readable name for an agent state.
fn state_name(state: AgentState) -> &'static str {
    match state {
        AgentState::None => "",
        AgentState::Idle => "Idle",
        AgentState::Thinking => "Thinking",
        AgentState::WritingCode => "Writing",
        AgentState::RunningCommand => "Running",
        AgentState::WaitingForInput => "Waiting",
        AgentState::Success => "Done",
        AgentState::Error => "Error",
    }
}

/// Return the display character and color for an agent state.
fn state_icon_and_color(state: AgentState, spinner_frame: usize) -> (char, Rgba) {
    if state.is_spinning() {
        let frame = spinner_frame % SPINNER_FRAMES.len();
        (SPINNER_FRAMES[frame], ACCENT)
    } else {
        match state {
            AgentState::None => (' ', FG_SECONDARY),
            AgentState::Idle => ('\u{00B7}', FG_SECONDARY), // ·
            AgentState::WaitingForInput => ('?', STATUS_WAITING),
            AgentState::Success => ('\u{2713}', STATUS_SUCCESS), // ✓
            AgentState::Error => ('\u{2717}', STATUS_ERROR),     // ✗
            _ => (' ', FG_SECONDARY),
        }
    }
}

// ── Header bar ─────────────────────────────────────────────────────────────

/// Render a header bar into a GridSnapshot.
///
/// New minimal format: `  Backend                         ⠋ Thinking `
/// Left: session name. Right: state icon + state label (colored by state).
pub fn render_header_bar(
    cols: usize,
    _view_mode: ViewMode,
    session_label: Option<&str>,
    agent_state: Option<AgentState>,
    spinner_frame: usize,
) -> GridSnapshot {
    let cols = cols.max(1);
    let mut cells = Vec::with_capacity(cols);

    let left = match session_label {
        Some(label) => format!("  {label}"),
        None => "  termesh".to_string(),
    };
    let left_chars: Vec<char> = left.chars().collect();

    // Right side: state icon + state name
    let (right_text, state_fg) = match agent_state {
        Some(state) => {
            let (icon, fg) = state_icon_and_color(state, spinner_frame);
            let name = state_name(state);
            if name.is_empty() {
                (String::new(), fg)
            } else {
                (format!("{icon} {name} "), fg)
            }
        }
        None => (String::new(), FG_SECONDARY),
    };
    let right_chars: Vec<char> = right_text.chars().collect();
    let right_start = cols.saturating_sub(right_chars.len());

    for col in 0..cols {
        if col < left_chars.len() {
            cells.push(RenderableCell {
                row: 0,
                col,
                c: left_chars[col],
                fg: FG_PRIMARY,
                bg: BG_ELEVATED,
                ..Default::default()
            });
        } else if col >= right_start && col - right_start < right_chars.len() {
            cells.push(RenderableCell {
                row: 0,
                col,
                c: right_chars[col - right_start],
                fg: state_fg,
                bg: BG_ELEVATED,
                ..Default::default()
            });
        } else {
            cells.push(RenderableCell {
                row: 0,
                col,
                c: ' ',
                fg: FG_SECONDARY,
                bg: BG_ELEVATED,
                ..Default::default()
            });
        }
    }

    GridSnapshot {
        cells,
        rows: 1,
        cols,
        cursor: CursorState {
            row: 0,
            col: 0,
            visible: false,
        },
        selection: None,
    }
}

// ── Status bar ─────────────────────────────────────────────────────────────

/// Render a status bar into a GridSnapshot.
///
/// Format: ` ^N New  ^] Next  ^R Rename  ^E Diff          1/3`
pub fn render_status_bar(
    cols: usize,
    session_count: usize,
    selected_index: usize,
    view_mode: ViewMode,
) -> GridSnapshot {
    let cols = cols.max(1);
    let mut cells = Vec::with_capacity(cols);

    // Platform-aware modifier prefix
    #[cfg(target_os = "macos")]
    const P: &str = "⌘";
    #[cfg(not(target_os = "macos"))]
    const P: &str = "Ctrl+";

    let hints: Vec<(String, &str)> = match view_mode {
        ViewMode::Focus => vec![
            (format!("{P}N"), "New"),
            (format!("{P}W"), "Close"),
            (format!("{P}["), "Prev"),
            (format!("{P}]"), "Next"),
            (format!("{P}B"), "List"),
            (format!("{P}E"), "Diff"),
            (format!("{P}Enter"), "Split Mode"),
        ],
        ViewMode::Split => vec![
            (format!("{P}N"), "New"),
            (format!("{P}1-9"), "Pane"),
            (format!("{P}["), "Prev"),
            (format!("{P}]"), "Next"),
            (format!("{P}B"), "List"),
            (format!("{P}Enter"), "Focus Mode"),
        ],
    };

    let right = format!(" {}/{} ", selected_index + 1, session_count);
    let right_chars: Vec<char> = right.chars().collect();
    let right_start = cols.saturating_sub(right_chars.len());

    // Build hint string with interleaved colors
    let mut hint_segments: Vec<(String, Rgba)> = Vec::new();
    hint_segments.push((" ".to_string(), FG_SECONDARY));
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            hint_segments.push(("  ".to_string(), FG_SECONDARY));
        }
        hint_segments.push((key.to_string(), ACCENT));
        hint_segments.push((format!(" {desc}"), FG_SECONDARY));
    }

    // Flatten hint segments into (char, color) pairs
    let mut hint_chars: Vec<(char, Rgba)> = Vec::new();
    for (text, color) in &hint_segments {
        for c in text.chars() {
            hint_chars.push((c, *color));
        }
    }

    for col in 0..cols {
        if col < hint_chars.len() && col < right_start {
            let (c, fg) = hint_chars[col];
            cells.push(RenderableCell {
                row: 0,
                col,
                c,
                fg,
                bg: BG_ELEVATED,
                ..Default::default()
            });
        } else if col >= right_start && col - right_start < right_chars.len() {
            cells.push(RenderableCell {
                row: 0,
                col,
                c: right_chars[col - right_start],
                fg: FG_SECONDARY,
                bg: BG_ELEVATED,
                ..Default::default()
            });
        } else {
            cells.push(RenderableCell {
                row: 0,
                col,
                c: ' ',
                fg: FG_SECONDARY,
                bg: BG_ELEVATED,
                ..Default::default()
            });
        }
    }

    GridSnapshot {
        cells,
        rows: 1,
        cols,
        cursor: CursorState {
            row: 0,
            col: 0,
            visible: false,
        },
        selection: None,
    }
}

// ── Side panel (unchanged logic) ───────────────────────────────────────────

/// Render the side panel into a GridSnapshot.
///
/// Minimal design: " Changes" title row + diff content.
/// Empty state shows centered "No changes" message.
pub fn render_side_panel(
    _panel: &SidePanel,
    diff_lines: &[DiffLine],
    rows: usize,
    cols: usize,
    scroll_offset: usize,
) -> GridSnapshot {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let mut cells = Vec::with_capacity(rows * cols);

    // Row 0: title " Changes"
    let title = " Changes";
    let title_chars: Vec<char> = title.chars().collect();
    for col_idx in 0..cols {
        let c = title_chars.get(col_idx).copied().unwrap_or(' ');
        let fg = if col_idx < title_chars.len() {
            FG_SECONDARY
        } else {
            FG_MUTED
        };
        cells.push(RenderableCell {
            row: 0,
            col: col_idx,
            c,
            fg,
            bg: BG_ELEVATED,
            ..Default::default()
        });
    }

    let content_rows = rows.saturating_sub(1);

    if diff_lines.is_empty() {
        // Empty state: center "No changes" in the content area
        let msg = "No changes";
        let msg_chars: Vec<char> = msg.chars().collect();
        let center_row = content_rows / 2;
        let center_col = cols.saturating_sub(msg_chars.len()) / 2;

        for content_row in 0..content_rows {
            let row_idx = content_row + 1;
            for col_idx in 0..cols {
                if content_row == center_row
                    && col_idx >= center_col
                    && col_idx - center_col < msg_chars.len()
                {
                    cells.push(RenderableCell {
                        row: row_idx,
                        col: col_idx,
                        c: msg_chars[col_idx - center_col],
                        fg: FG_MUTED,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                } else {
                    cells.push(RenderableCell {
                        row: row_idx,
                        col: col_idx,
                        c: ' ',
                        fg: FG_MUTED,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                }
            }
        }
    } else {
        // Diff content (scrollable)
        for content_row in 0..content_rows {
            let row_idx = content_row + 1;
            let line_idx = scroll_offset + content_row;

            if let Some(diff_line) = diff_lines.get(line_idx) {
                let (prefix, fg) = match diff_line.tag {
                    DiffTag::Insert => ('+', DIFF_ADD),
                    DiffTag::Delete => ('-', DIFF_DEL),
                    DiffTag::Equal => (' ', FG_SECONDARY),
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
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                }
            } else {
                fill_row(&mut cells, row_idx, cols, ' ', FG_SECONDARY, BG_SURFACE);
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

// ── Private helpers ────────────────────────────────────────────────────────

/// Fill an entire row with a single character and color.
fn fill_row(cells: &mut Vec<RenderableCell>, row: usize, cols: usize, c: char, fg: Rgba, bg: Rgba) {
    for col in 0..cols {
        cells.push(RenderableCell {
            row,
            col,
            c,
            fg,
            bg,
            ..Default::default()
        });
    }
}

// ── Split pane header ─────────────────────────────────────────────────────

/// Render a 1-row pane header for Split mode.
///
/// Format: ` {session_number} {label} {agent_kind}     {icon} {state_name} `
pub fn render_pane_header(
    label: &str,
    agent_kind: &str,
    state: AgentState,
    is_focused: bool,
    cols: usize,
    spinner_frame: usize,
    session_index: usize,
) -> GridSnapshot {
    let cols = cols.max(1);
    let mut cells = Vec::with_capacity(cols);
    let bg = BG_ELEVATED;

    // Left side: session number + label + agent kind
    let left = format!(" {} {label} {agent_kind}", session_index + 1);
    let left_chars: Vec<char> = left.chars().collect();

    // Right side: state
    let (icon, state_fg) = state_icon_and_color(state, spinner_frame);
    let name = state_name(state);
    let right_text = if name.is_empty() {
        String::new()
    } else {
        format!("{icon} {name} ")
    };
    let right_chars: Vec<char> = right_text.chars().collect();
    let right_start = cols.saturating_sub(right_chars.len());

    for col in 0..cols {
        if col == 0 && is_focused {
            // Focused pane: accent bar on leftmost column
            cells.push(RenderableCell {
                row: 0,
                col,
                c: '\u{2502}', // │
                fg: ACCENT,
                bg,
                ..Default::default()
            });
        } else if col < left_chars.len() {
            cells.push(RenderableCell {
                row: 0,
                col,
                c: left_chars[col],
                fg: if is_focused { FG_PRIMARY } else { FG_SECONDARY },
                bg,
                ..Default::default()
            });
        } else if col >= right_start && col - right_start < right_chars.len() {
            cells.push(RenderableCell {
                row: 0,
                col,
                c: right_chars[col - right_start],
                fg: state_fg,
                bg,
                ..Default::default()
            });
        } else {
            cells.push(RenderableCell {
                row: 0,
                col,
                c: ' ',
                fg: FG_MUTED,
                bg,
                ..Default::default()
            });
        }
    }

    GridSnapshot {
        cells,
        rows: 1,
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
    use termesh_core::types::{SessionId, SidePanelTab};
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

    fn make_agent_kinds() -> Vec<String> {
        vec!["claude".to_string(), "shell".to_string()]
    }

    #[test]
    fn test_render_basic() {
        let list = make_list();
        let grid = render_session_list(&list, 15, 25, 0, &make_agent_kinds());

        assert_eq!(grid.rows, 15);
        assert_eq!(grid.cols, 25);
        assert_eq!(grid.cells.len(), 15 * 25);
        assert!(!grid.cursor.visible);
    }

    #[test]
    fn test_session_entry_has_content() {
        let list = make_list();
        let grid = render_session_list(&list, 15, 30, 0, &make_agent_kinds());

        // Row 0 = first entry (selected by default, no header)
        let entry_row: String = grid.cells[0..30].iter().map(|c| c.c).collect();
        let trimmed = entry_row.trim_end();
        assert!(trimmed.contains("Backend"), "got: '{trimmed}'");
    }

    #[test]
    fn test_selected_entry_highlighted() {
        let list = make_list();
        let grid = render_session_list(&list, 15, 25, 0, &make_agent_kinds());

        // Row 0 (first entry) should have BG_SELECTED
        let row0_cell = &grid.cells[0];
        assert_eq!(row0_cell.bg, BG_SELECTED);

        // Row 1 (second entry) should have BG_SURFACE
        let row1_cell = &grid.cells[25];
        assert_eq!(row1_cell.bg, BG_SURFACE);
    }

    #[test]
    fn test_shell_entry_has_shell_label() {
        let list = make_list();
        let grid = render_session_list(&list, 15, 25, 0, &make_agent_kinds());

        // Row 1 = Shell entry
        let row1: String = grid.cells[25..50].iter().map(|c| c.c).collect();
        assert!(row1.contains("shell"), "row1: '{row1}'");
    }

    #[test]
    fn test_empty_list() {
        let list = SessionList::new();
        let grid = render_session_list(&list, 10, 15, 0, &[]);

        assert_eq!(grid.rows, 10);
        assert_eq!(grid.cols, 15);
        assert_eq!(grid.cells.len(), 10 * 15);
    }

    #[test]
    fn test_narrow_cols() {
        let list = make_list();
        let grid = render_session_list(&list, 10, 3, 0, &make_agent_kinds());

        assert_eq!(grid.cols, 3);
        assert_eq!(grid.cells.len(), 10 * 3);
    }

    #[test]
    fn test_editing_mode_render() {
        let mut list = make_list();
        list.start_editing();
        let grid = render_session_list(&list, 15, 30, 0, &make_agent_kinds());

        // Row 0 (editing) should have BG_SELECTED background
        assert_eq!(grid.cells[0].bg, BG_SELECTED);
        // Buffer content "Backend" should appear starting at col 2
        let row0: String = grid.cells[0..30].iter().map(|c| c.c).collect();
        assert!(row0.contains("Backend"), "editing row: '{row0}'");
    }

    // ── Header bar tests ───────────────────────────────────────────────────

    #[test]
    fn test_header_bar_basic() {
        let grid = render_header_bar(
            60,
            ViewMode::Focus,
            Some("Backend"),
            Some(AgentState::Thinking),
            0,
        );

        assert_eq!(grid.rows, 1);
        assert_eq!(grid.cols, 60);
        assert_eq!(grid.cells.len(), 60);

        let text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(text.contains("Backend"), "header: '{text}'");
        assert!(text.contains("Thinking"), "header: '{text}'");
    }

    #[test]
    fn test_header_bar_no_session() {
        let grid = render_header_bar(60, ViewMode::Split, None, None, 0);

        let text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(text.contains("termesh"), "header: '{text}'");
    }

    #[test]
    fn test_header_bar_primary_color() {
        let grid = render_header_bar(60, ViewMode::Focus, Some("Test"), None, 0);

        // " " space at col 0, then "T" at col 2
        assert_eq!(grid.cells[2].fg, FG_PRIMARY);
    }

    // ── Status bar tests ───────────────────────────────────────────────────

    #[test]
    fn test_status_bar_basic() {
        let grid = render_status_bar(60, 3, 0, ViewMode::Focus);

        assert_eq!(grid.rows, 1);
        assert_eq!(grid.cols, 60);
        assert_eq!(grid.cells.len(), 60);

        let text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(text.contains("New"), "status: '{text}'");
        assert!(text.contains("1/3"), "status: '{text}'");
    }

    #[test]
    fn test_status_bar_session_count() {
        let grid = render_status_bar(60, 5, 2, ViewMode::Focus);

        let text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(text.contains("3/5"), "status: '{text}'");
    }

    #[test]
    fn test_status_bar_has_rename_hint() {
        let grid = render_status_bar(60, 1, 0, ViewMode::Focus);

        let text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(text.contains("Rename"), "status: '{text}'");
    }

    // ── Side panel tests ───────────────────────────────────────────────────

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
        SidePanel::with_tabs(vec![SidePanelTab::Diff], true)
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
    fn test_side_panel_title() {
        let panel = make_panel();
        let grid = render_side_panel(&panel, &[], 5, 40, 0);

        let header: String = grid.cells[..40].iter().map(|c| c.c).collect();
        assert!(header.contains("Changes"), "header: '{header}'");
    }

    #[test]
    fn test_side_panel_title_bg_elevated() {
        let panel = make_panel();
        let grid = render_side_panel(&panel, &[], 5, 40, 0);

        assert_eq!(grid.cells[0].bg, BG_ELEVATED);
    }

    #[test]
    fn test_side_panel_diff_colors() {
        let panel = make_panel();
        let diff = make_diff_lines();
        let grid = render_side_panel(&panel, &diff, 10, 40, 0);

        // Row 1 = first diff line (equal), row 2 = delete, row 3 = insert
        let row1_start = 40;
        assert_eq!(grid.cells[row1_start].c, ' ');
        assert_eq!(grid.cells[row1_start].fg, FG_SECONDARY);

        let row2_start = 2 * 40;
        assert_eq!(grid.cells[row2_start].c, '-');
        assert_eq!(grid.cells[row2_start].fg, DIFF_DEL);

        let row3_start = 3 * 40;
        assert_eq!(grid.cells[row3_start].c, '+');
        assert_eq!(grid.cells[row3_start].fg, DIFF_ADD);
    }

    #[test]
    fn test_side_panel_scroll_offset() {
        let panel = make_panel();
        let diff = make_diff_lines();
        let grid = render_side_panel(&panel, &diff, 10, 40, 2);

        // Scroll by 2: row 1 should show diff_lines[2] (Insert)
        let row1_start = 40;
        assert_eq!(grid.cells[row1_start].c, '+');
        assert_eq!(grid.cells[row1_start].fg, DIFF_ADD);
    }

    #[test]
    fn test_side_panel_empty_diff_shows_message() {
        let panel = make_panel();
        let grid = render_side_panel(&panel, &[], 5, 30, 0);

        // "No changes" should appear somewhere in the content area
        let all_text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(
            all_text.contains("No changes"),
            "expected 'No changes': '{all_text}'"
        );
    }

    #[test]
    fn test_side_panel_scroll_past_content() {
        let panel = make_panel();
        let diff = make_diff_lines();
        let grid = render_side_panel(&panel, &diff, 10, 20, 100);

        for row in 1..10 {
            assert_eq!(grid.cells[row * 20].c, ' ');
        }
    }
}
