//! TUI agent selection screen shown when no `--agent` flag is provided.
//!
//! Two-stage picker: first select an agent type, then select a working directory.

use crate::theme::*;
use crate::ui_grid;
use std::path::PathBuf;
use termesh_terminal::grid::{CursorState, GridSnapshot, RenderableCell};

/// ASCII art logo lines.
const LOGO: &[&str] = &[
    r" _                              _      ",
    r"| |_  ___  _ _  _ __   ___  ___| |_    ",
    r"|  _|/ -_)| '_|| '  \ / -_)(_-<| ' \   ",
    r" \__|\___||_|  |_|_|_|\___||__/|_||_|  ",
];

/// Available agent choices.
#[allow(dead_code)]
const AGENTS: &[(&str, &str, &str)] = &[
    ("claude", "Claude", "Anthropic Claude Code"),
    ("codex", "Codex", "OpenAI Codex CLI"),
    ("gemini", "Gemini", "Google Gemini CLI"),
    ("shell", "Shell", "Plain terminal shell"),
];

/// Which stage the picker is currently displaying.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PickerStage {
    /// Choose an agent type.
    SelectAgent,
    /// Choose a working directory.
    SelectFolder,
}

/// State for the agent picker UI.
#[allow(dead_code)]
pub struct AgentPicker {
    /// Current picker stage.
    stage: PickerStage,
    /// Currently selected agent index.
    selected: usize,
    /// Error message displayed when agent is not installed.
    error_message: Option<String>,
    /// Agent type confirmed in stage 1.
    confirmed_agent: Option<String>,
    /// Available folder paths for stage 2.
    folders: Vec<PathBuf>,
    /// Currently selected folder index (includes "Type a path..." as last item).
    folder_selected: usize,
    /// Custom path input buffer (active when user chose "Type a path...").
    custom_input: Option<String>,
}

#[allow(dead_code)]
impl AgentPicker {
    /// Create a new agent picker starting at the agent selection stage.
    pub fn new() -> Self {
        Self {
            stage: PickerStage::SelectAgent,
            selected: 0,
            error_message: None,
            confirmed_agent: None,
            folders: Vec::new(),
            folder_selected: 0,
            custom_input: None,
        }
    }

    /// Set available folder paths for the folder selection stage.
    pub fn set_folders(&mut self, folders: Vec<PathBuf>) {
        self.folders = folders;
    }

    /// Whether the picker is currently in custom path input mode.
    pub fn is_custom_input(&self) -> bool {
        self.custom_input.is_some()
    }

    /// Move selection up (stage-aware).
    pub fn select_prev(&mut self) {
        self.error_message = None;
        match self.stage {
            PickerStage::SelectAgent => {
                if self.selected == 0 {
                    self.selected = AGENTS.len() - 1;
                } else {
                    self.selected -= 1;
                }
            }
            PickerStage::SelectFolder => {
                // folders + 1 for "Type a path..."
                let total = self.folders.len() + 1;
                if self.folder_selected == 0 {
                    self.folder_selected = total - 1;
                } else {
                    self.folder_selected -= 1;
                }
            }
        }
    }

    /// Move selection down (stage-aware).
    pub fn select_next(&mut self) {
        self.error_message = None;
        match self.stage {
            PickerStage::SelectAgent => {
                self.selected = (self.selected + 1) % AGENTS.len();
            }
            PickerStage::SelectFolder => {
                let total = self.folders.len() + 1;
                self.folder_selected = (self.folder_selected + 1) % total;
            }
        }
    }

    /// Confirm selection and return the agent type string (for tests).
    pub fn confirm(&self) -> &'static str {
        AGENTS[self.selected].0
    }

    /// Try to confirm the current stage.
    ///
    /// - **SelectAgent**: validates the agent is installed, then transitions to
    ///   SelectFolder. Returns `None` (picker stays open).
    /// - **SelectFolder**: returns `Some(agent_type)` so the caller can spawn.
    ///   If the "Type a path..." item is selected, activates custom input mode
    ///   and returns `None`.
    pub fn try_confirm(&mut self) -> Option<&str> {
        match self.stage {
            PickerStage::SelectAgent => {
                let agent_type = AGENTS[self.selected].0;
                if agent_type != "shell" && !termesh_core::platform::which(agent_type) {
                    self.error_message = Some(format!(
                        "'{}' is not installed. Install it first.",
                        agent_type
                    ));
                    return None;
                }
                self.error_message = None;
                self.confirmed_agent = Some(agent_type.to_string());
                self.stage = PickerStage::SelectFolder;
                self.folder_selected = 0;
                self.custom_input = None;
                None
            }
            PickerStage::SelectFolder => {
                if let Some(ref input) = self.custom_input {
                    // Custom input mode: Enter confirms the typed path
                    if input.is_empty() {
                        return None;
                    }
                    // Return the confirmed agent type
                    self.confirmed_agent.as_deref()
                } else if self.folder_selected >= self.folders.len() {
                    // "Type a path..." selected → activate custom input
                    self.custom_input = Some(String::new());
                    None
                } else {
                    // Regular folder selected
                    self.confirmed_agent.as_deref()
                }
            }
        }
    }

    /// Get the selected folder path (only valid in SelectFolder stage after confirm).
    pub fn selected_folder(&self) -> Option<PathBuf> {
        if self.stage != PickerStage::SelectFolder {
            return None;
        }
        if let Some(ref input) = self.custom_input {
            if !input.is_empty() {
                return Some(PathBuf::from(input));
            }
            return None;
        }
        self.folders.get(self.folder_selected).cloned()
    }

    /// Go back from SelectFolder to SelectAgent.
    pub fn go_back(&mut self) {
        if self.stage == PickerStage::SelectFolder {
            self.stage = PickerStage::SelectAgent;
            self.confirmed_agent = None;
            self.custom_input = None;
            self.error_message = None;
        }
    }

    /// Cancel custom input mode (return to folder list).
    pub fn cancel_custom_input(&mut self) {
        self.custom_input = None;
    }

    /// Handle a character typed in custom input mode.
    pub fn handle_char_input(&mut self, c: char) {
        if let Some(ref mut input) = self.custom_input {
            input.push(c);
        }
    }

    /// Handle backspace in custom input mode.
    pub fn handle_backspace(&mut self) {
        if let Some(ref mut input) = self.custom_input {
            input.pop();
        }
    }

    /// Whether we are in the folder selection stage.
    pub fn is_folder_stage(&self) -> bool {
        self.stage == PickerStage::SelectFolder
    }

    /// Render the picker into a `GridSnapshot`.
    pub fn render(&self, rows: usize, cols: usize) -> GridSnapshot {
        match self.stage {
            PickerStage::SelectAgent => self.render_agent_stage(rows, cols),
            PickerStage::SelectFolder => self.render_folder_stage(rows, cols),
        }
    }

    /// Render logo rows starting at `start_row`, centered as a block.
    fn render_logo(cells: &mut Vec<RenderableCell>, start_row: usize, cols: usize) {
        // Center all lines as a block using the widest content width.
        let max_width = LOGO
            .iter()
            .map(|line| line.trim_end().len())
            .max()
            .unwrap_or(0);
        let pad = cols.saturating_sub(max_width) / 2;
        for (i, line) in LOGO.iter().enumerate() {
            let trimmed = line.trim_end();
            let chars: Vec<char> = trimmed.chars().collect();
            for col in 0..cols {
                let ch_idx = col.wrapping_sub(pad);
                let c = if col >= pad && ch_idx < chars.len() {
                    chars[ch_idx]
                } else {
                    ' '
                };
                cells.push(RenderableCell {
                    row: start_row + i,
                    col,
                    c,
                    fg: ACCENT,
                    bg: BG_SURFACE,
                    ..Default::default()
                });
            }
        }
    }

    /// Render the agent selection stage.
    fn render_agent_stage(&self, rows: usize, cols: usize) -> GridSnapshot {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let mut cells = Vec::with_capacity(rows * cols);

        let has_error = self.error_message.is_some();
        let logo_height = LOGO.len();
        let content_height =
            logo_height + 1 + 2 + AGENTS.len() + 1 + (if has_error { 1 } else { 0 }) + 1;
        let start_row = if rows > content_height {
            (rows - content_height) / 2
        } else {
            0
        };
        let body_start = logo_height + 1;

        // Render LOGO as a block first
        Self::render_logo(&mut cells, start_row, cols);

        for row in 0..rows {
            let rel = row.wrapping_sub(start_row);

            if rel < logo_height {
                continue; // Already rendered by render_logo
            } else if rel == body_start {
                ui_grid::push_centered_row(
                    &mut cells,
                    row,
                    cols,
                    "Select Agent",
                    FG_PRIMARY,
                    BG_SURFACE,
                );
            } else if rel == body_start + 1 {
                ui_grid::push_centered_row(
                    &mut cells,
                    row,
                    cols,
                    "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                    FG_MUTED,
                    BG_SURFACE,
                );
            } else if rel >= body_start + 2 && rel < body_start + 2 + AGENTS.len() {
                let idx = rel - (body_start + 2);
                let is_selected = idx == self.selected;
                let (_, name, desc) = AGENTS[idx];
                let line = if is_selected {
                    format!("  \u{25B8} {name}  {desc}")
                } else {
                    format!("    {name}  {desc}")
                };
                let bg = if is_selected { BG_SELECTED } else { BG_SURFACE };
                ui_grid::push_centered_row(&mut cells, row, cols, &line, FG_PRIMARY, bg);
            } else if has_error && rel == body_start + 2 + AGENTS.len() {
                let err = self.error_message.as_deref().unwrap_or("");
                ui_grid::push_centered_row(&mut cells, row, cols, err, STATUS_ERROR, BG_SURFACE);
            } else if rel == body_start + 2 + AGENTS.len() + (if has_error { 2 } else { 1 }) {
                ui_grid::push_centered_row(
                    &mut cells,
                    row,
                    cols,
                    "Arrow keys to select, Enter to confirm",
                    FG_MUTED,
                    BG_SURFACE,
                );
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

    /// Render the folder selection stage.
    fn render_folder_stage(&self, rows: usize, cols: usize) -> GridSnapshot {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let mut cells = Vec::with_capacity(rows * cols);

        let logo_height = LOGO.len();
        // folders + "Type a path..." + optional custom input line
        let item_count = self.folders.len() + 1;
        let has_custom = self.custom_input.is_some();
        // logo + blank + title + separator + items + (custom_input?) + blank + hint
        let content_height =
            logo_height + 1 + 2 + item_count + (if has_custom { 1 } else { 0 }) + 1 + 1;
        let start_row = if rows > content_height {
            (rows - content_height) / 2
        } else {
            0
        };
        let body_start = logo_height + 1;

        // Render LOGO as a block first
        Self::render_logo(&mut cells, start_row, cols);

        for row in 0..rows {
            let rel = row.wrapping_sub(start_row);

            if rel < logo_height {
                continue; // Already rendered by render_logo
            } else if rel == body_start {
                ui_grid::push_centered_row(
                    &mut cells,
                    row,
                    cols,
                    "Select working directory",
                    FG_PRIMARY,
                    BG_SURFACE,
                );
            } else if rel == body_start + 1 {
                ui_grid::push_centered_row(
                    &mut cells,
                    row,
                    cols,
                    "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                    FG_MUTED,
                    BG_SURFACE,
                );
            } else if rel >= body_start + 2 && rel < body_start + 2 + item_count {
                let idx = rel - (body_start + 2);
                let is_selected = idx == self.folder_selected && !has_custom;

                let line = if idx < self.folders.len() {
                    let path = self.folders[idx].display().to_string();
                    let suffix = if idx == 0 { "  (current)" } else { "" };
                    if is_selected {
                        format!("  \u{25B8} {path}{suffix}")
                    } else {
                        format!("    {path}{suffix}")
                    }
                } else {
                    // "Type a path..." item
                    if is_selected {
                        "  \u{25B8} [ Type a path... ]".to_string()
                    } else {
                        "    [ Type a path... ]".to_string()
                    }
                };

                let bg = if is_selected { BG_SELECTED } else { BG_SURFACE };
                ui_grid::push_centered_row(&mut cells, row, cols, &line, FG_PRIMARY, bg);
            } else if has_custom && rel == body_start + 2 + item_count {
                // Custom input line
                let input = self.custom_input.as_deref().unwrap_or("");
                let line = format!("  > {input}\u{2588}");
                ui_grid::push_centered_row(&mut cells, row, cols, &line, ACCENT, BG_SURFACE);
            } else if rel == body_start + 2 + item_count + (if has_custom { 1 } else { 0 }) + 1 {
                let hint = if has_custom {
                    "Type a path, Enter to confirm, Esc to cancel"
                } else {
                    "Arrow keys to select, Enter to confirm, Esc to go back"
                };
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

    #[test]
    fn test_picker_stage_transition() {
        let mut picker = AgentPicker::new();
        picker.set_folders(vec![PathBuf::from("/tmp")]);
        // Select "shell" (no install check needed)
        picker.selected = 3;
        // Confirm agent → transitions to SelectFolder, returns None
        assert!(picker.try_confirm().is_none());
        assert!(picker.is_folder_stage());
        assert_eq!(picker.confirmed_agent.as_deref(), Some("shell"));
    }

    #[test]
    fn test_picker_folder_selection() {
        let mut picker = AgentPicker::new();
        let folders = vec![PathBuf::from("/home/user/project")];
        picker.set_folders(folders.clone());
        picker.selected = 3; // shell
        picker.try_confirm(); // → SelectFolder

        // First item is the folder
        assert_eq!(
            picker.selected_folder(),
            Some(PathBuf::from("/home/user/project"))
        );

        // Confirm folder → returns agent type
        assert_eq!(picker.try_confirm(), Some("shell"));
    }

    #[test]
    fn test_picker_custom_input() {
        let mut picker = AgentPicker::new();
        picker.set_folders(vec![PathBuf::from("/tmp")]);
        picker.selected = 3; // shell
        picker.try_confirm(); // → SelectFolder

        // Select "Type a path..." (index 1 = after 1 folder)
        picker.folder_selected = 1;
        assert!(picker.try_confirm().is_none()); // activates custom input
        assert!(picker.is_custom_input());

        picker.handle_char_input('/');
        picker.handle_char_input('o');
        picker.handle_char_input('p');
        picker.handle_char_input('t');
        assert_eq!(picker.selected_folder(), Some(PathBuf::from("/opt")));

        picker.handle_backspace();
        assert_eq!(picker.selected_folder(), Some(PathBuf::from("/op")));
    }

    #[test]
    fn test_picker_go_back() {
        let mut picker = AgentPicker::new();
        picker.set_folders(vec![PathBuf::from("/tmp")]);
        picker.selected = 3;
        picker.try_confirm(); // → SelectFolder
        assert!(picker.is_folder_stage());

        picker.go_back();
        assert!(!picker.is_folder_stage());
        assert!(picker.confirmed_agent.is_none());
    }

    #[test]
    fn test_picker_folder_render_dimensions() {
        let mut picker = AgentPicker::new();
        picker.set_folders(vec![PathBuf::from("/tmp")]);
        picker.selected = 3;
        picker.try_confirm(); // → SelectFolder

        let grid = picker.render(20, 60);
        assert_eq!(grid.rows, 20);
        assert_eq!(grid.cols, 60);
        assert_eq!(grid.cells.len(), 20 * 60);
    }

    #[test]
    fn test_picker_folder_navigate() {
        let mut picker = AgentPicker::new();
        picker.set_folders(vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        picker.selected = 3;
        picker.try_confirm(); // → SelectFolder

        assert_eq!(picker.folder_selected, 0);
        picker.select_next();
        assert_eq!(picker.folder_selected, 1);
        picker.select_next();
        assert_eq!(picker.folder_selected, 2); // "Type a path..."
        picker.select_next();
        assert_eq!(picker.folder_selected, 0); // wrap
        picker.select_prev();
        assert_eq!(picker.folder_selected, 2); // wrap back
    }
}
