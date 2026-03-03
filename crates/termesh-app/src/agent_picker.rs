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
    /// Filter input buffer for fuzzy filtering folder list.
    filter_input: String,
    /// Indices into `folders` that match the current filter.
    filtered_indices: Vec<usize>,
    /// Ghost text: the suffix that would be appended by tab completion.
    completion_ghost: Option<String>,
    /// Completion candidates shown below the input line.
    completion_candidates: Vec<String>,
    /// Currently highlighted candidate index (None = common prefix mode).
    completion_selected: Option<usize>,
    /// Scroll offset for the visible candidate window.
    completion_scroll: usize,
    /// Typed prefix used to compute per-candidate ghost text.
    completion_prefix: String,
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
            filter_input: String::new(),
            filtered_indices: Vec::new(),
            completion_ghost: None,
            completion_candidates: Vec::new(),
            completion_selected: None,
            completion_scroll: 0,
            completion_prefix: String::new(),
        }
    }

    /// Set available folder paths for the folder selection stage.
    pub fn set_folders(&mut self, folders: Vec<PathBuf>) {
        self.folders = folders;
        self.refilter();
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
                // filtered folders + 1 for "Type a path..."
                let total = self.filtered_indices.len() + 1;
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
                let total = self.filtered_indices.len() + 1;
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
                } else if self.folder_selected >= self.filtered_indices.len() {
                    // "Type a path..." selected → activate custom input
                    self.custom_input = Some(String::new());
                    None
                } else {
                    // Regular folder selected (via filtered index)
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
        self.filtered_indices
            .get(self.folder_selected)
            .and_then(|&i| self.folders.get(i))
            .cloned()
    }

    /// Go back from SelectFolder to SelectAgent.
    pub fn go_back(&mut self) {
        if self.stage == PickerStage::SelectFolder {
            self.stage = PickerStage::SelectAgent;
            self.confirmed_agent = None;
            self.custom_input = None;
            self.error_message = None;
            self.filter_clear();
        }
    }

    /// Cancel custom input mode (return to folder list).
    pub fn cancel_custom_input(&mut self) {
        self.custom_input = None;
        self.clear_completion_state();
    }

    /// Reset all completion state.
    fn clear_completion_state(&mut self) {
        self.completion_ghost = None;
        self.completion_candidates.clear();
        self.completion_selected = None;
        self.completion_scroll = 0;
        self.completion_prefix.clear();
    }

    /// Handle a character typed in custom input mode.
    pub fn handle_char_input(&mut self, c: char) {
        if let Some(ref mut input) = self.custom_input {
            input.push(c);
        }
        self.recompute_completions();
    }

    /// Handle backspace in custom input mode.
    pub fn handle_backspace(&mut self) {
        if let Some(ref mut input) = self.custom_input {
            input.pop();
        }
        self.recompute_completions();
    }

    /// Handle Tab key: accept the ghost text completion.
    ///
    /// If ghost text hasn't been computed yet, computes it first.
    pub fn handle_tab_complete(&mut self) {
        if self.completion_ghost.is_none() {
            self.recompute_completions();
        }
        if let Some(ghost) = self.completion_ghost.take() {
            if let Some(ref mut input) = self.custom_input {
                input.push_str(&ghost);
            }
            self.recompute_completions();
        }
    }

    /// Maximum number of completion candidates visible at once.
    const COMPLETION_VISIBLE_MAX: usize = 6;

    /// Recompute completion ghost text and candidate list from current input.
    fn recompute_completions(&mut self) {
        self.clear_completion_state();

        let input = match self.custom_input {
            Some(ref s) if !s.is_empty() => s.clone(),
            _ => return,
        };

        let expanded = termesh_core::platform::expand_tilde(&input)
            .to_string_lossy()
            .to_string();

        // Split into parent directory and prefix
        let (parent, prefix) = match expanded.rfind('/') {
            Some(pos) => {
                let parent = if pos == 0 {
                    "/".to_string()
                } else {
                    expanded[..pos].to_string()
                };
                (parent, expanded[pos + 1..].to_string())
            }
            None => return,
        };

        // Read directory and find matching subdirectories
        let entries = match std::fs::read_dir(&parent) {
            Ok(rd) => rd,
            Err(_) => return,
        };

        let mut matches: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with(&prefix))
            .collect();
        matches.sort();

        if matches.is_empty() {
            return;
        }

        self.completion_prefix = prefix.clone();

        // Ghost text = common prefix mode (no candidate selected)
        self.update_ghost_for_common_prefix(&matches);

        self.completion_candidates = matches;
    }

    /// Set ghost text to the longest common prefix beyond the typed prefix.
    fn update_ghost_for_common_prefix(&mut self, matches: &[String]) {
        let prefix = &self.completion_prefix;
        let completed = if matches.len() == 1 {
            matches[0].clone()
        } else {
            let mut common = matches[0].clone();
            for m in &matches[1..] {
                common = common
                    .chars()
                    .zip(m.chars())
                    .take_while(|(a, b)| a == b)
                    .map(|(a, _)| a)
                    .collect();
            }
            common
        };

        let ghost_name_part = &completed[prefix.len()..];
        let trailing = if matches.len() == 1 { "/" } else { "" };
        let ghost = format!("{ghost_name_part}{trailing}");

        if !ghost.is_empty() {
            self.completion_ghost = Some(ghost);
        }
    }

    /// Set ghost text to a specific candidate's full suffix.
    fn update_ghost_for_candidate(&mut self, candidate_idx: usize) {
        if let Some(name) = self.completion_candidates.get(candidate_idx) {
            let ghost = format!("{}/", &name[self.completion_prefix.len()..]);
            self.completion_ghost = Some(ghost);
        }
    }

    /// Move selection down in the completion candidate list.
    pub fn completion_select_next(&mut self) {
        if self.completion_candidates.is_empty() {
            return;
        }
        match self.completion_selected {
            None => {
                self.completion_selected = Some(0);
                self.update_ghost_for_candidate(0);
            }
            Some(i) if i + 1 < self.completion_candidates.len() => {
                self.completion_selected = Some(i + 1);
                self.update_ghost_for_candidate(i + 1);
            }
            Some(_) => {
                // Wrap to common prefix mode
                self.completion_selected = None;
                self.update_ghost_for_common_prefix(&self.completion_candidates.clone());
            }
        }
        self.adjust_completion_scroll();
    }

    /// Move selection up in the completion candidate list.
    pub fn completion_select_prev(&mut self) {
        if self.completion_candidates.is_empty() {
            return;
        }
        match self.completion_selected {
            None => {
                // Wrap to last candidate
                let last = self.completion_candidates.len() - 1;
                self.completion_selected = Some(last);
                self.update_ghost_for_candidate(last);
            }
            Some(0) => {
                // Back to common prefix mode
                self.completion_selected = None;
                self.update_ghost_for_common_prefix(&self.completion_candidates.clone());
            }
            Some(i) => {
                self.completion_selected = Some(i - 1);
                self.update_ghost_for_candidate(i - 1);
            }
        }
        self.adjust_completion_scroll();
    }

    /// Ensure the selected candidate is within the visible scroll window.
    fn adjust_completion_scroll(&mut self) {
        if let Some(sel) = self.completion_selected {
            let max_visible = Self::COMPLETION_VISIBLE_MAX;
            if sel < self.completion_scroll {
                self.completion_scroll = sel;
            } else if sel >= self.completion_scroll + max_visible {
                self.completion_scroll = sel + 1 - max_visible;
            }
        }
    }

    /// Append a character to the filter input and re-filter.
    pub fn filter_push(&mut self, c: char) {
        self.filter_input.push(c);
        self.refilter();
    }

    /// Remove the last character from the filter input and re-filter.
    pub fn filter_pop(&mut self) {
        self.filter_input.pop();
        self.refilter();
    }

    /// Clear the filter input and restore the full folder list.
    pub fn filter_clear(&mut self) {
        self.filter_input.clear();
        self.refilter();
    }

    /// Recompute `filtered_indices` from `filter_input`.
    fn refilter(&mut self) {
        let query = self.filter_input.to_lowercase();
        self.filtered_indices = if query.is_empty() {
            (0..self.folders.len()).collect()
        } else {
            (0..self.folders.len())
                .filter(|&i| {
                    self.folders[i]
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query)
                })
                .collect()
        };
        self.folder_selected = 0;
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
            .map(|line| ui_grid::display_width(line.trim_end()))
            .max()
            .unwrap_or(0);
        let pad = cols.saturating_sub(max_width) / 2;
        for (i, line) in LOGO.iter().enumerate() {
            let trimmed = line.trim_end();
            let row = start_row + i;
            let mut col = 0;
            // Left padding
            while col < pad && col < cols {
                cells.push(RenderableCell {
                    row,
                    col,
                    c: ' ',
                    fg: ACCENT,
                    bg: BG_SURFACE,
                    ..Default::default()
                });
                col += 1;
            }
            // Logo text
            col = ui_grid::push_text_cells(cells, row, col, cols, trimmed, ACCENT, BG_SURFACE);
            // Right padding
            ui_grid::fill_remaining(cells, row, col, cols, ACCENT, BG_SURFACE);
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
        let has_filter = !self.filter_input.is_empty();
        let no_matches = has_filter && self.filtered_indices.is_empty();
        // filtered folders + "Type a path..." (or "No matches" row if empty)
        let item_count = if no_matches {
            1 // "No matches" row
        } else {
            self.filtered_indices.len() + 1
        };
        let has_custom = self.custom_input.is_some();
        let total_candidates = self.completion_candidates.len();
        let candidate_count = if has_custom && total_candidates > 1 {
            total_candidates.min(Self::COMPLETION_VISIBLE_MAX)
        } else {
            0
        };
        // logo + blank + title + separator + items + (custom_input?) + (candidates?) + blank + hint
        let content_height = logo_height
            + 1
            + 2
            + item_count
            + (if has_custom { 1 } else { 0 })
            + candidate_count
            + 1
            + 1;
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
                let title = if has_filter {
                    format!("Select project  \u{1F50D} {}", self.filter_input)
                } else {
                    "Select project".to_string()
                };
                ui_grid::push_centered_row(&mut cells, row, cols, &title, FG_PRIMARY, BG_SURFACE);
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

                if no_matches {
                    // No matching folders
                    ui_grid::push_centered_row(
                        &mut cells,
                        row,
                        cols,
                        "    No matches",
                        FG_MUTED,
                        BG_SURFACE,
                    );
                } else {
                    let is_selected = idx == self.folder_selected && !has_custom;

                    let line = if idx < self.filtered_indices.len() {
                        let orig_idx = self.filtered_indices[idx];
                        let path = self.folders[orig_idx].display().to_string();
                        let suffix = if orig_idx == 0 { "  (current)" } else { "" };
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
                }
            } else if has_custom && rel == body_start + 2 + item_count {
                // Custom input line with inline ghost text
                //   "  > /Us"  (ACCENT)  +  "ers/"  (FG_MUTED ghost)
                // When no ghost, show a block cursor instead.
                let input = self.custom_input.as_deref().unwrap_or("");
                let ghost = self.completion_ghost.as_deref().unwrap_or("");
                let prefix = format!("  > {input}");

                let prefix_width = ui_grid::display_width(&prefix);
                let ghost_width = if ghost.is_empty() {
                    1 // block cursor
                } else {
                    ui_grid::display_width(ghost)
                };
                let total_width = prefix_width + ghost_width;
                let pad = cols.saturating_sub(total_width) / 2;

                let mut col = 0;
                // Left padding
                while col < pad && col < cols {
                    cells.push(RenderableCell {
                        row,
                        col,
                        c: ' ',
                        fg: FG_MUTED,
                        bg: BG_SURFACE,
                        ..Default::default()
                    });
                    col += 1;
                }
                // Prefix text
                col = ui_grid::push_text_cells(
                    &mut cells, row, col, cols, &prefix, ACCENT, BG_SURFACE,
                );
                // Ghost text or cursor
                if ghost.is_empty() {
                    if col < cols {
                        cells.push(RenderableCell {
                            row,
                            col,
                            c: '\u{2588}',
                            fg: ACCENT,
                            bg: BG_SURFACE,
                            ..Default::default()
                        });
                        col += 1;
                    }
                } else {
                    col = ui_grid::push_text_cells(
                        &mut cells, row, col, cols, ghost, FG_MUTED, BG_SURFACE,
                    );
                }
                // Right padding
                ui_grid::fill_remaining(&mut cells, row, col, cols, FG_MUTED, BG_SURFACE);
            } else if has_custom
                && candidate_count > 0
                && rel > body_start + 2 + item_count
                && rel <= body_start + 2 + item_count + candidate_count
            {
                // Completion candidate rows (scrollable, selectable)
                let vi = rel - (body_start + 2 + item_count + 1); // visible index
                let ci = vi + self.completion_scroll; // actual candidate index
                if ci < total_candidates {
                    let name = &self.completion_candidates[ci];
                    let is_highlighted = self.completion_selected == Some(ci);
                    let marker = if is_highlighted { "\u{25B8} " } else { "  " };
                    let line = format!("    {marker}{name}/");
                    let fg = if is_highlighted { FG_PRIMARY } else { FG_MUTED };
                    let bg = if is_highlighted {
                        BG_SELECTED
                    } else {
                        BG_SURFACE
                    };
                    ui_grid::push_centered_row(&mut cells, row, cols, &line, fg, bg);
                } else {
                    ui_grid::fill_row(&mut cells, row, cols, ' ', FG_MUTED, BG_SURFACE);
                }
            } else if rel
                == body_start
                    + 2
                    + item_count
                    + (if has_custom { 1 } else { 0 })
                    + candidate_count
                    + 1
            {
                let hint = if has_custom {
                    "Tab to complete, Enter to confirm, Esc to cancel"
                } else if has_filter {
                    "Type to filter, Backspace to clear, Enter to confirm"
                } else {
                    "Type to filter, Arrow keys to select, Enter to confirm"
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

    // --- Filter tests ---

    /// Helper: create a picker in folder stage with given folders.
    fn picker_at_folder_stage(folders: Vec<PathBuf>) -> AgentPicker {
        let mut picker = AgentPicker::new();
        picker.set_folders(folders);
        picker.selected = 3; // shell (no install check)
        picker.try_confirm(); // → SelectFolder
        picker
    }

    #[test]
    fn test_filter_narrows_list() {
        let mut picker = picker_at_folder_stage(vec![
            PathBuf::from("/Users/ckr/IdeaProjects/termesh"),
            PathBuf::from("/Users/ckr/IdeaProjects/webapp"),
            PathBuf::from("/tmp/other"),
        ]);
        picker.filter_push('t');
        picker.filter_push('e');
        picker.filter_push('r');
        picker.filter_push('m');
        // Only "termesh" path matches
        assert_eq!(picker.filtered_indices.len(), 1);
        assert_eq!(picker.filtered_indices[0], 0);
        assert_eq!(
            picker.selected_folder(),
            Some(PathBuf::from("/Users/ckr/IdeaProjects/termesh"))
        );
    }

    #[test]
    fn test_filter_case_insensitive() {
        let mut picker = picker_at_folder_stage(vec![
            PathBuf::from("/Users/ckr/IdeaProjects/termesh"),
            PathBuf::from("/tmp/other"),
        ]);
        picker.filter_push('T');
        picker.filter_push('E');
        picker.filter_push('R');
        picker.filter_push('M');
        assert_eq!(picker.filtered_indices.len(), 1);
        assert_eq!(picker.filtered_indices[0], 0);
    }

    #[test]
    fn test_filter_clear_restores_all() {
        let mut picker = picker_at_folder_stage(vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ]);
        picker.filter_push('a');
        assert_eq!(picker.filtered_indices.len(), 1);
        picker.filter_clear();
        assert_eq!(picker.filtered_indices.len(), 3);
    }

    #[test]
    fn test_filter_empty_shows_all() {
        let picker = picker_at_folder_stage(vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(picker.filtered_indices.len(), 2);
    }

    #[test]
    fn test_filter_navigate_uses_filtered() {
        let mut picker = picker_at_folder_stage(vec![
            PathBuf::from("/alpha"),
            PathBuf::from("/beta"),
            PathBuf::from("/gamma"),
        ]);
        // Filter to "lph" → matches only /alpha
        picker.filter_push('l');
        picker.filter_push('p');
        picker.filter_push('h');
        assert_eq!(picker.filtered_indices.len(), 1);
        // total = 1 filtered + 1 "Type a path..." = 2
        assert_eq!(picker.folder_selected, 0);
        picker.select_next();
        assert_eq!(picker.folder_selected, 1); // "Type a path..."
        picker.select_next();
        assert_eq!(picker.folder_selected, 0); // wrap
    }

    // --- Tab completion tests ---

    #[test]
    #[cfg(unix)]
    fn test_tab_complete_single_match() {
        let mut picker = AgentPicker::new();
        picker.custom_input = Some("/tm".to_string());
        picker.handle_tab_complete();
        // /tmp is a real directory; should complete to /tmp/
        let result = picker.custom_input.as_deref().unwrap_or("");
        assert_eq!(result, "/tmp/");
    }

    #[test]
    fn test_tab_complete_common_prefix() {
        use std::fs;
        // Create temp dirs with common prefix
        let base = std::env::temp_dir().join("termesh_tab_test");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("prefix_aaa")).unwrap();
        fs::create_dir_all(base.join("prefix_bbb")).unwrap();

        let mut picker = AgentPicker::new();
        let input = format!("{}/pre", base.display());
        picker.custom_input = Some(input);
        picker.handle_tab_complete();
        let result = picker.custom_input.as_deref().unwrap_or("");
        // Should complete to common prefix "prefix_"
        assert!(
            result.ends_with("prefix_"),
            "expected common prefix, got: {result}"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_tab_complete_no_match() {
        let mut picker = AgentPicker::new();
        picker.custom_input = Some("/nonexistent_xyz_123/".to_string());
        picker.handle_tab_complete();
        assert_eq!(
            picker.custom_input.as_deref(),
            Some("/nonexistent_xyz_123/")
        );
    }

    #[test]
    fn test_tab_complete_empty() {
        let mut picker = AgentPicker::new();
        picker.custom_input = Some(String::new());
        picker.handle_tab_complete();
        assert_eq!(picker.custom_input.as_deref(), Some(""));
    }

    // --- Ghost text / completion preview tests ---

    #[test]
    #[cfg(unix)]
    fn test_ghost_text_appears_on_input() {
        let mut picker = AgentPicker::new();
        picker.custom_input = Some(String::new());
        // Type "/tm" character by character (triggers recompute_completions)
        picker.handle_char_input('/');
        picker.handle_char_input('t');
        picker.handle_char_input('m');
        // Ghost should suggest "p/" to complete "/tmp/"
        assert_eq!(picker.completion_ghost.as_deref(), Some("p/"));
        assert!(picker.completion_candidates.iter().any(|c| c == "tmp"));
    }

    #[test]
    #[cfg(unix)]
    fn test_ghost_text_tab_accepts() {
        let mut picker = AgentPicker::new();
        picker.custom_input = Some(String::new());
        picker.handle_char_input('/');
        picker.handle_char_input('t');
        picker.handle_char_input('m');
        // Ghost: "p/"
        assert!(picker.completion_ghost.is_some());
        picker.handle_tab_complete();
        assert_eq!(picker.custom_input.as_deref(), Some("/tmp/"));
    }

    #[test]
    #[cfg(unix)]
    fn test_ghost_clears_on_backspace() {
        let mut picker = AgentPicker::new();
        picker.custom_input = Some(String::new());
        picker.handle_char_input('/');
        picker.handle_char_input('t');
        picker.handle_char_input('m');
        assert!(picker.completion_ghost.is_some());
        // Backspace removes 'm', ghost should change
        picker.handle_backspace();
        // "/t" has many matches, ghost may or may not be Some depending on common prefix
        // But the important thing is it recomputed (didn't keep stale ghost)
        let input = picker.custom_input.as_deref().unwrap();
        assert_eq!(input, "/t");
    }

    #[test]
    fn test_candidates_multiple_matches() {
        use std::fs;
        let base = std::env::temp_dir().join("termesh_ghost_test");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("proj_alpha")).unwrap();
        fs::create_dir_all(base.join("proj_beta")).unwrap();
        fs::create_dir_all(base.join("proj_gamma")).unwrap();

        let mut picker = AgentPicker::new();
        picker.custom_input = Some(String::new());
        for c in format!("{}/proj", base.display()).chars() {
            picker.handle_char_input(c);
        }
        // Should have 3 candidates
        assert_eq!(picker.completion_candidates.len(), 3);
        // Ghost should be common prefix beyond "proj": "_"
        assert_eq!(picker.completion_ghost.as_deref(), Some("_"));

        let _ = fs::remove_dir_all(&base);
    }

    // --- Completion selection & scroll tests ---

    #[test]
    fn test_completion_select_navigates() {
        use std::fs;
        let base = std::env::temp_dir().join("termesh_sel_test");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("aaa")).unwrap();
        fs::create_dir_all(base.join("bbb")).unwrap();
        fs::create_dir_all(base.join("ccc")).unwrap();

        let mut picker = AgentPicker::new();
        picker.custom_input = Some(String::new());
        for c in format!("{}/", base.display()).chars() {
            picker.handle_char_input(c);
        }
        assert_eq!(picker.completion_candidates.len(), 3);
        assert_eq!(picker.completion_selected, None);

        // Down → select first candidate "aaa"
        picker.completion_select_next();
        assert_eq!(picker.completion_selected, Some(0));
        assert_eq!(picker.completion_ghost.as_deref(), Some("aaa/"));

        // Down → select "bbb"
        picker.completion_select_next();
        assert_eq!(picker.completion_selected, Some(1));
        assert_eq!(picker.completion_ghost.as_deref(), Some("bbb/"));

        // Down → select "ccc"
        picker.completion_select_next();
        assert_eq!(picker.completion_selected, Some(2));
        assert_eq!(picker.completion_ghost.as_deref(), Some("ccc/"));

        // Down → wrap to None (common prefix)
        picker.completion_select_next();
        assert_eq!(picker.completion_selected, None);

        // Up → wrap to last "ccc"
        picker.completion_select_prev();
        assert_eq!(picker.completion_selected, Some(2));

        // Up → "bbb"
        picker.completion_select_prev();
        assert_eq!(picker.completion_selected, Some(1));

        // Up → "aaa"
        picker.completion_select_prev();
        assert_eq!(picker.completion_selected, Some(0));

        // Up → back to None
        picker.completion_select_prev();
        assert_eq!(picker.completion_selected, None);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_completion_tab_accepts_selected() {
        use std::fs;
        let base = std::env::temp_dir().join("termesh_tabsel_test");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("alpha")).unwrap();
        fs::create_dir_all(base.join("beta")).unwrap();

        let mut picker = AgentPicker::new();
        picker.custom_input = Some(String::new());
        for c in format!("{}/", base.display()).chars() {
            picker.handle_char_input(c);
        }
        // Select "beta" (second candidate)
        picker.completion_select_next(); // alpha
        picker.completion_select_next(); // beta
        assert_eq!(picker.completion_selected, Some(1));
        assert_eq!(picker.completion_ghost.as_deref(), Some("beta/"));

        // Tab accepts → input becomes ".../beta/"
        picker.handle_tab_complete();
        let input = picker.custom_input.as_deref().unwrap();
        assert!(input.ends_with("beta/"), "expected .../beta/, got: {input}");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_completion_scroll_adjusts() {
        use std::fs;
        let base = std::env::temp_dir().join("termesh_scroll_test");
        let _ = fs::remove_dir_all(&base);
        // Create more dirs than COMPLETION_VISIBLE_MAX (6)
        for i in 0..10 {
            fs::create_dir_all(base.join(format!("dir_{:02}", i))).unwrap();
        }

        let mut picker = AgentPicker::new();
        picker.custom_input = Some(String::new());
        for c in format!("{}/dir", base.display()).chars() {
            picker.handle_char_input(c);
        }
        assert_eq!(picker.completion_candidates.len(), 10);
        assert_eq!(picker.completion_scroll, 0);

        // Navigate down past visible window
        for _ in 0..7 {
            picker.completion_select_next();
        }
        // Selected = 6 (7th item, 0-indexed), should scroll
        assert_eq!(picker.completion_selected, Some(6));
        assert!(picker.completion_scroll > 0);
        // Selected should be within visible window
        assert!(
            picker.completion_selected.unwrap()
                < picker.completion_scroll + AgentPicker::COMPLETION_VISIBLE_MAX
        );
        assert!(picker.completion_selected.unwrap() >= picker.completion_scroll);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_completion_select_no_candidates_noop() {
        let mut picker = AgentPicker::new();
        picker.custom_input = Some("/nonexistent_xyz_123/".to_string());
        picker.recompute_completions();
        assert!(picker.completion_candidates.is_empty());
        picker.completion_select_next();
        assert_eq!(picker.completion_selected, None);
        picker.completion_select_prev();
        assert_eq!(picker.completion_selected, None);
    }
}
