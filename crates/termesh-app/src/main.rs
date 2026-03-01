mod agent_picker;
mod app;
mod cli;
#[allow(dead_code)]
mod session_manager;
mod theme;
mod ui_grid;

use agent_picker::AgentPicker;
use app::App;
use clap::Parser;
use cli::{Cli, Command};
use session_manager::SessionManager;
use std::path::PathBuf;
use std::time::Instant;
use termesh_agent::preset::WorkspacePreset;
use termesh_core::types::{AgentState, SplitLayout, ViewMode};
use termesh_diff::diff_generator::{self, DiffLine};
use termesh_diff::history::ChangeHistory;
use termesh_diff::watcher::{FileChangeKind, FileWatcher};
use termesh_input::action::Action;
use termesh_layout::focus_layout::FocusLayout;
use termesh_layout::session_list::SessionEntry;
use termesh_layout::split_layout::SplitLayoutManager;
use termesh_platform::event_loop::{self, AppCallbacks, PlatformConfig};
use termesh_pty::session::SessionConfig;
use termesh_terminal::grid::GridSnapshot;

/// Bridges the platform event loop to the session manager.
#[allow(dead_code)]
struct TermeshCallbacks {
    session_mgr: SessionManager,
    layout: SplitLayoutManager,
    /// Focus mode layout (session list + terminal + side panel).
    focus_layout: FocusLayout,
    /// Current view mode.
    view_mode: ViewMode,
    /// Window size in pixels (for layout calculations).
    window_size: (u32, u32),
    /// Font cell dimensions (cached from last resize).
    cell_size: (f32, f32),
    /// Whether the session list panel is visible.
    show_session_list: bool,
    /// Cached diff lines for the side panel.
    diff_lines: Vec<DiffLine>,
    /// Scroll offset for the side panel diff view.
    side_panel_scroll: usize,
    /// File watcher for detecting workspace file changes.
    file_watcher: Option<FileWatcher>,
    /// File change history for diff generation.
    change_history: ChangeHistory,
    /// Agent picker TUI (shown when no --agent flag).
    picker: Option<AgentPicker>,
    /// Current spinner frame index (0..9).
    spinner_frame: usize,
    /// Last time the spinner frame advanced.
    last_spinner_tick: Instant,
}

impl TermeshCallbacks {
    fn new() -> Self {
        let file_watcher = Self::start_watcher(None);
        Self {
            session_mgr: SessionManager::new(),
            layout: SplitLayoutManager::new(SplitLayout::Dual),
            focus_layout: FocusLayout::new(),
            view_mode: ViewMode::Focus,
            window_size: (800, 600),
            cell_size: (8.0, 16.0),
            show_session_list: true,
            diff_lines: Vec::new(),
            side_panel_scroll: 0,
            file_watcher,
            change_history: ChangeHistory::new(),
            picker: None,
            spinner_frame: 0,
            last_spinner_tick: Instant::now(),
        }
    }

    /// Create callbacks from a workspace preset, spawning all preset sessions.
    fn from_preset(preset: &WorkspacePreset) -> Self {
        let split = match preset.panes.len() {
            1 | 2 => SplitLayout::Dual,
            _ => SplitLayout::Quad,
        };

        let view_mode = match preset.default_mode.as_str() {
            "focus" => ViewMode::Focus,
            _ => ViewMode::Split,
        };

        // Use first pane's cwd as the watch root
        let watch_root = preset
            .panes
            .first()
            .and_then(|p| p.cwd.as_ref())
            .map(PathBuf::from);
        let file_watcher = Self::start_watcher(watch_root.as_deref());

        let mut callbacks = Self {
            session_mgr: SessionManager::new(),
            layout: SplitLayoutManager::new(split),
            focus_layout: FocusLayout::new(),
            view_mode,
            window_size: (800, 600),
            cell_size: (8.0, 16.0),
            show_session_list: true,
            diff_lines: Vec::new(),
            side_panel_scroll: 0,
            file_watcher,
            change_history: ChangeHistory::new(),
            picker: None,
            spinner_frame: 0,
            last_spinner_tick: Instant::now(),
        };

        // For single-pane presets, close the extra pane created by Dual layout
        if preset.panes.len() == 1 {
            let panes = callbacks.layout.layout().panes().to_vec();
            if panes.len() > 1 {
                callbacks.layout.layout_mut().close_pane(panes[1].id);
                // Expand remaining pane to fullscreen
                callbacks.layout.layout_mut().reset_single();
            }
        }

        let pane_ids: Vec<termesh_core::types::PaneId> = callbacks
            .layout
            .layout()
            .panes()
            .iter()
            .map(|p| p.id)
            .collect();

        for (i, pane_preset) in preset.panes.iter().enumerate() {
            if i >= pane_ids.len() {
                // More panes in preset than layout supports (>4); spawn extra via split
                let new_pane_id = callbacks.layout.layout_mut().split_horizontal();
                callbacks.spawn_preset_session(new_pane_id, pane_preset);
            } else {
                callbacks.spawn_preset_session(pane_ids[i], pane_preset);
            }
        }

        callbacks
    }

    /// Spawn a session from a PanePreset and bind it to a pane.
    fn spawn_preset_session(
        &mut self,
        pane_id: termesh_core::types::PaneId,
        pane_preset: &termesh_agent::preset::PanePreset,
    ) {
        let (command, args) = match &pane_preset.command {
            Some(cmd) => {
                let mut parts = cmd.split_whitespace();
                let cmd_name = parts.next().unwrap_or("").to_string();
                let args: Vec<String> = parts.map(|s| s.to_string()).collect();
                (cmd_name, args)
            }
            None => (termesh_core::platform::default_shell(), Vec::new()),
        };

        let config = SessionConfig {
            name: pane_preset.label.clone(),
            command,
            args,
            cwd: pane_preset.cwd.as_ref().map(PathBuf::from),
            agent: "auto".to_string(),
            ..Default::default()
        };

        match self.session_mgr.spawn(config) {
            Ok(session_id) => {
                self.layout.bind_session(pane_id, session_id);
                let is_agent = self.session_mgr.is_agent(session_id);
                self.focus_layout.sessions_mut().add(SessionEntry {
                    id: session_id,
                    label: pane_preset.label.clone(),
                    is_agent,
                    state: self.session_mgr.agent_state(session_id),
                });
            }
            Err(e) => {
                log::error!(
                    "failed to spawn preset session '{}': {e}",
                    pane_preset.label
                );
            }
        }
    }

    /// Spawn an agent session by type (claude, codex, gemini, shell).
    fn spawn_agent(&mut self, agent_type: &str) {
        let (command, args, label, _agent_kind, is_agent) = agent_picker::resolve_agent(agent_type);

        // In Split mode, bind to the focused pane; otherwise use pane[0].
        let target_pane_id = if self.view_mode == ViewMode::Split {
            self.layout.layout().focused_pane().id
        } else {
            self.layout.layout().panes()[0].id
        };

        let config = SessionConfig {
            name: label.clone(),
            command,
            args,
            cwd: None,
            agent: if is_agent {
                agent_type.to_string()
            } else {
                "none".to_string()
            },
            ..Default::default()
        };

        match self.session_mgr.spawn(config) {
            Ok(session_id) => {
                self.layout.bind_session(target_pane_id, session_id);
                self.focus_layout.sessions_mut().add(SessionEntry {
                    id: session_id,
                    label,
                    is_agent,
                    state: if is_agent {
                        AgentState::Idle
                    } else {
                        AgentState::None
                    },
                });
            }
            Err(e) => {
                log::error!("failed to spawn agent session: {e}");
            }
        }
    }

    /// Check whether any pane in the split layout has no session bound.
    fn has_empty_panes(&self) -> bool {
        self.layout
            .layout()
            .panes()
            .iter()
            .any(|p| p.session_id.is_none())
    }

    /// Resize all split pane PTYs to match their current dimensions.
    fn resize_split_panes(&mut self) {
        use termesh_layout::focus_layout::{HEADER_HEIGHT, STATUS_HEIGHT};

        let (w, h) = self.window_size;
        let (cw, ch) = self.cell_size;
        let header_px = (HEADER_HEIGHT as f32 * ch) as u32;
        let status_px = (STATUS_HEIGHT as f32 * ch) as u32;
        let regions = self
            .focus_layout
            .compute_regions_with_bars(w, h, header_px, status_px);
        for pane in self.layout.layout().panes().to_vec() {
            if let Some(sid) = pane.session_id {
                let (r, c) =
                    pane.grid_size(regions.terminal.width, regions.terminal.height, cw, ch);
                self.session_mgr.resize(sid, r, c);
            }
        }
    }

    /// Show the agent picker if there are empty panes (Split mode).
    fn show_picker_if_needed(&mut self) {
        if self.view_mode == ViewMode::Split && self.has_empty_panes() {
            // Focus the first empty pane so the picker renders there
            if let Some(idx) = self
                .layout
                .layout()
                .panes()
                .iter()
                .position(|p| p.session_id.is_none())
            {
                self.layout.focus_index(idx);
            }
            self.picker = Some(AgentPicker::new());
        }
    }

    /// Sync agent states from SessionManager to FocusLayout's session list.
    fn sync_agent_states(&mut self) {
        for session_id in self.session_mgr.session_ids() {
            let state = self.session_mgr.agent_state(session_id);
            self.focus_layout
                .sessions_mut()
                .update_state(session_id, state);
        }
    }

    /// Get the session ID that should receive input (mode-aware).
    fn focused_session(&self) -> Option<termesh_core::types::SessionId> {
        match self.view_mode {
            ViewMode::Focus => self.focus_layout.sessions().selected_id(),
            ViewMode::Split => self.layout.layout().focused_pane().session_id,
        }
    }

    /// Select the N-th session (0-based) in both Focus and Split mode.
    ///
    /// - Focus: switch to that session.
    /// - Split: if the session is already visible in a pane, focus that pane;
    ///   otherwise, replace the current pane's session with the target.
    fn select_session_by_index(&mut self, index: usize) {
        let entries = self.focus_layout.sessions().entries().to_vec();
        let target_id = match entries.get(index).map(|e| e.id) {
            Some(id) => id,
            None => return,
        };

        match self.view_mode {
            ViewMode::Focus => {
                self.focus_layout.sessions_mut().select_by_id(target_id);
                self.resize_focused_session();
            }
            ViewMode::Split => {
                // Check if target session is already visible in a pane
                let panes = self.layout.layout().panes().to_vec();
                if let Some(pane_idx) = panes.iter().position(|p| p.session_id == Some(target_id)) {
                    // Already visible → just move focus
                    self.layout.focus_index(pane_idx);
                } else {
                    // Not visible → replace current pane's session
                    let focused_pane_id = self.layout.layout().focused_pane().id;
                    self.layout.bind_session(focused_pane_id, target_id);
                    self.resize_split_panes();
                }
                // Keep session list selection in sync
                self.focus_layout.sessions_mut().select_by_id(target_id);
            }
        }
    }

    /// Resize the selected session to fill the terminal region (for Focus mode).
    fn resize_focused_session(&mut self) {
        use termesh_layout::focus_layout::{HEADER_HEIGHT, STATUS_HEIGHT};

        if self.view_mode != ViewMode::Focus {
            return;
        }
        if let Some(session_id) = self.focus_layout.sessions().selected_id() {
            let (width, height) = self.window_size;
            let (cell_w, cell_h) = self.cell_size;
            let header_px = (HEADER_HEIGHT as f32 * cell_h) as u32;
            let status_px = (STATUS_HEIGHT as f32 * cell_h) as u32;
            let regions = self
                .focus_layout
                .compute_regions_with_bars(width, height, header_px, status_px);
            let cols = (regions.terminal.width as f32 / cell_w).floor() as usize;
            let rows = (regions.terminal.height as f32 / cell_h).floor() as usize;
            self.session_mgr
                .resize(session_id, rows.max(1), cols.max(1));
        }
    }

    /// Apply a split layout, bind sessions to panes, and resize PTYs.
    fn enter_split(&mut self, split: SplitLayout) {
        use termesh_layout::focus_layout::{HEADER_HEIGHT, STATUS_HEIGHT};

        // Apply a fresh layout (resets pane positions to equal sizes)
        self.layout.set_split(split);

        // Bind existing sessions to panes; leave excess panes empty.
        let entries = self.focus_layout.sessions().entries().to_vec();
        let panes = self.layout.layout().panes().to_vec();
        for (i, pane) in panes.iter().enumerate() {
            if let Some(entry) = entries.get(i) {
                self.layout.bind_session(pane.id, entry.id);
            }
            // Empty panes stay unbound — picker will fill them.
        }

        // Resize all pane PTYs to match their new dimensions
        let (w, h) = self.window_size;
        let (cw, ch) = self.cell_size;
        let header_px = (HEADER_HEIGHT as f32 * ch) as u32;
        let status_px = (STATUS_HEIGHT as f32 * ch) as u32;
        let regions = self
            .focus_layout
            .compute_regions_with_bars(w, h, header_px, status_px);
        for pane in self.layout.layout().panes().to_vec() {
            if let Some(sid) = pane.session_id {
                let (r, c) =
                    pane.grid_size(regions.terminal.width, regions.terminal.height, cw, ch);
                self.session_mgr.resize(sid, r, c);
            }
        }
    }

    /// Start a FileWatcher on the given path, or current directory if None.
    fn start_watcher(root: Option<&std::path::Path>) -> Option<FileWatcher> {
        let path = match root {
            Some(p) => p.to_path_buf(),
            None => std::env::current_dir().unwrap_or_default(),
        };
        if !path.is_dir() {
            return None;
        }
        match FileWatcher::new(&path) {
            Ok(w) => {
                log::info!("file watcher started: {}", path.display());
                Some(w)
            }
            Err(e) => {
                log::warn!("failed to start file watcher: {e}");
                None
            }
        }
    }

    /// Poll file watcher for changes and update diff_lines.
    fn poll_file_changes(&mut self) {
        let watcher = match &self.file_watcher {
            Some(w) => w,
            None => return,
        };

        let changes = watcher.drain();
        if changes.is_empty() {
            return;
        }

        let mut updated = false;
        for change in changes {
            match change.kind {
                FileChangeKind::Created | FileChangeKind::Modified => {
                    if self.change_history.record_change(&change.path) {
                        updated = true;
                    }
                }
                FileChangeKind::Removed => {
                    // Skip removed files for diff display
                }
            }
        }

        if updated {
            // Rebuild diff_lines from the most recent change
            self.diff_lines.clear();
            for record in self.change_history.recent(1) {
                if let Some(old) = &record.old_content {
                    let diff = diff_generator::diff_texts(old, &record.new_content);
                    self.diff_lines = diff.lines;
                }
            }
        }
    }
}

impl AppCallbacks for TermeshCallbacks {
    fn on_input(&mut self, text: &[u8]) {
        // Editing mode: inline rename of session label
        if self.focus_layout.sessions().is_editing() {
            match text {
                b"\r" => {
                    self.focus_layout.sessions_mut().confirm_editing();
                }
                b"\x1b" => {
                    self.focus_layout.sessions_mut().cancel_editing();
                }
                b"\x7f" | b"\x08" => {
                    if let Some(edit) = self.focus_layout.sessions_mut().edit_state_mut() {
                        edit.backspace();
                    }
                }
                b"\x1b[D" => {
                    if let Some(edit) = self.focus_layout.sessions_mut().edit_state_mut() {
                        edit.move_left();
                    }
                }
                b"\x1b[C" => {
                    if let Some(edit) = self.focus_layout.sessions_mut().edit_state_mut() {
                        edit.move_right();
                    }
                }
                b"\x1b[3~" => {
                    if let Some(edit) = self.focus_layout.sessions_mut().edit_state_mut() {
                        edit.delete();
                    }
                }
                _ => {
                    if let Ok(s) = std::str::from_utf8(text) {
                        for c in s.chars() {
                            if !c.is_control() {
                                if let Some(edit) =
                                    self.focus_layout.sessions_mut().edit_state_mut()
                                {
                                    edit.insert(c);
                                }
                            }
                        }
                    }
                }
            }
            return;
        }

        // Picker mode: arrow keys navigate, Enter confirms
        if let Some(picker) = &mut self.picker {
            match text {
                b"\x1b[A" => picker.select_prev(), // Arrow Up
                b"\x1b[B" => picker.select_next(), // Arrow Down
                b"\x1b" => {
                    // Escape: cancel picker (only if sessions exist)
                    if !self.session_mgr.is_empty() {
                        self.picker = None;
                    }
                }
                b"\r" => {
                    if let Some(agent_type) = picker.try_confirm() {
                        let agent_type = agent_type.to_string();
                        self.picker = None;
                        self.spawn_agent(&agent_type);
                        if self.view_mode == ViewMode::Split {
                            // Resize the newly bound pane
                            self.resize_split_panes();
                            // Still empty panes? Show picker again.
                            self.show_picker_if_needed();
                        } else {
                            self.resize_focused_session();
                        }
                    }
                    // If try_confirm() returned None, error_message is set and
                    // the picker stays visible so the user can pick another option.
                }
                _ => {}
            }
            return;
        }

        // Send input to the session bound to the focused pane
        let focused = self.focused_session();
        if focused.is_none() {
            log::warn!(
                "on_input: mode={:?} NO focused session! pane={:?}",
                self.view_mode,
                match self.view_mode {
                    ViewMode::Split => {
                        let p = self.layout.layout().focused_pane();
                        format!("id={:?} session={:?}", p.id, p.session_id)
                    }
                    ViewMode::Focus =>
                        format!("selected={:?}", self.focus_layout.sessions().selected_id()),
                }
            );
        }
        if let Some(session_id) = focused {
            let _ = self.session_mgr.write_to(session_id, text);
        }
    }

    fn on_action(&mut self, action: Action) {
        // Picker mode: only handle navigation
        if let Some(picker) = &mut self.picker {
            match action {
                Action::FocusNext => picker.select_next(),
                Action::FocusPrev => picker.select_prev(),
                _ => {}
            }
            return;
        }

        match action {
            Action::SplitHorizontal | Action::SplitVertical => {
                // Switch to Dual split (or Quad if already Dual)
                if self.view_mode == ViewMode::Focus {
                    self.view_mode = ViewMode::Split;
                    self.enter_split(SplitLayout::Dual);
                } else {
                    let current = self.layout.layout().mode();
                    let next = match current {
                        SplitLayout::Dual => SplitLayout::Quad,
                        SplitLayout::Quad => SplitLayout::Dual,
                    };
                    self.enter_split(next);
                }
                self.show_picker_if_needed();
            }
            Action::ClosePane => {
                match self.view_mode {
                    ViewMode::Focus => {
                        // Kill the current session
                        if let Some(sid) = self.focus_layout.sessions().selected_id() {
                            self.session_mgr.remove(sid);
                            self.focus_layout.sessions_mut().remove(sid);
                            for pane in self.layout.layout_mut().panes_mut() {
                                if pane.session_id == Some(sid) {
                                    pane.unbind_session();
                                }
                            }
                            if self.focus_layout.sessions().is_empty() {
                                self.picker = Some(AgentPicker::new());
                            } else {
                                self.resize_focused_session();
                            }
                        }
                    }
                    ViewMode::Split => {
                        // Return to Focus mode
                        self.view_mode = ViewMode::Focus;
                        self.resize_focused_session();
                    }
                }
            }
            Action::FocusNext => match self.view_mode {
                ViewMode::Focus => {
                    self.focus_layout.sessions_mut().select_next();
                    self.resize_focused_session();
                }
                ViewMode::Split => {
                    self.layout.focus_next();
                }
            },
            Action::FocusPrev => match self.view_mode {
                ViewMode::Focus => {
                    self.focus_layout.sessions_mut().select_prev();
                    self.resize_focused_session();
                }
                ViewMode::Split => {
                    self.layout.focus_prev();
                }
            },
            Action::FocusPane1 => self.select_session_by_index(0),
            Action::FocusPane2 => self.select_session_by_index(1),
            Action::FocusPane3 => self.select_session_by_index(2),
            Action::FocusPane4 => self.select_session_by_index(3),
            Action::FocusPane5 => self.select_session_by_index(4),
            Action::FocusPane6 => self.select_session_by_index(5),
            Action::FocusPane7 => self.select_session_by_index(6),
            Action::FocusPane8 => self.select_session_by_index(7),
            Action::FocusPane9 => self.select_session_by_index(8),
            Action::ToggleMode => {
                // Cycle: Focus → Split(Dual) → Split(Quad) → Focus
                let current_split = self.layout.layout().mode();
                let (new_mode, new_split) = match self.view_mode {
                    ViewMode::Focus => (ViewMode::Split, Some(SplitLayout::Dual)),
                    ViewMode::Split => match current_split {
                        SplitLayout::Dual => (ViewMode::Split, Some(SplitLayout::Quad)),
                        SplitLayout::Quad => (ViewMode::Focus, None),
                    },
                };
                log::info!(
                    "ToggleMode: {:?}({:?}) → {:?}({:?})",
                    self.view_mode,
                    current_split,
                    new_mode,
                    new_split
                );
                self.view_mode = new_mode;
                if let Some(split) = new_split {
                    self.enter_split(split);
                    self.show_picker_if_needed();
                }
                self.resize_focused_session();
            }
            Action::SpawnSession => {
                // Show agent picker instead of spawning a shell directly
                log::info!("SpawnSession → opening agent picker");
                self.picker = Some(AgentPicker::new());
            }
            Action::ToggleSidePanel => {
                if self.view_mode == ViewMode::Focus {
                    self.focus_layout.toggle_side_panel();
                    self.side_panel_scroll = 0;
                    self.resize_focused_session();
                }
            }
            Action::RenameSession => {
                self.focus_layout.sessions_mut().start_editing();
            }
            Action::ToggleSessionList => {
                self.show_session_list = !self.show_session_list;
                if self.show_session_list {
                    self.focus_layout.set_session_list_width(180);
                } else {
                    self.focus_layout.set_session_list_width(0);
                }
                match self.view_mode {
                    ViewMode::Focus => self.resize_focused_session(),
                    ViewMode::Split => self.resize_split_panes(),
                }
            }
            Action::SidePanelScrollUp => {
                if self.focus_layout.side_panel().is_visible() {
                    self.side_panel_scroll = self.side_panel_scroll.saturating_sub(5);
                }
            }
            Action::SidePanelScrollDown => {
                if self.focus_layout.side_panel().is_visible() {
                    let max = self.diff_lines.len().saturating_sub(1);
                    self.side_panel_scroll = (self.side_panel_scroll + 5).min(max);
                }
            }
            Action::SidePanelNextTab => {
                if self.focus_layout.side_panel().is_visible() {
                    self.focus_layout.side_panel_mut().next_tab();
                }
            }
            Action::SidePanelPrevTab => {
                if self.focus_layout.side_panel().is_visible() {
                    self.focus_layout.side_panel_mut().prev_tab();
                }
            }
            Action::Copy | Action::Paste => { /* handled by platform layer */ }
        }
    }

    fn on_tick(&mut self) -> Vec<(GridSnapshot, f32, f32)> {
        use termesh_layout::focus_layout::{HEADER_HEIGHT, STATUS_HEIGHT};

        // Initial picker (no sessions yet): full-screen overlay
        if let Some(picker) = self.picker.as_ref() {
            if self.session_mgr.is_empty() {
                let (screen_w, screen_h) = self.window_size;
                let (cell_w, cell_h) = self.cell_size;
                let cols = (screen_w as f32 / cell_w).floor() as usize;
                let rows = (screen_h as f32 / cell_h).floor() as usize;
                return vec![(picker.render(rows, cols), 0.0, 0.0)];
            }
        }

        // Process pending PTY output; clean up exited sessions
        let exited = self.session_mgr.process_events();
        if !exited.is_empty() {
            for &sid in &exited {
                self.focus_layout.sessions_mut().remove(sid);
                // Unbind from any pane that had this session
                for pane in self.layout.layout_mut().panes_mut() {
                    if pane.session_id == Some(sid) {
                        pane.unbind_session();
                    }
                }
            }

            // After removal: ensure we have a valid selection or show picker
            if self.focus_layout.sessions().is_empty() {
                // No sessions left → show agent picker
                self.picker = Some(AgentPicker::new());
            } else {
                // Resize the newly selected session (select_index auto-clamped)
                self.resize_focused_session();
                // In Split mode, re-sync pane bindings
                if self.view_mode == ViewMode::Split {
                    self.resize_split_panes();
                }
            }
        }

        // Advance spinner frame every 100ms
        if self.last_spinner_tick.elapsed() >= std::time::Duration::from_millis(100) {
            self.spinner_frame = (self.spinner_frame + 1) % 10;
            self.last_spinner_tick = Instant::now();
        }

        // Sync agent states to session list
        self.sync_agent_states();

        // In Split mode, sync session list selection with focused pane
        if self.view_mode == ViewMode::Split {
            if let Some(sid) = self.layout.layout().focused_pane().session_id {
                self.focus_layout.sessions_mut().select_by_id(sid);
            }
        }

        // Poll file watcher for changes and update diff
        self.poll_file_changes();

        let (screen_w, screen_h) = self.window_size;
        let (cell_w, cell_h) = self.cell_size;
        let mut grids = Vec::new();

        let header_px = (HEADER_HEIGHT as f32 * cell_h) as u32;
        let status_px = (STATUS_HEIGHT as f32 * cell_h) as u32;
        let total_cols = (screen_w as f32 / cell_w).floor() as usize;

        let regions = self
            .focus_layout
            .compute_regions_with_bars(screen_w, screen_h, header_px, status_px);

        // ── Header bar ──
        {
            let (session_label, agent_state) = match self.focus_layout.sessions().selected_entry() {
                Some(entry) => (Some(entry.label.as_str()), Some(entry.state)),
                None => (None, None),
            };
            let header_grid = ui_grid::render_header_bar(
                total_cols,
                self.view_mode,
                session_label,
                agent_state,
                self.spinner_frame,
            );
            grids.push((header_grid, 0.0, 0.0));
        }

        // ── Status bar ──
        {
            let session_count = self.focus_layout.sessions().len();
            let selected_idx = self.focus_layout.sessions().selected_index();
            let status_grid =
                ui_grid::render_status_bar(total_cols, session_count, selected_idx, self.view_mode);
            let status_y = screen_h.saturating_sub(status_px) as f32;
            grids.push((status_grid, 0.0, status_y));
        }

        // Render session list panel if visible
        if self.show_session_list {
            let list_rect = regions.session_list;
            if list_rect.width > 0 && list_rect.height > 0 {
                let list_cols = (list_rect.width as f32 / cell_w).floor() as usize;
                let list_rows = (list_rect.height as f32 / cell_h).floor() as usize;
                let agent_kinds: Vec<String> = self
                    .focus_layout
                    .sessions()
                    .entries()
                    .iter()
                    .map(|e| self.session_mgr.agent_kind(e.id).to_string())
                    .collect();
                let list_grid = ui_grid::render_session_list(
                    self.focus_layout.sessions(),
                    list_rows,
                    list_cols,
                    self.spinner_frame,
                    &agent_kinds,
                );
                grids.push((list_grid, list_rect.x as f32, list_rect.y as f32));
            }
        }

        // Render side panel if visible
        let side_rect = regions.side_panel;
        if side_rect.width > 0 && side_rect.height > 0 {
            let panel_cols = (side_rect.width as f32 / cell_w).floor() as usize;
            let panel_rows = (side_rect.height as f32 / cell_h).floor() as usize;
            let panel_grid = ui_grid::render_side_panel(
                self.focus_layout.side_panel(),
                &self.diff_lines,
                panel_rows,
                panel_cols,
                self.side_panel_scroll,
            );
            grids.push((panel_grid, side_rect.x as f32, side_rect.y as f32));
        }

        // Terminal grids in center region
        let terminal_rect = regions.terminal;

        match self.view_mode {
            ViewMode::Focus => {
                if let Some(picker) = &self.picker {
                    // Focus mode picker: full terminal area
                    let picker_cols = (terminal_rect.width as f32 / cell_w).floor() as usize;
                    let picker_rows = (terminal_rect.height as f32 / cell_h).floor() as usize;
                    grids.push((
                        picker.render(picker_rows, picker_cols),
                        terminal_rect.x as f32,
                        terminal_rect.y as f32,
                    ));
                } else if let Some(session_id) = self.focus_layout.sessions().selected_id() {
                    if let Some(terminal) = self.session_mgr.terminal(session_id) {
                        grids.push((
                            terminal.render_grid(),
                            terminal_rect.x as f32,
                            terminal_rect.y as f32,
                        ));
                    }
                }
            }
            ViewMode::Split => {
                // Split mode: render pane header + terminal for each visible pane
                let focused_pane_id = self.layout.layout().focused_pane().id;
                for pane in self.layout.layout().panes() {
                    if !self.layout.is_pane_visible(pane.id) {
                        continue;
                    }
                    let rect = pane.pixel_rect(terminal_rect.width, terminal_rect.height);
                    let pane_x = terminal_rect.x as f32 + rect.x as f32;
                    let pane_y = terminal_rect.y as f32 + rect.y as f32;
                    let is_focused = pane.id == focused_pane_id;

                    // Show picker inside focused pane if active
                    if let Some(picker) = self.picker.as_ref() {
                        if is_focused {
                            let pane_cols = (rect.width as f32 / cell_w).floor() as usize;
                            let pane_rows = (rect.height as f32 / cell_h).floor() as usize;
                            grids.push((picker.render(pane_rows, pane_cols), pane_x, pane_y));
                            continue;
                        }
                    }

                    if let Some(session_id) = pane.session_id {
                        // Pane header
                        let entries = self.focus_layout.sessions().entries();
                        let (label, session_index) = entries
                            .iter()
                            .enumerate()
                            .find(|(_, e)| e.id == session_id)
                            .map(|(i, e)| (e.label.as_str(), i))
                            .unwrap_or(("???", 0));
                        let state = self.session_mgr.agent_state(session_id);
                        let agent_kind = self.session_mgr.agent_kind(session_id);
                        let pane_cols = (rect.width as f32 / cell_w).floor() as usize;
                        let pane_header = ui_grid::render_pane_header(
                            label,
                            agent_kind,
                            state,
                            is_focused,
                            pane_cols,
                            self.spinner_frame,
                            session_index,
                        );
                        grids.push((pane_header, pane_x, pane_y));

                        // Terminal content (offset by 1 row for header)
                        if let Some(terminal) = self.session_mgr.terminal(session_id) {
                            grids.push((terminal.render_grid(), pane_x, pane_y + cell_h));
                        }
                    }
                }
            }
        }

        grids
    }

    fn dividers(&self) -> Vec<(f32, f32, f32, bool, [f32; 4])> {
        // No dividers in picker mode
        if self.picker.is_some() {
            return Vec::new();
        }

        use termesh_layout::focus_layout::{HEADER_HEIGHT, STATUS_HEIGHT};

        // theme::BORDER = #2A2C32 → [0.165, 0.173, 0.196, 1.0]
        const BORDER_COLOR: [f32; 4] = [0.165, 0.173, 0.196, 1.0];
        // theme::ACCENT = #5C9FE0 → [0.361, 0.624, 0.878, 1.0]
        const FOCUS_COLOR: [f32; 4] = [0.361, 0.624, 0.878, 1.0];

        let (w, h) = self.window_size;
        let (_cell_w, cell_h) = self.cell_size;
        let header_px = (HEADER_HEIGHT as f32 * cell_h) as u32;
        let status_px = (STATUS_HEIGHT as f32 * cell_h) as u32;
        let regions = self
            .focus_layout
            .compute_regions_with_bars(w, h, header_px, status_px);
        let terminal_x = regions.terminal.x as f32;
        let terminal_y = regions.terminal.y as f32;
        let panel_h = regions.terminal.height;

        let mut dividers: Vec<(f32, f32, f32, bool, [f32; 4])> = Vec::new();

        // Split pane dividers + focused pane border
        if self.view_mode == ViewMode::Split {
            let terminal_w = regions.terminal.width;

            // Pane-to-pane dividers
            dividers.extend(
                self.layout
                    .compute_dividers(terminal_w, panel_h)
                    .iter()
                    .map(|d| {
                        let is_vertical = d.orientation
                            == termesh_layout::split_layout::DividerOrientation::Vertical;
                        (
                            terminal_x + d.x as f32,
                            terminal_y + d.y as f32,
                            d.length as f32,
                            is_vertical,
                            BORDER_COLOR,
                        )
                    }),
            );

            // Focused pane border (4 edges)
            let focused = self.layout.layout().focused_pane();
            let rect = focused.pixel_rect(terminal_w, panel_h);
            let fx = terminal_x + rect.x as f32;
            let fy = terminal_y + rect.y as f32;
            let fw = rect.width as f32;
            let fh = rect.height as f32;
            // Top edge
            dividers.push((fx, fy, fw, false, FOCUS_COLOR));
            // Bottom edge
            dividers.push((fx, fy + fh - 1.0, fw, false, FOCUS_COLOR));
            // Left edge
            dividers.push((fx, fy, fh, true, FOCUS_COLOR));
            // Right edge
            dividers.push((fx + fw - 1.0, fy, fh, true, FOCUS_COLOR));
        }

        // Session list / side panel dividers (both modes)
        if self.show_session_list && regions.session_list.width > 0 {
            dividers.push((
                terminal_x,
                header_px as f32,
                panel_h as f32,
                true,
                BORDER_COLOR,
            ));
        }
        if regions.side_panel.width > 0 {
            let side_x = regions.side_panel.x as f32;
            dividers.push((side_x, header_px as f32, panel_h as f32, true, BORDER_COLOR));
        }

        dividers
    }

    fn on_resize(
        &mut self,
        _rows: usize,
        _cols: usize,
        width: u32,
        height: u32,
        cell_w: f32,
        cell_h: f32,
    ) {
        use termesh_layout::focus_layout::{HEADER_HEIGHT, STATUS_HEIGHT};

        self.window_size = (width, height);
        self.cell_size = (cell_w, cell_h);

        let header_px = (HEADER_HEIGHT as f32 * cell_h) as u32;
        let status_px = (STATUS_HEIGHT as f32 * cell_h) as u32;
        let regions = self
            .focus_layout
            .compute_regions_with_bars(width, height, header_px, status_px);

        match self.view_mode {
            ViewMode::Focus => {
                // Resize only the selected session to fill the terminal region
                if let Some(session_id) = self.focus_layout.sessions().selected_id() {
                    let cols = (regions.terminal.width as f32 / cell_w).floor() as usize;
                    let rows = (regions.terminal.height as f32 / cell_h).floor() as usize;
                    self.session_mgr
                        .resize(session_id, rows.max(1), cols.max(1));
                }
            }
            ViewMode::Split => {
                // Per-pane resize: each pane gets its own grid dimensions
                let terminal_width = regions.terminal.width;
                let terminal_height = regions.terminal.height;
                for pane in self.layout.layout().panes().to_vec() {
                    if let Some(session_id) = pane.session_id {
                        let (rows, cols) =
                            pane.grid_size(terminal_width, terminal_height, cell_w, cell_h);
                        self.session_mgr.resize(session_id, rows, cols);
                    }
                }
            }
        }
    }

    fn on_scroll(&mut self, delta: i32) {
        if let Some(session_id) = self.focused_session() {
            if let Some(terminal) = self.session_mgr.terminal_mut(session_id) {
                if delta > 0 {
                    terminal.scroll_up(delta as usize);
                } else {
                    terminal.scroll_down((-delta) as usize);
                }
            }
        }
    }

    fn on_mouse_press(&mut self, row: usize, col: usize) {
        if let Some(session_id) = self.focused_session() {
            if let Some(terminal) = self.session_mgr.terminal_mut(session_id) {
                terminal.selection_start(row, col);
            }
        }
    }

    fn on_mouse_drag(&mut self, row: usize, col: usize) {
        if let Some(session_id) = self.focused_session() {
            if let Some(terminal) = self.session_mgr.terminal_mut(session_id) {
                terminal.selection_update(row, col);
            }
        }
    }

    fn on_mouse_release(&mut self) {
        // Selection stays visible until next click or explicit clear
    }

    fn on_copy(&mut self) -> Option<String> {
        let session_id = self.focused_session()?;
        let terminal = self.session_mgr.terminal(session_id)?;
        terminal.selected_text()
    }

    fn on_paste(&mut self, text: &str) {
        if let Some(session_id) = self.focused_session() {
            let _ = self.session_mgr.write_to(session_id, text.as_bytes());
        }
    }

    fn should_exit(&self) -> bool {
        if self.picker.is_some() {
            return false;
        }
        self.session_mgr.is_empty()
    }
}

fn main() {
    // Prevent nested termesh instances (screen-within-screen).
    if std::env::var("TERMESH").is_ok() {
        eprintln!("Error: termesh cannot run inside another termesh session.");
        std::process::exit(1);
    }

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    // Authentication gate
    let license_dir = termesh_core::platform::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("license");
    let store = termesh_core::license::LicenseStore::new(license_dir);
    match termesh_core::auth_gate::check_auth_local(&store) {
        termesh_core::auth_gate::AuthState::Authenticated { plan, email } => {
            log::info!(
                "authenticated: {} ({})",
                email.as_deref().unwrap_or("unknown"),
                plan
            );
        }
        termesh_core::auth_gate::AuthState::OfflineGrace { remaining_secs, .. } => {
            let hours = remaining_secs / 3600;
            eprintln!(
                "warning: offline mode — {}h remaining before re-authentication required",
                hours
            );
        }
        termesh_core::auth_gate::AuthState::NeedsLogin => {
            log::warn!("no authentication credentials found — running in trial mode");
        }
        termesh_core::auth_gate::AuthState::Failed(reason) => {
            log::warn!("authentication check failed: {reason} — running in trial mode");
        }
    }

    // Initialize tokio runtime for async session management
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let _guard = rt.enter();

    let cli = Cli::parse();

    let (app, callbacks) = match cli.command {
        Some(Command::Open { name }) => match App::open_workspace(&name) {
            Ok(app) => {
                let preset = {
                    let loader = termesh_agent::workspace::WorkspaceLoader::default_dir()
                        .expect("cannot determine config directory");
                    loader
                        .load(&name)
                        .unwrap_or_else(|e| panic!("failed to load workspace '{name}': {e}"))
                };
                let callbacks = TermeshCallbacks::from_preset(&preset);
                (app, callbacks)
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        None => {
            let app = App::new();
            let mut callbacks = TermeshCallbacks::new();

            if let Some(agent_type) = cli.agent {
                // --agent flag: spawn the specified agent directly
                callbacks.spawn_agent(&agent_type);
            } else {
                // No --agent flag: show agent picker TUI
                callbacks.picker = Some(AgentPicker::new());
            }

            (app, callbacks)
        }
    };

    let config = PlatformConfig {
        font_size: 14.0,
        scrollback: 10_000,
        input_handler: app.input().clone(),
        callbacks: Some(Box::new(callbacks)),
    };

    if let Err(e) = event_loop::run(config) {
        eprintln!("Fatal: {e}");
        std::process::exit(1);
    }
}
