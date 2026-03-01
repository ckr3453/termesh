//! TUI agent selection screen shown when no `--agent` flag is provided.
//!
//! The `AgentPicker` struct is used when the event loop supports picker mode.
//! Currently only `resolve_agent` is called from `main.rs`.

use crate::theme::*;
use termesh_terminal::grid::{CursorState, GridSnapshot, RenderableCell};

/// ASCII art logo lines.
const LOGO: &[&str] = &[
    r" _                              _     ",
    r"| |_ ___ _ __ _ __ ___   ___  | |__  ",
    r"| __/ _ \ '__| '_ ` _ \ / _ \ | '_ \ ",
    r"| ||  __/ |  | | | | | |  __/_| | | |",
    r" \__\___|_|  |_| |_| |_|\___(_)_| |_|",
];

/// Available agent choices.
#[allow(dead_code)]
const AGENTS: &[(&str, &str, &str)] = &[
    ("claude", "Claude", "Anthropic Claude Code"),
    ("codex", "Codex", "OpenAI Codex CLI"),
    ("gemini", "Gemini", "Google Gemini CLI"),
    ("shell", "Shell", "Plain terminal shell"),
];

/// State for the agent picker UI.
#[allow(dead_code)]
pub struct AgentPicker {
    /// Currently selected index.
    selected: usize,
    /// Error message displayed when agent is not installed.
    error_message: Option<String>,
}

#[allow(dead_code)]
impl AgentPicker {
    /// Create a new agent picker.
    pub fn new() -> Self {
        Self {
            selected: 0,
            error_message: None,
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        self.error_message = None;
        if self.selected == 0 {
            self.selected = AGENTS.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        self.error_message = None;
        self.selected = (self.selected + 1) % AGENTS.len();
    }

    /// Confirm selection and return the agent type string.
    pub fn confirm(&self) -> &'static str {
        AGENTS[self.selected].0
    }

    /// Try to confirm the selection, checking if the agent CLI is installed.
    ///
    /// Returns `Some(agent_type)` if installed (or "shell"), `None` if not
    /// installed (sets `error_message` for display).
    pub fn try_confirm(&mut self) -> Option<&'static str> {
        let agent_type = AGENTS[self.selected].0;
        if agent_type == "shell" {
            self.error_message = None;
            return Some(agent_type);
        }
        if termesh_core::platform::which(agent_type) {
            self.error_message = None;
            Some(agent_type)
        } else {
            self.error_message = Some(format!(
                "'{}' is not installed. Install it first.",
                agent_type
            ));
            None
        }
    }

    /// Render the picker into a `GridSnapshot`.
    pub fn render(&self, rows: usize, cols: usize) -> GridSnapshot {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let mut cells = Vec::with_capacity(rows * cols);

        // Vertical centering
        let has_error = self.error_message.is_some();
        let logo_height = LOGO.len();
        // logo + blank + title + separator + options + (error?) + blank + hint
        let content_height =
            logo_height + 1 + 2 + AGENTS.len() + 1 + (if has_error { 1 } else { 0 }) + 1;
        let start_row = if rows > content_height {
            (rows - content_height) / 2
        } else {
            0
        };

        // Offset after logo block
        let body_start = logo_height + 1; // logo lines + 1 blank

        for row in 0..rows {
            let rel = row.wrapping_sub(start_row);

            if rel < logo_height {
                // Logo rows
                let line = LOGO[rel];
                let line_chars: Vec<char> = line.chars().collect();
                let pad = cols.saturating_sub(line_chars.len()) / 2;
                for col in 0..cols {
                    let ch_idx = col.wrapping_sub(pad);
                    let c = if col >= pad && ch_idx < line_chars.len() {
                        line_chars[ch_idx]
                    } else {
                        ' '
                    };
                    cells.push(RenderableCell {
                        row,
                        col,
                        c,
                        fg: ACCENT,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                }
            } else if rel == body_start {
                // Title row
                let title = "Select Agent";
                let pad = cols.saturating_sub(title.len()) / 2;
                for col in 0..cols {
                    let ch_idx = col.wrapping_sub(pad);
                    let c = if col >= pad && ch_idx < title.len() {
                        title.as_bytes()[ch_idx] as char
                    } else {
                        ' '
                    };
                    cells.push(RenderableCell {
                        row,
                        col,
                        c,
                        fg: FG_PRIMARY,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                }
            } else if rel == body_start + 1 {
                // Separator
                let sep = "────────────────────";
                let pad = cols.saturating_sub(sep.chars().count()) / 2;
                let sep_chars: Vec<char> = sep.chars().collect();
                for col in 0..cols {
                    let ch_idx = col.wrapping_sub(pad);
                    let c = if col >= pad && ch_idx < sep_chars.len() {
                        sep_chars[ch_idx]
                    } else {
                        ' '
                    };
                    cells.push(RenderableCell {
                        row,
                        col,
                        c,
                        fg: FG_MUTED,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                }
            } else if rel >= body_start + 2 && rel < body_start + 2 + AGENTS.len() {
                // Agent option
                let idx = rel - (body_start + 2);
                let is_selected = idx == self.selected;
                let (_, name, desc) = AGENTS[idx];
                let line = if is_selected {
                    format!("  \u{25B8} {name}  {desc}")
                } else {
                    format!("    {name}  {desc}")
                };
                let bg = if is_selected { BG_SELECTED } else { BG_SURFACE };
                let pad = cols.saturating_sub(line.chars().count()) / 2;
                let line_chars: Vec<char> = line.chars().collect();

                for col in 0..cols {
                    let ch_idx = col.wrapping_sub(pad);
                    let c = if col >= pad && ch_idx < line_chars.len() {
                        line_chars[ch_idx]
                    } else {
                        ' '
                    };
                    cells.push(RenderableCell {
                        row,
                        col,
                        c,
                        fg: FG_PRIMARY,
                        bg,
                        ..Default::default()
                    });
                }
            } else if has_error && rel == body_start + 2 + AGENTS.len() {
                // Error message row
                let err = self.error_message.as_deref().unwrap_or("");
                let pad = cols.saturating_sub(err.len()) / 2;
                for col in 0..cols {
                    let ch_idx = col.wrapping_sub(pad);
                    let c = if col >= pad && ch_idx < err.len() {
                        err.as_bytes().get(ch_idx).copied().unwrap_or(b' ') as char
                    } else {
                        ' '
                    };
                    cells.push(RenderableCell {
                        row,
                        col,
                        c,
                        fg: STATUS_ERROR,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                }
            } else if rel == body_start + 2 + AGENTS.len() + (if has_error { 2 } else { 1 }) {
                // Hint row
                let hint = "Arrow keys to select, Enter to confirm";
                let pad = cols.saturating_sub(hint.len()) / 2;
                for col in 0..cols {
                    let ch_idx = col.wrapping_sub(pad);
                    let c = if col >= pad && ch_idx < hint.len() {
                        hint.as_bytes()[ch_idx] as char
                    } else {
                        ' '
                    };
                    cells.push(RenderableCell {
                        row,
                        col,
                        c,
                        fg: FG_MUTED,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                }
            } else {
                // Empty row
                for col in 0..cols {
                    cells.push(RenderableCell {
                        row,
                        col,
                        c: ' ',
                        fg: FG_MUTED,
                        bg: BG_SURFACE,
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
}

/// Wrap a command for execution on Windows.
///
/// On Windows, npm-installed CLI tools (claude, codex, gemini) are `.cmd` files
/// that cannot be spawned directly via `CreateProcessW`. We wrap them with
/// `cmd.exe /c <command>` to execute them through the shell.
fn wrap_command(cmd: &str) -> (String, Vec<String>) {
    if cfg!(windows) {
        (
            "cmd.exe".to_string(),
            vec!["/c".to_string(), cmd.to_string()],
        )
    } else {
        (cmd.to_string(), Vec::new())
    }
}

/// Generate a short unique session ID (8 hex chars).
///
/// Uses an atomic counter mixed with timestamp to guarantee uniqueness
/// even across rapid successive calls.
fn random_session_label() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let mixed = nanos.wrapping_mul(0x9E37_79B9) ^ count.wrapping_mul(0x517C_C1B7);
    format!("{:08x}", mixed)
}

/// Resolve an agent type string to a command and display label.
///
/// Returns `(command, args, label, agent_kind, is_agent)`.
/// `agent_kind` is a short type name like "claude", "codex", "gemini", "shell".
pub fn resolve_agent(agent_type: &str) -> (String, Vec<String>, String, String, bool) {
    let label = random_session_label();
    match agent_type {
        "claude" => {
            let (cmd, args) = wrap_command("claude");
            (cmd, args, label, "claude".to_string(), true)
        }
        "codex" => {
            let (cmd, args) = wrap_command("codex");
            (cmd, args, label, "codex".to_string(), true)
        }
        "gemini" => {
            let (cmd, args) = wrap_command("gemini");
            (cmd, args, label, "gemini".to_string(), true)
        }
        _ => (
            termesh_core::platform::default_shell(),
            Vec::new(),
            label,
            "shell".to_string(),
            false,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_picker_default_selection() {
        let picker = AgentPicker::new();
        assert_eq!(picker.confirm(), "claude");
    }

    #[test]
    fn test_picker_navigate() {
        let mut picker = AgentPicker::new();
        picker.select_next();
        assert_eq!(picker.confirm(), "codex");
        picker.select_next();
        assert_eq!(picker.confirm(), "gemini");
        picker.select_next();
        assert_eq!(picker.confirm(), "shell");
        // Wrap around
        picker.select_next();
        assert_eq!(picker.confirm(), "claude");
    }

    #[test]
    fn test_picker_navigate_prev() {
        let mut picker = AgentPicker::new();
        picker.select_prev();
        assert_eq!(picker.confirm(), "shell");
    }

    #[test]
    fn test_picker_render_dimensions() {
        let picker = AgentPicker::new();
        let grid = picker.render(20, 60);
        assert_eq!(grid.rows, 20);
        assert_eq!(grid.cols, 60);
        assert_eq!(grid.cells.len(), 20 * 60);
        assert!(!grid.cursor.visible);
    }

    #[test]
    fn test_resolve_agent_claude() {
        let (cmd, args, _label, agent_kind, is_agent) = resolve_agent("claude");
        if cfg!(windows) {
            assert_eq!(cmd, "cmd.exe");
            assert_eq!(args, vec!["/c", "claude"]);
        } else {
            assert_eq!(cmd, "claude");
            assert!(args.is_empty());
        }
        assert_eq!(agent_kind, "claude");
        assert!(is_agent);
        // Label should be a random hex string
        assert_eq!(_label.len(), 8);
    }

    #[test]
    fn test_resolve_agent_shell() {
        let (_cmd, _args, _label, agent_kind, is_agent) = resolve_agent("shell");
        assert_eq!(agent_kind, "shell");
        assert!(!is_agent);
    }

    #[test]
    fn test_resolve_agent_unknown() {
        let (_cmd, _args, _label, agent_kind, is_agent) = resolve_agent("unknown");
        assert_eq!(agent_kind, "shell");
        assert!(!is_agent);
    }
}
