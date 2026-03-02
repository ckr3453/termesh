mod agent_picker;
mod app;
mod cli;
#[allow(dead_code)]
mod session_manager;
mod session_picker;
mod theme;
mod ui_grid;

use agent_picker::AgentPicker;
use app::App;
use clap::Parser;
use cli::{Cli, Command};
use session_manager::SessionManager;
use session_picker::{SessionPicker, SessionPickerEntry};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;
use termesh_agent::preset::WorkspacePreset;
use termesh_core::types::{AgentState, SessionId, SplitLayout, ViewMode};
use termesh_diff::diff_generator::{DiffLine, DiffMode, SideBySideLine};
use termesh_diff::git_changes::GitChangeTracker;
use termesh_diff::history::ChangedFile;
use termesh_input::action::Action;
use termesh_layout::focus_layout::{FocusLayout, FocusRegion};
use termesh_layout::session_list::SessionEntry;
use termesh_layout::split_layout::SplitLayoutManager;
use termesh_platform::event_loop::{self, AppCallbacks, PlatformConfig};
use termesh_pty::session::SessionConfig;
use termesh_terminal::grid::GridSnapshot;

/// Which view the side panel is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SidePanelView {
    /// File list showing all changed files.
    FileList,
    /// Diff view for a specific file.
    FileDiff,
}

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
    /// Current side panel view mode.
    side_panel_view: SidePanelView,
    /// Cached list of changed files for the file list.
    changed_files: Vec<ChangedFile>,
    /// Selected index in the file list.
    file_list_selected: usize,
    /// Current diff display mode.
    diff_mode: DiffMode,
    /// Cached side-by-side diff lines.
    sbs_lines: Vec<SideBySideLine>,
    /// Per-session git change trackers.
    git_trackers: HashMap<SessionId, GitChangeTracker>,
    /// Agent picker TUI (shown when no --agent flag).
    picker: Option<AgentPicker>,
    /// Whether the agent picker should render full-screen (true) or inside a pane (false).
    picker_fullscreen: bool,
    /// Session picker overlay for swapping sessions in Split mode.
    session_picker: Option<SessionPicker>,
    /// Current spinner frame index (0..9).
    spinner_frame: usize,
    /// Last time the spinner frame advanced.
    last_spinner_tick: Instant,
}

impl TermeshCallbacks {
    fn new() -> Self {
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
            side_panel_view: SidePanelView::FileList,
            changed_files: Vec::new(),
            file_list_selected: 0,
            diff_mode: DiffMode::Unified,
            sbs_lines: Vec::new(),
            git_trackers: HashMap::new(),
            picker: None,
            picker_fullscreen: false,
            session_picker: None,
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

        let mut callbacks = Self {
            layout: SplitLayoutManager::new(split),
            view_mode,
            ..Self::new()
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
                self.init_git_tracker(session_id);
            }
            Err(e) => {
                log::error!(
                    "failed to spawn preset session '{}': {e}",
                    pane_preset.label
                );
            }
        }
    }

    /// Create a new agent picker with folder paths pre-populated.
    fn create_picker(&self) -> AgentPicker {
        let mut folders = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            folders.push(cwd);
        }
        for tracker in self.git_trackers.values() {
            let root = tracker.git_root().to_path_buf();
            if !folders.contains(&root) {
                folders.push(root);
            }
        }
        let mut picker = AgentPicker::new();
        picker.set_folders(folders);
        picker
    }

    /// Spawn an agent session by type (claude, codex, gemini, shell).
    ///
    /// Confirm the picker selection, spawn the agent, and clean up picker state.
    fn finalize_picker_spawn(&mut self) {
        let picker = match self.picker.as_mut() {
            Some(p) => p,
            None => return,
        };
        let agent_type = match picker.try_confirm() {
            Some(a) => a.to_string(),
            None => return,
        };
        let cwd = picker.selected_folder();
        self.picker = None;
        self.picker_fullscreen = false;
        self.spawn_agent(&agent_type, cwd);
        if self.view_mode == ViewMode::Split {
            self.resize_split_panes();
            self.show_picker_if_needed();
        } else {
            self.resize_focused_session();
        }
    }

    /// If `cwd` is `Some`, the session starts in the given directory.
    fn spawn_agent(&mut self, agent_type: &str, cwd: Option<PathBuf>) {
        let (command, args, label, _agent_kind, is_agent) = agent_picker::resolve_agent(agent_type);

        // Determine which pane to bind:
        // - Focus mode: always bind to pane[0] (the single visible pane)
        // - Split mode with empty focused pane: bind to focused pane
        // - Split mode with occupied focused pane: don't bind (session added to list only)
        let target_pane_id = match self.view_mode {
            ViewMode::Focus => Some(self.layout.layout().panes()[0].id),
            ViewMode::Split => {
                let focused = self.layout.layout().focused_pane();
                if focused.session_id.is_none() {
                    Some(focused.id)
                } else {
                    None
                }
            }
        };

        let config = SessionConfig {
            name: label.clone(),
            command,
            args,
            cwd,
            agent: if is_agent {
                agent_type.to_string()
            } else {
                "none".to_string()
            },
            ..Default::default()
        };

        match self.session_mgr.spawn(config) {
            Ok(session_id) => {
                if let Some(pane_id) = target_pane_id {
                    self.layout.bind_session(pane_id, session_id);
                }
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
                self.init_git_tracker(session_id);
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

    /// Fill empty split panes with unbound sessions.
    ///
    /// Collects session IDs already bound to panes, then assigns unbound
    /// sessions to empty panes in order.
    fn rebind_split_panes(&mut self) {
        let bound: HashSet<SessionId> = self
            .layout
            .layout()
            .panes()
            .iter()
            .filter_map(|p| p.session_id)
            .collect();
        let unbound: Vec<SessionId> = self
            .focus_layout
            .sessions()
            .entries()
            .iter()
            .filter(|e| !bound.contains(&e.id))
            .map(|e| e.id)
            .collect();
        let mut unbound_iter = unbound.into_iter();
        for pane in self.layout.layout().panes().to_vec() {
            if pane.session_id.is_none() {
                if let Some(sid) = unbound_iter.next() {
                    self.layout.bind_session(pane.id, sid);
                }
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
            self.picker = Some(self.create_picker());
        }
    }

    /// Build a SessionPicker with all sessions and their pane bindings.
    fn create_session_picker(&self) -> SessionPicker {
        let entries = self.focus_layout.sessions().entries();
        let panes = self.layout.layout().panes();
        let current_session = self.layout.layout().focused_pane().session_id;

        let picker_entries: Vec<SessionPickerEntry> = entries
            .iter()
            .map(|e| {
                let pane_index = panes.iter().position(|p| p.session_id == Some(e.id));
                SessionPickerEntry {
                    id: e.id,
                    label: e.label.clone(),
                    agent_kind: self.session_mgr.agent_kind(e.id).to_string(),
                    pane_index,
                }
            })
            .collect();

        SessionPicker::new(picker_entries, current_session)
    }

    /// Swap the focused pane's session with the given target session.
    ///
    /// - If target is already in another pane → exchange the two panes' sessions.
    /// - If target is unbound → bind it to the focused pane.
    /// - If target is already in the focused pane → no-op.
    fn swap_session_to_focused_pane(&mut self, target_id: SessionId) {
        let focused_pane_id = self.layout.layout().focused_pane().id;
        let focused_session = self.layout.layout().focused_pane().session_id;

        // No-op if already the same
        if focused_session == Some(target_id) {
            return;
        }

        // Find which pane has the target session
        let target_pane_id = self
            .layout
            .layout()
            .panes()
            .iter()
            .find(|p| p.session_id == Some(target_id))
            .map(|p| p.id);

        // Exchange: move focused session to the target's pane
        if let Some(other_pane_id) = target_pane_id {
            if let Some(fs) = focused_session {
                self.layout.bind_session(other_pane_id, fs);
            } else {
                for pane in self.layout.layout_mut().panes_mut() {
                    if pane.id == other_pane_id {
                        pane.unbind_session();
                        break;
                    }
                }
            }
        }

        // Bind target to focused pane
        self.layout.bind_session(focused_pane_id, target_id);
        self.resize_split_panes();
    }

    /// Sync agent states from SessionManager to FocusLayout's session list.
    fn sync_agent_states(&mut self) {
        for session_id in self.session_mgr.session_ids() {
            let state = self.session_mgr.agent_state(session_id);
            self.focus_layout
                .sessions_mut()
                .update_state(session_id, state);
            let is_agent = self.session_mgr.is_agent(session_id);
            self.focus_layout
                .sessions_mut()
                .update_is_agent(session_id, is_agent);
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
        match self.view_mode {
            ViewMode::Focus => {
                let entries = self.focus_layout.sessions().entries().to_vec();
                let target_id = match entries.get(index).map(|e| e.id) {
                    Some(id) => id,
                    None => return,
                };
                self.focus_layout.sessions_mut().select_by_id(target_id);
                self.resize_focused_session();
            }
            ViewMode::Split => {
                // In Split mode, Ctrl+1~9 focuses the pane at that position
                let panes = self.layout.layout().panes().to_vec();
                if index < panes.len() {
                    self.layout.focus_index(index);
                    // Sync session list selection with the pane's session
                    if let Some(sid) = panes[index].session_id {
                        self.focus_layout.sessions_mut().select_by_id(sid);
                    }
                }
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

    /// Initialize a git change tracker for a session.
    fn init_git_tracker(&mut self, session_id: SessionId) {
        if let Some(cwd) = self.session_mgr.session_cwd(session_id) {
            if let Some(tracker) = GitChangeTracker::new(cwd) {
                self.git_trackers.insert(session_id, tracker);
            }
        }
    }

    /// Poll git trackers for the active session and update the changed files list.
    fn poll_git_changes(&mut self) {
        let active_id = match self.focused_session() {
            Some(id) => id,
            None => return,
        };

        let updated = match self.git_trackers.get_mut(&active_id) {
            Some(tracker) => tracker.poll(),
            None => return,
        };

        if updated {
            self.changed_files = self.git_trackers[&active_id].changed_files().to_vec();
            if !self.changed_files.is_empty() {
                self.file_list_selected = self.file_list_selected.min(self.changed_files.len() - 1);
            } else {
                self.file_list_selected = 0;
            }

            if self.side_panel_view == SidePanelView::FileDiff {
                self.refresh_selected_diff();
            }
        }
    }

    /// Refresh diff_lines and sbs_lines for the currently selected file.
    fn refresh_selected_diff(&mut self) {
        use termesh_diff::diff_generator::side_by_side_diff;

        self.diff_lines.clear();
        self.sbs_lines.clear();

        let active_id = match self.focused_session() {
            Some(id) => id,
            None => return,
        };

        if let Some(file) = self.changed_files.get(self.file_list_selected) {
            if let Some(tracker) = self.git_trackers.get(&active_id) {
                if let Some((old, new)) = tracker.file_diff(&file.path) {
                    let diff = termesh_diff::diff_generator::diff_texts(&old, &new);
                    self.diff_lines = diff.lines;
                    self.sbs_lines = side_by_side_diff(&old, &new);
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

        // Picker mode: two-stage input handling
        let mut picker_spawn = false;
        if let Some(picker) = &mut self.picker {
            if picker.is_custom_input() {
                // Custom path input mode
                match text {
                    b"\x1b" => picker.cancel_custom_input(),
                    b"\x7f" | b"\x08" => picker.handle_backspace(),
                    b"\r" => picker_spawn = true,
                    _ => {
                        if let Ok(s) = std::str::from_utf8(text) {
                            for c in s.chars() {
                                if !c.is_control() {
                                    picker.handle_char_input(c);
                                }
                            }
                        }
                    }
                }
                if picker_spawn {
                    self.finalize_picker_spawn();
                }
                return;
            }

            if picker.is_folder_stage() {
                // Folder selection mode
                match text {
                    b"\x1b[A" => picker.select_prev(),
                    b"\x1b[B" => picker.select_next(),
                    b"\x1b" => picker.go_back(),
                    b"\r" => picker_spawn = true,
                    _ => {}
                }
                if picker_spawn {
                    self.finalize_picker_spawn();
                    return;
                }
            } else {
                // Agent selection mode
                match text {
                    b"\x1b[A" => picker.select_prev(),
                    b"\x1b[B" => picker.select_next(),
                    b"\x1b" => {
                        if !self.session_mgr.is_empty() {
                            self.picker = None;
                            self.picker_fullscreen = false;
                        }
                    }
                    b"\r" => {
                        // try_confirm transitions to SelectFolder, returns None
                        picker.try_confirm();
                    }
                    _ => {}
                }
            }
            return;
        }

        // Session picker mode (swap session in Split mode)
        if let Some(sp) = &mut self.session_picker {
            match text {
                b"\x1b[A" => sp.select_prev(),
                b"\x1b[B" => sp.select_next(),
                b"\x1b" => {
                    self.session_picker = None;
                }
                b"\r" => {
                    if let Some(target_id) = sp.confirm() {
                        self.session_picker = None;
                        self.swap_session_to_focused_pane(target_id);
                    }
                }
                _ => {}
            }
            return;
        }

        // SessionList focus mode: arrow keys navigate, Enter selects, Escape exits
        if self.view_mode == ViewMode::Focus
            && self.focus_layout.focus_region() == FocusRegion::SessionList
        {
            match text {
                b"\x1b[A" => {
                    self.focus_layout.sessions_mut().select_prev();
                    self.resize_focused_session();
                }
                b"\x1b[B" => {
                    self.focus_layout.sessions_mut().select_next();
                    self.resize_focused_session();
                }
                b"\r" => {
                    self.resize_focused_session();
                    self.focus_layout.set_focus(FocusRegion::Terminal);
                }
                b"\x1b" => {
                    self.focus_layout.set_focus(FocusRegion::Terminal);
                }
                _ => {} // ignore other input while session list is focused
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

        // Session picker mode: block other actions
        if self.session_picker.is_some() {
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
                            self.git_trackers.remove(&sid);
                            for pane in self.layout.layout_mut().panes_mut() {
                                if pane.session_id == Some(sid) {
                                    pane.unbind_session();
                                }
                            }
                            if self.focus_layout.sessions().is_empty() {
                                self.picker = Some(self.create_picker());
                                self.picker_fullscreen = true;
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
                // Show agent picker full-screen
                log::info!("SpawnSession → opening agent picker");
                self.picker = Some(self.create_picker());
                self.picker_fullscreen = true;
            }
            Action::ToggleSidePanel => {
                if self.view_mode == ViewMode::Focus {
                    self.focus_layout.toggle_side_panel();
                    self.side_panel_scroll = 0;
                    self.side_panel_view = SidePanelView::FileList;
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
                    match self.side_panel_view {
                        SidePanelView::FileList => {
                            self.file_list_selected = self.file_list_selected.saturating_sub(1);
                        }
                        SidePanelView::FileDiff => {
                            self.side_panel_scroll = self.side_panel_scroll.saturating_sub(5);
                        }
                    }
                }
            }
            Action::SidePanelScrollDown => {
                if self.focus_layout.side_panel().is_visible() {
                    match self.side_panel_view {
                        SidePanelView::FileList => {
                            if !self.changed_files.is_empty() {
                                self.file_list_selected =
                                    (self.file_list_selected + 1).min(self.changed_files.len() - 1);
                            }
                        }
                        SidePanelView::FileDiff => {
                            let max = match self.diff_mode {
                                DiffMode::Unified => self.diff_lines.len(),
                                DiffMode::SideBySide => self.sbs_lines.len(),
                            }
                            .saturating_sub(1);
                            self.side_panel_scroll = (self.side_panel_scroll + 5).min(max);
                        }
                    }
                }
            }
            Action::SidePanelSelect => {
                if self.focus_layout.side_panel().is_visible()
                    && self.side_panel_view == SidePanelView::FileList
                    && !self.changed_files.is_empty()
                {
                    self.side_panel_view = SidePanelView::FileDiff;
                    self.side_panel_scroll = 0;
                    self.refresh_selected_diff();
                }
            }
            Action::SidePanelBack => {
                if self.focus_layout.side_panel().is_visible()
                    && self.side_panel_view == SidePanelView::FileDiff
                {
                    self.side_panel_view = SidePanelView::FileList;
                    self.diff_lines.clear();
                    self.sbs_lines.clear();
                }
            }
            Action::ToggleDiffMode => {
                if self.focus_layout.side_panel().is_visible()
                    && self.side_panel_view == SidePanelView::FileDiff
                {
                    self.diff_mode = match self.diff_mode {
                        DiffMode::Unified => DiffMode::SideBySide,
                        DiffMode::SideBySide => DiffMode::Unified,
                    };
                    self.side_panel_scroll = 0;
                }
            }
            Action::CycleFocusRegion => {
                if self.view_mode == ViewMode::Focus {
                    self.focus_layout.cycle_focus();
                }
            }
            Action::SwapSession => {
                if self.view_mode == ViewMode::Split {
                    let sp = self.create_session_picker();
                    if !sp.is_empty() {
                        self.session_picker = Some(sp);
                    }
                }
            }
            Action::Copy | Action::Paste => { /* handled by platform layer */ }
        }
    }

    fn on_tick(&mut self) -> Vec<(GridSnapshot, f32, f32)> {
        use termesh_layout::focus_layout::{HEADER_HEIGHT, STATUS_HEIGHT};

        // Full-screen picker (no sessions or Ctrl+N): render over entire screen
        if self.picker_fullscreen && self.session_mgr.is_empty() {
            if let Some(picker) = self.picker.as_ref() {
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
                self.git_trackers.remove(&sid);
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
                self.picker = Some(self.create_picker());
                self.picker_fullscreen = true;
            } else {
                // Resize the newly selected session (select_index auto-clamped)
                self.resize_focused_session();
                // In Split mode, fill empty panes with remaining sessions
                if self.view_mode == ViewMode::Split {
                    self.rebind_split_panes();
                    self.resize_split_panes();
                    self.show_picker_if_needed();
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

        // Poll git trackers for changes and update diff
        self.poll_git_changes();

        let (screen_w, screen_h) = self.window_size;
        let (cell_w, cell_h) = self.cell_size;
        let mut grids = Vec::new();

        let header_px = (HEADER_HEIGHT as f32 * cell_h) as u32;
        let status_px = (STATUS_HEIGHT as f32 * cell_h) as u32;
        let total_cols = (screen_w as f32 / cell_w).floor() as usize;

        let regions = self
            .focus_layout
            .compute_regions_with_bars(screen_w, screen_h, header_px, status_px);

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
                let git_projects: Vec<String> = self
                    .focus_layout
                    .sessions()
                    .entries()
                    .iter()
                    .map(|e| {
                        self.git_trackers
                            .get(&e.id)
                            .and_then(|t| {
                                t.git_root()
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                            })
                            .unwrap_or_default()
                    })
                    .collect();
                let list_grid = ui_grid::render_session_list(
                    self.focus_layout.sessions(),
                    list_rows,
                    list_cols,
                    self.spinner_frame,
                    &agent_kinds,
                    &git_projects,
                );
                grids.push((list_grid, list_rect.x as f32, list_rect.y as f32));
            }
        }

        // Render side panel if visible
        let side_rect = regions.side_panel;
        if side_rect.width > 0 && side_rect.height > 0 {
            let panel_cols = (side_rect.width as f32 / cell_w).floor() as usize;
            let panel_rows = (side_rect.height as f32 / cell_h).floor() as usize;
            let panel_grid = match self.side_panel_view {
                SidePanelView::FileList => ui_grid::render_file_list(
                    self.focus_layout.side_panel(),
                    &self.changed_files,
                    self.file_list_selected,
                    panel_rows,
                    panel_cols,
                ),
                SidePanelView::FileDiff => match self.diff_mode {
                    DiffMode::Unified => ui_grid::render_side_panel(
                        self.focus_layout.side_panel(),
                        &self.diff_lines,
                        panel_rows,
                        panel_cols,
                        self.side_panel_scroll,
                    ),
                    DiffMode::SideBySide => ui_grid::render_side_by_side(
                        self.focus_layout.side_panel(),
                        &self.sbs_lines,
                        panel_rows,
                        panel_cols,
                        self.side_panel_scroll,
                    ),
                },
            };
            grids.push((panel_grid, side_rect.x as f32, side_rect.y as f32));
        }

        // Terminal grids in center region
        let terminal_rect = regions.terminal;

        // Agent picker full-screen (Ctrl+N or no sessions)
        if self.picker_fullscreen {
            if let Some(picker) = &self.picker {
                let picker_cols = (terminal_rect.width as f32 / cell_w).floor() as usize;
                let picker_rows = (terminal_rect.height as f32 / cell_h).floor() as usize;
                grids.push((
                    picker.render(picker_rows, picker_cols),
                    terminal_rect.x as f32,
                    terminal_rect.y as f32,
                ));
                return grids;
            }
        }

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
                for (pane_idx, pane) in self.layout.layout().panes().iter().enumerate() {
                    if !self.layout.is_pane_visible(pane.id) {
                        continue;
                    }
                    let rect = pane.pixel_rect(terminal_rect.width, terminal_rect.height);
                    let pane_x = terminal_rect.x as f32 + rect.x as f32;
                    let pane_y = terminal_rect.y as f32 + rect.y as f32;
                    let is_focused = pane.id == focused_pane_id;

                    // Show agent picker inside focused pane if active (non-fullscreen)
                    if let Some(picker) = self.picker.as_ref() {
                        if is_focused {
                            let pane_cols = (rect.width as f32 / cell_w).floor() as usize;
                            let pane_rows = (rect.height as f32 / cell_h).floor() as usize;
                            grids.push((picker.render(pane_rows, pane_cols), pane_x, pane_y));
                            continue;
                        }
                    }

                    // Show session picker inside focused pane if active
                    if let Some(sp) = self.session_picker.as_ref() {
                        if is_focused {
                            let pane_cols = (rect.width as f32 / cell_w).floor() as usize;
                            let pane_rows = (rect.height as f32 / cell_h).floor() as usize;
                            grids.push((sp.render(pane_rows, pane_cols), pane_x, pane_y));
                            continue;
                        }
                    }

                    if let Some(session_id) = pane.session_id {
                        // Pane header — use pane position index, not session list index
                        let label = self
                            .focus_layout
                            .sessions()
                            .entries()
                            .iter()
                            .find(|e| e.id == session_id)
                            .map(|e| e.label.as_str())
                            .unwrap_or("???");
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
                            pane_idx,
                        );
                        grids.push((pane_header, pane_x, pane_y));

                        // Terminal content (offset by 1 row for header)
                        if let Some(terminal) = self.session_mgr.terminal(session_id) {
                            let mut grid = terminal.render_grid();
                            // Hide cursor in non-focused panes
                            if !is_focused || self.session_picker.is_some() {
                                grid.cursor.visible = false;
                            }
                            grids.push((grid, pane_x, pane_y + cell_h));
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
                callbacks.spawn_agent(&agent_type, None);
            } else {
                // No --agent flag: show agent picker TUI
                callbacks.picker = Some(callbacks.create_picker());
                callbacks.picker_fullscreen = true;
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
