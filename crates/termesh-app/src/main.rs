mod app;
mod cli;
#[allow(dead_code)]
mod session_manager;
mod ui_grid;

use app::App;
use clap::Parser;
use cli::{Cli, Command};
use session_manager::SessionManager;
use std::path::PathBuf;
use termesh_agent::preset::WorkspacePreset;
use termesh_core::types::{AgentState, SplitLayout, ViewMode};
use termesh_input::action::Action;
use termesh_layout::focus_layout::FocusLayout;
use termesh_layout::session_list::SessionEntry;
use termesh_layout::split_layout::{Direction, SplitLayoutManager};
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
}

impl TermeshCallbacks {
    fn new() -> Self {
        Self {
            session_mgr: SessionManager::new(),
            layout: SplitLayoutManager::new(SplitLayout::Dual),
            focus_layout: FocusLayout::new(),
            view_mode: ViewMode::Split,
            window_size: (800, 600),
            cell_size: (8.0, 16.0),
            show_session_list: true,
        }
    }

    /// Create callbacks from a workspace preset, spawning all preset sessions.
    fn from_preset(preset: &WorkspacePreset) -> Self {
        let split = match preset.panes.len() {
            1 => SplitLayout::Dual,
            2 => SplitLayout::Dual,
            3 => SplitLayout::Triple,
            _ => SplitLayout::Quad,
        };

        let view_mode = match preset.default_mode.as_str() {
            "focus" => ViewMode::Focus,
            _ => ViewMode::Split,
        };

        let mut callbacks = Self {
            session_mgr: SessionManager::new(),
            layout: SplitLayoutManager::new(split),
            focus_layout: FocusLayout::new(),
            view_mode,
            window_size: (800, 600),
            cell_size: (8.0, 16.0),
            show_session_list: true,
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

    /// Spawn a new shell session and bind it to a specific pane.
    fn spawn_and_bind(&mut self, pane_id: termesh_core::types::PaneId) {
        let config = SessionConfig::default();
        match self.session_mgr.spawn(config) {
            Ok(session_id) => {
                self.layout.bind_session(pane_id, session_id);
                self.focus_layout.sessions_mut().add(SessionEntry {
                    id: session_id,
                    label: format!("Shell {}", session_id.0),
                    is_agent: false,
                    state: AgentState::None,
                });
            }
            Err(e) => {
                log::error!("failed to spawn shell: {e}");
            }
        }
    }

    /// Spawn the default shell session in the first pane.
    fn spawn_default_shell(&mut self) {
        let first_pane_id = self.layout.layout().panes()[0].id;
        self.spawn_and_bind(first_pane_id);
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

    /// Get the session ID bound to the focused pane.
    fn focused_session(&self) -> Option<termesh_core::types::SessionId> {
        self.layout.layout().focused_pane().session_id
    }
}

impl AppCallbacks for TermeshCallbacks {
    fn on_input(&mut self, text: &[u8]) {
        // Send input to the session bound to the focused pane
        if let Some(session_id) = self.focused_session() {
            let _ = self.session_mgr.write_to(session_id, text);
        }
    }

    fn on_action(&mut self, action: Action) {
        match action {
            Action::SplitHorizontal => {
                let new_pane_id = self.layout.layout_mut().split_horizontal();
                self.spawn_and_bind(new_pane_id);
            }
            Action::SplitVertical => {
                let new_pane_id = self.layout.layout_mut().split_vertical();
                self.spawn_and_bind(new_pane_id);
            }
            Action::ClosePane => {
                let focused = self.layout.layout().focused_pane().clone();
                if let Some(session_id) = focused.session_id {
                    self.session_mgr.remove(session_id);
                    self.focus_layout.sessions_mut().remove(session_id);
                }
                self.layout.layout_mut().close_pane(focused.id);
            }
            Action::FocusNext => {
                self.layout.focus_next();
            }
            Action::FocusPrev => {
                self.layout.focus_prev();
            }
            Action::NavigateLeft => {
                let (w, h) = self.window_size;
                self.layout.focus_direction(Direction::Left, w, h);
            }
            Action::NavigateDown => {
                let (w, h) = self.window_size;
                self.layout.focus_direction(Direction::Down, w, h);
            }
            Action::NavigateUp => {
                let (w, h) = self.window_size;
                self.layout.focus_direction(Direction::Up, w, h);
            }
            Action::NavigateRight => {
                let (w, h) = self.window_size;
                self.layout.focus_direction(Direction::Right, w, h);
            }
            Action::ToggleMode => {
                self.layout.toggle_zoom();
            }
            Action::ToggleSidePanel => {
                self.show_session_list = !self.show_session_list;
            }
            Action::Copy | Action::Paste => { /* handled by platform layer */ }
        }
    }

    fn on_tick(&mut self) -> Vec<(GridSnapshot, f32, f32)> {
        // Process pending PTY output
        self.session_mgr.process_events();

        // Sync agent states to session list
        self.sync_agent_states();

        let (screen_w, screen_h) = self.window_size;
        let (cell_w, cell_h) = self.cell_size;
        let mut grids = Vec::new();

        // Render session list panel if visible
        if self.show_session_list {
            let regions = self.focus_layout.compute_regions(screen_w, screen_h);
            let list_rect = regions.session_list;
            if list_rect.width > 0 && list_rect.height > 0 {
                let list_cols = (list_rect.width as f32 / cell_w).floor() as usize;
                let list_rows = (list_rect.height as f32 / cell_h).floor() as usize;
                let list_grid = ui_grid::render_session_list(
                    self.focus_layout.sessions(),
                    list_rows,
                    list_cols,
                );
                grids.push((list_grid, list_rect.x as f32, list_rect.y as f32));
            }

            // Terminal grids offset by session list width
            let terminal_x_offset = list_rect.width as f32;
            for pane in self.layout.layout().panes() {
                if !self.layout.is_pane_visible(pane.id) {
                    continue;
                }
                if let Some(session_id) = pane.session_id {
                    if let Some(terminal) = self.session_mgr.terminal(session_id) {
                        let rect =
                            pane.pixel_rect(screen_w.saturating_sub(list_rect.width), screen_h);
                        grids.push((
                            terminal.render_grid(),
                            terminal_x_offset + rect.x as f32,
                            rect.y as f32,
                        ));
                    }
                }
            }
        } else {
            // No session list — full-width terminal grids
            for pane in self.layout.layout().panes() {
                if !self.layout.is_pane_visible(pane.id) {
                    continue;
                }
                if let Some(session_id) = pane.session_id {
                    if let Some(terminal) = self.session_mgr.terminal(session_id) {
                        let rect = pane.pixel_rect(screen_w, screen_h);
                        grids.push((terminal.render_grid(), rect.x as f32, rect.y as f32));
                    }
                }
            }
        }
        grids
    }

    fn dividers(&self) -> Vec<(f32, f32, f32, bool)> {
        let (w, h) = self.window_size;
        let x_offset = if self.show_session_list {
            let regions = self.focus_layout.compute_regions(w, h);
            regions.session_list.width as f32
        } else {
            0.0
        };

        let terminal_w = w - x_offset as u32;
        let mut dividers: Vec<(f32, f32, f32, bool)> = self
            .layout
            .compute_dividers(terminal_w, h)
            .iter()
            .map(|d| {
                let is_vertical =
                    d.orientation == termesh_layout::split_layout::DividerOrientation::Vertical;
                (
                    x_offset + d.x as f32,
                    d.y as f32,
                    d.length as f32,
                    is_vertical,
                )
            })
            .collect();

        // Add a vertical divider between session list and terminal area
        if self.show_session_list && x_offset > 0.0 {
            dividers.push((x_offset, 0.0, h as f32, true));
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
        self.window_size = (width, height);
        self.cell_size = (cell_w, cell_h);

        // Compute available width for terminal panes (subtract session list width)
        let terminal_width = if self.show_session_list {
            let regions = self.focus_layout.compute_regions(width, height);
            width.saturating_sub(regions.session_list.width)
        } else {
            width
        };

        // Per-pane resize: each pane gets its own grid dimensions
        for pane in self.layout.layout().panes().to_vec() {
            if let Some(session_id) = pane.session_id {
                let (rows, cols) = pane.grid_size(terminal_width, height, cell_w, cell_h);
                self.session_mgr.resize(session_id, rows, cols);
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
        self.session_mgr.is_empty()
    }
}

fn main() {
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

    let (app, callbacks) = match Cli::parse().command {
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
            callbacks.spawn_default_shell();
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
