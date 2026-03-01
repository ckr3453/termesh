mod app;
mod cli;
#[allow(dead_code)]
mod session_manager;

use app::App;
use clap::Parser;
use cli::{Cli, Command};
use session_manager::SessionManager;
use termesh_input::action::Action;
use termesh_layout::split_layout::{Direction, SplitLayoutManager};
use termesh_platform::event_loop::{self, AppCallbacks, PlatformConfig};
use termesh_pty::session::SessionConfig;
use termesh_terminal::grid::GridSnapshot;

/// Bridges the platform event loop to the session manager.
struct TermeshCallbacks {
    session_mgr: SessionManager,
    layout: SplitLayoutManager,
    /// Window size in pixels (for layout calculations).
    window_size: (u32, u32),
}

impl TermeshCallbacks {
    fn new() -> Self {
        Self {
            session_mgr: SessionManager::new(),
            layout: SplitLayoutManager::new(termesh_core::types::SplitLayout::Dual),
            window_size: (800, 600),
        }
    }

    /// Spawn a new shell session and bind it to a specific pane.
    fn spawn_and_bind(&mut self, pane_id: termesh_core::types::PaneId) {
        let config = SessionConfig::default();
        match self.session_mgr.spawn(config) {
            Ok(session_id) => {
                self.layout.bind_session(pane_id, session_id);
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
            Action::ToggleSidePanel => log::info!("action: ToggleSidePanel"),
            Action::Copy | Action::Paste => { /* handled by platform layer */ }
        }
    }

    fn on_tick(&mut self) -> Vec<(GridSnapshot, f32, f32)> {
        // Process pending PTY output
        self.session_mgr.process_events();

        let (screen_w, screen_h) = self.window_size;
        let mut grids = Vec::new();

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
        grids
    }

    fn on_resize(&mut self, rows: usize, cols: usize, width: u32, height: u32) {
        self.window_size = (width, height);
        // For now, resize all sessions to the same grid size.
        // Per-pane resize will be implemented in task 036.
        self.session_mgr.resize_all(rows, cols);
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

    let _app = match Cli::parse().command {
        Some(Command::Open { name }) => match App::open_workspace(&name) {
            Ok(app) => app,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        None => App::new(),
    };

    let mut callbacks = TermeshCallbacks::new();
    callbacks.spawn_default_shell();

    let config = PlatformConfig {
        font_size: 14.0,
        scrollback: 10_000,
        input_handler: _app.input().clone(),
        callbacks: Some(Box::new(callbacks)),
    };

    if let Err(e) = event_loop::run(config) {
        eprintln!("Fatal: {e}");
        std::process::exit(1);
    }
}
