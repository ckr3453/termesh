//! Converts UI elements (session list, side panel, header bar, status bar) into renderable
//! GridSnapshots.

use crate::theme::*;
use termesh_core::types::{AgentState, ViewMode, SPINNER_FRAMES};
use termesh_diff::diff_generator::{DiffLine, DiffTag, SideBySideLine};
use termesh_diff::history::ChangedFile;
use termesh_layout::session_list::SessionList;
use termesh_layout::side_panel::SidePanel;
use termesh_terminal::grid::{CursorState, GridSnapshot, RenderableCell};

// ── Session list ───────────────────────────────────────────────────────────

/// What a visual row in the session list represents.
enum DisplayRow {
    /// Project name header (e.g. "  termesh").
    Header(String),
    /// An entry from `SessionList::entries()` at the given index.
    Entry(usize),
}

/// Build a display plan that groups entries by git project.
///
/// Consecutive entries with the same non-empty project get a single header.
/// Entries with an empty project string appear without a header.
fn build_display_plan(entry_count: usize, git_projects: &[String]) -> Vec<DisplayRow> {
    let mut plan = Vec::with_capacity(entry_count * 2);
    let mut prev_project: Option<&str> = None;

    for idx in 0..entry_count {
        let project = git_projects.get(idx).map(|s| s.as_str()).unwrap_or("");
        if !project.is_empty() {
            let need_header = match prev_project {
                Some(prev) => prev != project,
                None => true,
            };
            if need_header {
                plan.push(DisplayRow::Header(project.to_string()));
            }
        }
        plan.push(DisplayRow::Entry(idx));
        prev_project = Some(project);
    }

    plan
}

/// Compute scroll offset so the selected entry is visible within the viewport.
fn compute_scroll_offset(
    plan: &[DisplayRow],
    selected_index: usize,
    viewport_rows: usize,
) -> usize {
    // Find the visual row of the selected entry.
    let selected_row = plan
        .iter()
        .position(|r| matches!(r, DisplayRow::Entry(idx) if *idx == selected_index));

    let Some(selected_row) = selected_row else {
        return 0;
    };

    // If the selected row is beyond the viewport, scroll down.
    if selected_row >= viewport_rows {
        selected_row - viewport_rows + 1
    } else {
        0
    }
}

/// Render a session list into a GridSnapshot (minimal design).
///
/// Layout: entries grouped by git project, with optional project headers.
/// ```text
/// ─ termesh ─────────────
///   ⠋ Backend          claude
///   · Frontend         shell
/// ─ my-app ──────────────
///   ⠋ API              claude
/// ```
/// Selected entry uses `BG_SELECTED` background.
/// When editing, the selected row shows an inline text input.
pub fn render_session_list(
    list: &SessionList,
    rows: usize,
    cols: usize,
    spinner_frame: usize,
    agent_kinds: &[String],
    git_projects: &[String],
) -> GridSnapshot {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let mut cells = Vec::with_capacity(rows * cols);

    let is_editing = list.is_editing();
    let display_plan = build_display_plan(list.entries().len(), git_projects);

    // Compute scroll offset so the selected entry is always visible.
    let scroll_offset = compute_scroll_offset(&display_plan, list.selected_index(), rows);

    for row in 0..rows {
        match display_plan.get(row + scroll_offset) {
            Some(DisplayRow::Header(project)) => {
                // "  {project}" with remaining space filled by blanks
                let prefix = format!("  {}", project);
                let prefix_chars: Vec<char> = prefix.chars().collect();
                for col_idx in 0..cols {
                    let c = if col_idx < prefix_chars.len() {
                        prefix_chars[col_idx]
                    } else {
                        ' '
                    };
                    cells.push(RenderableCell {
                        row,
                        col: col_idx,
                        c,
                        fg: FG_MUTED,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                }
            }
            Some(DisplayRow::Entry(idx)) => {
                let entry = &list.entries()[*idx];
                let is_selected = *idx == list.selected_index();
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
                } else {
                    // Normal entry: "  {icon} {label}                claude"
                    let (state_icon, state_fg) = state_icon_and_color(entry.state, spinner_frame);
                    let fg = if entry.is_agent {
                        FG_PRIMARY
                    } else {
                        FG_SECONDARY
                    };

                    // Right-side label: agent kind from dynamic lookup
                    let right_label = agent_kinds.get(*idx).map(|s| s.as_str()).unwrap_or("");
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
                        } else if col_idx >= right_start
                            && col_idx - right_start < right_chars.len()
                        {
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
                }
            }
            None => {
                // Empty row
                fill_row(&mut cells, row, cols, ' ', FG_PRIMARY, BG_SURFACE);
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
            (format!("{P}S"), "Swap"),
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

// ── Side-by-side diff ─────────────────────────────────────────────────────

/// Render a side-by-side diff into a GridSnapshot.
///
/// Layout:
/// ```text
/// Row 0: " Changes (side-by-side)"
/// Row 1+: left_half │ right_half
/// ```
pub fn render_side_by_side(
    _panel: &SidePanel,
    sbs_lines: &[SideBySideLine],
    rows: usize,
    cols: usize,
    scroll_offset: usize,
) -> GridSnapshot {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let mut cells = Vec::with_capacity(rows * cols);

    // Row 0: title
    let title = " Changes (side-by-side)";
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
    // Split: left half + divider + right half
    let half = cols.saturating_sub(1) / 2;
    let div_col = half;

    if sbs_lines.is_empty() {
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
        for content_row in 0..content_rows {
            let row_idx = content_row + 1;
            let line_idx = scroll_offset + content_row;

            if let Some(sbs) = sbs_lines.get(line_idx) {
                let left_text = sbs.left.as_deref().unwrap_or("");
                let right_text = sbs.right.as_deref().unwrap_or("");
                let left_chars: Vec<char> = left_text.trim_end_matches('\n').chars().collect();
                let right_chars: Vec<char> = right_text.trim_end_matches('\n').chars().collect();

                let (left_fg, right_fg) = match sbs.tag {
                    DiffTag::Equal => (FG_SECONDARY, FG_SECONDARY),
                    DiffTag::Delete => (DIFF_DEL, DIFF_ADD),
                    DiffTag::Insert => (FG_MUTED, DIFF_ADD),
                };

                for col_idx in 0..cols {
                    if col_idx == div_col {
                        // Divider column
                        cells.push(RenderableCell {
                            row: row_idx,
                            col: col_idx,
                            c: '\u{2502}', // │
                            fg: FG_MUTED,
                            bg: BG_SURFACE,
                            ..Default::default()
                        });
                    } else if col_idx < div_col {
                        // Left half
                        let c = left_chars.get(col_idx).copied().unwrap_or(' ');
                        cells.push(RenderableCell {
                            row: row_idx,
                            col: col_idx,
                            c,
                            fg: left_fg,
                            bg: BG_SURFACE,
                            ..Default::default()
                        });
                    } else {
                        // Right half
                        let right_idx = col_idx - div_col - 1;
                        let c = right_chars.get(right_idx).copied().unwrap_or(' ');
                        cells.push(RenderableCell {
                            row: row_idx,
                            col: col_idx,
                            c,
                            fg: right_fg,
                            bg: BG_SURFACE,
                            ..Default::default()
                        });
                    }
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

// ── File list (side panel) ─────────────────────────────────────────────────

/// Render the side panel file list into a GridSnapshot.
///
/// Layout:
/// ```text
/// Row 0: " Changes (N files)"
/// Row 1+: " M path/to/file.rs         +5 -2"
/// ```
/// Selected row uses `BG_SELECTED` background.
pub fn render_file_list(
    _panel: &SidePanel,
    files: &[ChangedFile],
    selected: usize,
    rows: usize,
    cols: usize,
) -> GridSnapshot {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let mut cells = Vec::with_capacity(rows * cols);

    // Row 0: title
    let title = if files.is_empty() {
        " Changes".to_string()
    } else {
        format!(" Changes ({} files)", files.len())
    };
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

    if files.is_empty() {
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
        for content_row in 0..content_rows {
            let row_idx = content_row + 1;
            let is_selected = content_row < files.len() && content_row == selected;
            let bg = if is_selected { BG_SELECTED } else { BG_SURFACE };

            if let Some(file) = files.get(content_row) {
                // Format: " M path/to/file.rs       +5 -2"
                let filename = file
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.path.to_string_lossy().to_string());

                let status_fg = match file.status {
                    'A' => DIFF_ADD,
                    'M' => STATUS_WAITING,
                    _ => FG_SECONDARY,
                };

                // Right side: colored stats segments
                let add_str = format!("+{}", file.insertions);
                let del_str = format!("-{}", file.deletions);
                // Build (char, color) pairs for right side
                let mut right_parts: Vec<(char, Rgba)> = Vec::new();
                for c in add_str.chars() {
                    right_parts.push((c, DIFF_ADD));
                }
                right_parts.push((' ', FG_MUTED));
                for c in del_str.chars() {
                    right_parts.push((c, DIFF_DEL));
                }
                let stats_start = cols.saturating_sub(right_parts.len() + 1);

                // Left side: " M filename"
                let left = format!(" {} {}", file.status, filename);
                let left_chars: Vec<char> = left.chars().collect();

                for col_idx in 0..cols {
                    if col_idx == 1 {
                        // Status character
                        cells.push(RenderableCell {
                            row: row_idx,
                            col: col_idx,
                            c: file.status,
                            fg: status_fg,
                            bg,
                            ..Default::default()
                        });
                    } else if col_idx < left_chars.len() && col_idx < stats_start {
                        cells.push(RenderableCell {
                            row: row_idx,
                            col: col_idx,
                            c: left_chars[col_idx],
                            fg: FG_PRIMARY,
                            bg,
                            ..Default::default()
                        });
                    } else if col_idx >= stats_start && col_idx - stats_start < right_parts.len() {
                        let (c, fg) = right_parts[col_idx - stats_start];
                        cells.push(RenderableCell {
                            row: row_idx,
                            col: col_idx,
                            c,
                            fg,
                            bg,
                            ..Default::default()
                        });
                    } else {
                        cells.push(RenderableCell {
                            row: row_idx,
                            col: col_idx,
                            c: ' ',
                            fg: FG_MUTED,
                            bg,
                            ..Default::default()
                        });
                    }
                }
            } else {
                fill_row(&mut cells, row_idx, cols, ' ', FG_MUTED, BG_SURFACE);
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

// ── Shared rendering helpers ──────────────────────────────────────────────

/// Render a centered text row into cells.
pub(crate) fn push_centered_row(
    cells: &mut Vec<RenderableCell>,
    row: usize,
    cols: usize,
    text: &str,
    fg: Rgba,
    bg: Rgba,
) {
    let chars: Vec<char> = text.chars().collect();
    let pad = cols.saturating_sub(chars.len()) / 2;
    for col in 0..cols {
        let ch_idx = col.wrapping_sub(pad);
        let c = if col >= pad && ch_idx < chars.len() {
            chars[ch_idx]
        } else {
            ' '
        };
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

/// Fill an entire row with a single character and color.
pub(crate) fn fill_row(
    cells: &mut Vec<RenderableCell>,
    row: usize,
    cols: usize,
    c: char,
    fg: Rgba,
    bg: Rgba,
) {
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
        let grid = render_session_list(&list, 15, 25, 0, &make_agent_kinds(), &[]);

        assert_eq!(grid.rows, 15);
        assert_eq!(grid.cols, 25);
        assert_eq!(grid.cells.len(), 15 * 25);
        assert!(!grid.cursor.visible);
    }

    #[test]
    fn test_session_entry_has_content() {
        let list = make_list();
        let grid = render_session_list(&list, 15, 30, 0, &make_agent_kinds(), &[]);

        // Row 0 = first entry (selected by default, no header)
        let entry_row: String = grid.cells[0..30].iter().map(|c| c.c).collect();
        let trimmed = entry_row.trim_end();
        assert!(trimmed.contains("Backend"), "got: '{trimmed}'");
    }

    #[test]
    fn test_selected_entry_highlighted() {
        let list = make_list();
        let grid = render_session_list(&list, 15, 25, 0, &make_agent_kinds(), &[]);

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
        let grid = render_session_list(&list, 15, 25, 0, &make_agent_kinds(), &[]);

        // Row 1 = Shell entry
        let row1: String = grid.cells[25..50].iter().map(|c| c.c).collect();
        assert!(row1.contains("shell"), "row1: '{row1}'");
    }

    #[test]
    fn test_empty_list() {
        let list = SessionList::new();
        let grid = render_session_list(&list, 10, 15, 0, &[], &[]);

        assert_eq!(grid.rows, 10);
        assert_eq!(grid.cols, 15);
        assert_eq!(grid.cells.len(), 10 * 15);
    }

    #[test]
    fn test_narrow_cols() {
        let list = make_list();
        let grid = render_session_list(&list, 10, 3, 0, &make_agent_kinds(), &[]);

        assert_eq!(grid.cols, 3);
        assert_eq!(grid.cells.len(), 10 * 3);
    }

    #[test]
    fn test_editing_mode_render() {
        let mut list = make_list();
        list.start_editing();
        let grid = render_session_list(&list, 15, 30, 0, &make_agent_kinds(), &[]);

        // Row 0 (editing) should have BG_SELECTED background
        assert_eq!(grid.cells[0].bg, BG_SELECTED);
        // Buffer content "Backend" should appear starting at col 2
        let row0: String = grid.cells[0..30].iter().map(|c| c.c).collect();
        assert!(row0.contains("Backend"), "editing row: '{row0}'");
    }

    // ── Git project grouping tests ──────────────────────────────────────────

    #[test]
    fn test_build_display_plan_no_projects() {
        let plan = build_display_plan(2, &[]);
        assert_eq!(plan.len(), 2);
        assert!(matches!(plan[0], DisplayRow::Entry(0)));
        assert!(matches!(plan[1], DisplayRow::Entry(1)));
    }

    #[test]
    fn test_build_display_plan_same_project() {
        let projects = vec!["termesh".to_string(), "termesh".to_string()];
        let plan = build_display_plan(2, &projects);
        // Header + 2 entries
        assert_eq!(plan.len(), 3);
        assert!(matches!(&plan[0], DisplayRow::Header(p) if p == "termesh"));
        assert!(matches!(plan[1], DisplayRow::Entry(0)));
        assert!(matches!(plan[2], DisplayRow::Entry(1)));
    }

    #[test]
    fn test_build_display_plan_different_projects() {
        let projects = vec!["termesh".to_string(), "my-app".to_string()];
        let plan = build_display_plan(2, &projects);
        // Header + entry + header + entry
        assert_eq!(plan.len(), 4);
        assert!(matches!(&plan[0], DisplayRow::Header(p) if p == "termesh"));
        assert!(matches!(plan[1], DisplayRow::Entry(0)));
        assert!(matches!(&plan[2], DisplayRow::Header(p) if p == "my-app"));
        assert!(matches!(plan[3], DisplayRow::Entry(1)));
    }

    #[test]
    fn test_build_display_plan_mixed_with_empty() {
        let projects = vec!["termesh".to_string(), String::new(), "termesh".to_string()];
        let plan = build_display_plan(3, &projects);
        // Header("termesh") + Entry(0) + Entry(1) + Header("termesh") + Entry(2)
        assert_eq!(plan.len(), 5);
        assert!(matches!(&plan[0], DisplayRow::Header(p) if p == "termesh"));
        assert!(matches!(plan[1], DisplayRow::Entry(0)));
        assert!(matches!(plan[2], DisplayRow::Entry(1))); // no header for empty
        assert!(matches!(&plan[3], DisplayRow::Header(p) if p == "termesh"));
        assert!(matches!(plan[4], DisplayRow::Entry(2)));
    }

    #[test]
    fn test_grouped_render_header_row() {
        let list = make_list();
        let projects = vec!["termesh".to_string(), "termesh".to_string()];
        let grid = render_session_list(&list, 15, 30, 0, &make_agent_kinds(), &projects);

        // Row 0 = header "  termesh   ..."
        let row0: String = grid.cells[0..30].iter().map(|c| c.c).collect();
        assert!(row0.contains("termesh"), "header row: '{row0}'");
        assert!(
            row0.starts_with("  termesh"),
            "should start with '  termesh': '{row0}'"
        );

        // Header should use FG_MUTED
        assert_eq!(grid.cells[0].fg, FG_MUTED);
        assert_eq!(grid.cells[0].bg, BG_SURFACE);

        // Row 1 = first entry (selected, BG_SELECTED)
        let row1: String = grid.cells[30..60].iter().map(|c| c.c).collect();
        assert!(row1.contains("Backend"), "entry row: '{row1}'");
        assert_eq!(grid.cells[30].bg, BG_SELECTED);

        // Row 2 = second entry (not selected, BG_SURFACE)
        let row2: String = grid.cells[60..90].iter().map(|c| c.c).collect();
        assert!(row2.contains("Shell"), "entry row: '{row2}'");
        assert_eq!(grid.cells[60].bg, BG_SURFACE);
    }

    #[test]
    fn test_grouped_render_multiple_projects() {
        let mut list = SessionList::new();
        list.add(SessionEntry {
            id: SessionId(1),
            label: "Backend".to_string(),
            is_agent: true,
            state: AgentState::Thinking,
        });
        list.add(SessionEntry {
            id: SessionId(2),
            label: "Frontend".to_string(),
            is_agent: true,
            state: AgentState::Idle,
        });
        let kinds = vec!["claude".to_string(), "claude".to_string()];
        let projects = vec!["termesh".to_string(), "my-app".to_string()];
        let grid = render_session_list(&list, 15, 30, 0, &kinds, &projects);

        // Row 0 = header "termesh"
        let row0: String = grid.cells[0..30].iter().map(|c| c.c).collect();
        assert!(row0.contains("termesh"), "header: '{row0}'");

        // Row 1 = Backend entry
        let row1: String = grid.cells[30..60].iter().map(|c| c.c).collect();
        assert!(row1.contains("Backend"), "entry: '{row1}'");

        // Row 2 = header "my-app"
        let row2: String = grid.cells[60..90].iter().map(|c| c.c).collect();
        assert!(row2.contains("my-app"), "header: '{row2}'");

        // Row 3 = Frontend entry
        let row3: String = grid.cells[90..120].iter().map(|c| c.c).collect();
        assert!(row3.contains("Frontend"), "entry: '{row3}'");
    }

    #[test]
    fn test_grouped_selection_after_header_offset() {
        let mut list = make_list();
        list.select_next(); // select entry index 1 (Shell)
        let projects = vec!["termesh".to_string(), "termesh".to_string()];
        let grid = render_session_list(&list, 15, 30, 0, &make_agent_kinds(), &projects);

        // Display plan: Header(row 0), Entry 0(row 1), Entry 1(row 2)
        // Entry 0 (Backend) at row 1 should NOT be selected
        assert_eq!(grid.cells[30].bg, BG_SURFACE);
        // Entry 1 (Shell) at row 2 should be selected
        assert_eq!(grid.cells[60].bg, BG_SELECTED);
    }

    // ── Scroll tests ──────────────────────────────────────────────────────

    #[test]
    fn test_scroll_offset_no_scroll_needed() {
        let plan = vec![
            DisplayRow::Header("proj".to_string()),
            DisplayRow::Entry(0),
            DisplayRow::Entry(1),
        ];
        assert_eq!(compute_scroll_offset(&plan, 0, 5), 0);
        assert_eq!(compute_scroll_offset(&plan, 1, 5), 0);
    }

    #[test]
    fn test_scroll_offset_selected_below_viewport() {
        // 4 entries with 2 headers = 6 display rows, viewport = 3
        let plan = vec![
            DisplayRow::Header("a".to_string()),
            DisplayRow::Entry(0),
            DisplayRow::Entry(1),
            DisplayRow::Header("b".to_string()),
            DisplayRow::Entry(2),
            DisplayRow::Entry(3),
        ];
        // Entry 3 is at visual row 5, viewport = 3 → offset = 5 - 3 + 1 = 3
        assert_eq!(compute_scroll_offset(&plan, 3, 3), 3);
    }

    #[test]
    fn test_scroll_renders_selected_visible() {
        // Create 4 entries in 2 projects, but only 3 rows of viewport
        let mut list = SessionList::new();
        for i in 0..4 {
            list.add(SessionEntry {
                id: SessionId(i as u64),
                label: format!("S{i}"),
                is_agent: true,
                state: AgentState::Idle,
            });
        }
        // Select last entry
        list.select_next(); // 1
        list.select_next(); // 2
        list.select_next(); // 3

        let kinds = vec![
            "claude".into(),
            "claude".into(),
            "claude".into(),
            "claude".into(),
        ];
        let projects = vec!["a".into(), "a".into(), "b".into(), "b".into()];
        // Display plan: Header(a), E0, E1, Header(b), E2, E3 = 6 rows
        // Viewport = 3 rows, selected = entry 3 (visual row 5)
        let grid = render_session_list(&list, 3, 20, 0, &kinds, &projects);

        // The selected entry (S3) must be visible somewhere in the 3-row viewport
        let all_text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(
            all_text.contains("S3"),
            "selected entry should be visible: '{all_text}'"
        );

        // And it should have BG_SELECTED
        let has_selected_bg = grid.cells.iter().any(|c| c.bg == BG_SELECTED);
        assert!(has_selected_bg, "selected entry should be highlighted");
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
    fn test_status_bar_has_list_hint() {
        let grid = render_status_bar(100, 1, 0, ViewMode::Focus);

        let text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(text.contains("List"), "status: '{text}'");
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

    // ── File list tests ───────────────────────────────────────────────────

    fn make_changed_files() -> Vec<ChangedFile> {
        vec![
            ChangedFile {
                path: std::path::PathBuf::from("src/main.rs"),
                status: 'M',
                insertions: 5,
                deletions: 2,
            },
            ChangedFile {
                path: std::path::PathBuf::from("src/lib.rs"),
                status: 'A',
                insertions: 10,
                deletions: 0,
            },
        ]
    }

    #[test]
    fn test_file_list_basic() {
        let panel = make_panel();
        let files = make_changed_files();
        let grid = render_file_list(&panel, &files, 0, 10, 40);

        assert_eq!(grid.rows, 10);
        assert_eq!(grid.cols, 40);
        assert_eq!(grid.cells.len(), 10 * 40);
    }

    #[test]
    fn test_file_list_title() {
        let panel = make_panel();
        let files = make_changed_files();
        let grid = render_file_list(&panel, &files, 0, 10, 40);

        let header: String = grid.cells[..40].iter().map(|c| c.c).collect();
        assert!(header.contains("Changes"), "header: '{header}'");
        assert!(header.contains("2 files"), "header: '{header}'");
    }

    #[test]
    fn test_file_list_selected_highlighted() {
        let panel = make_panel();
        let files = make_changed_files();
        let grid = render_file_list(&panel, &files, 0, 10, 40);

        // Row 1 (selected=0) should have BG_SELECTED
        let row1_cell = &grid.cells[40];
        assert_eq!(row1_cell.bg, BG_SELECTED);

        // Row 2 (not selected) should have BG_SURFACE
        let row2_cell = &grid.cells[80];
        assert_eq!(row2_cell.bg, BG_SURFACE);
    }

    #[test]
    fn test_file_list_status_chars() {
        let panel = make_panel();
        let files = make_changed_files();
        let grid = render_file_list(&panel, &files, 0, 10, 40);

        // Row 1, col 1 should be 'M'
        assert_eq!(grid.cells[40 + 1].c, 'M');
        // Row 2, col 1 should be 'A'
        assert_eq!(grid.cells[80 + 1].c, 'A');
    }

    #[test]
    fn test_file_list_empty() {
        let panel = make_panel();
        let grid = render_file_list(&panel, &[], 0, 10, 40);

        let all_text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(
            all_text.contains("No changes"),
            "expected 'No changes': '{all_text}'"
        );
    }

    #[test]
    fn test_file_list_contains_filename() {
        let panel = make_panel();
        let files = make_changed_files();
        let grid = render_file_list(&panel, &files, 0, 10, 40);

        let row1: String = grid.cells[40..80].iter().map(|c| c.c).collect();
        assert!(row1.contains("main.rs"), "row1: '{row1}'");
    }

    // ── Side-by-side diff tests ───────────────────────────────────────────

    fn make_sbs_lines() -> Vec<SideBySideLine> {
        vec![
            SideBySideLine {
                left: Some("fn main() {\n".to_string()),
                right: Some("fn main() {\n".to_string()),
                tag: DiffTag::Equal,
            },
            SideBySideLine {
                left: Some("    old_line();\n".to_string()),
                right: Some("    new_line();\n".to_string()),
                tag: DiffTag::Delete, // modification
            },
            SideBySideLine {
                left: Some("}\n".to_string()),
                right: Some("}\n".to_string()),
                tag: DiffTag::Equal,
            },
        ]
    }

    #[test]
    fn test_sbs_basic() {
        let panel = make_panel();
        let sbs = make_sbs_lines();
        let grid = render_side_by_side(&panel, &sbs, 10, 40, 0);

        assert_eq!(grid.rows, 10);
        assert_eq!(grid.cols, 40);
        assert_eq!(grid.cells.len(), 10 * 40);
    }

    #[test]
    fn test_sbs_title() {
        let panel = make_panel();
        let grid = render_side_by_side(&panel, &[], 5, 40, 0);

        let header: String = grid.cells[..40].iter().map(|c| c.c).collect();
        assert!(header.contains("side-by-side"), "header: '{header}'");
    }

    #[test]
    fn test_sbs_divider() {
        let panel = make_panel();
        let sbs = make_sbs_lines();
        let grid = render_side_by_side(&panel, &sbs, 10, 40, 0);

        // Divider at col 19 (half of 39)
        let div_col = (40 - 1) / 2;
        // Row 1 (first content row)
        let div_cell = &grid.cells[40 + div_col];
        assert_eq!(div_cell.c, '\u{2502}'); // │
    }

    #[test]
    fn test_sbs_equal_both_sides() {
        let panel = make_panel();
        let sbs = make_sbs_lines();
        let grid = render_side_by_side(&panel, &sbs, 10, 40, 0);

        // Row 1: equal line — both sides should have FG_SECONDARY
        let left_cell = &grid.cells[40]; // col 0
        assert_eq!(left_cell.fg, FG_SECONDARY);
    }

    #[test]
    fn test_sbs_modification_colors() {
        let panel = make_panel();
        let sbs = make_sbs_lines();
        let grid = render_side_by_side(&panel, &sbs, 10, 40, 0);

        let div_col = (40 - 1) / 2;
        // Row 2: modification — left DIFF_DEL, right DIFF_ADD
        let left_cell = &grid.cells[2 * 40]; // col 0
        assert_eq!(left_cell.fg, DIFF_DEL);

        let right_cell = &grid.cells[2 * 40 + div_col + 1]; // first right col
        assert_eq!(right_cell.fg, DIFF_ADD);
    }

    #[test]
    fn test_sbs_empty_shows_message() {
        let panel = make_panel();
        let grid = render_side_by_side(&panel, &[], 5, 30, 0);

        let all_text: String = grid.cells.iter().map(|c| c.c).collect();
        assert!(
            all_text.contains("No changes"),
            "expected 'No changes': '{all_text}'"
        );
    }
}
