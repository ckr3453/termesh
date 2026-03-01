mod app;
mod cli;
#[allow(dead_code)]
mod session_manager;

use app::App;
use clap::Parser;
use cli::{Cli, Command};
use session_manager::SessionManager;
use termesh_input::action::Action;
use termesh_platform::event_loop::{self, AppCallbacks, PlatformConfig};
use termesh_pty::session::SessionConfig;
use termesh_terminal::grid::GridSnapshot;

/// Bridges the platform event loop to the session manager.
struct TermeshCallbacks {
    session_mgr: SessionManager,
}

impl TermeshCallbacks {
    fn new() -> Self {
        Self {
            session_mgr: SessionManager::new(),
        }
    }

    /// Spawn the default shell session.
    fn spawn_default_shell(&mut self) {
        let config = SessionConfig::default();
        if let Err(e) = self.session_mgr.spawn(config) {
            log::error!("failed to spawn default shell: {e}");
        }
    }
}

impl AppCallbacks for TermeshCallbacks {
    fn on_input(&mut self, text: &[u8]) {
        let _ = self.session_mgr.write_active(text);
    }

    fn on_action(&mut self, action: Action) {
        match action {
            Action::ToggleMode => log::info!("action: ToggleMode"),
            Action::ToggleSidePanel => log::info!("action: ToggleSidePanel"),
            Action::NavigateLeft => log::info!("action: NavigateLeft"),
            Action::NavigateDown => log::info!("action: NavigateDown"),
            Action::NavigateUp => log::info!("action: NavigateUp"),
            Action::NavigateRight => log::info!("action: NavigateRight"),
            Action::FocusNext => log::info!("action: FocusNext"),
            Action::FocusPrev => log::info!("action: FocusPrev"),
            Action::SplitHorizontal => log::info!("action: SplitHorizontal"),
            Action::SplitVertical => log::info!("action: SplitVertical"),
            Action::ClosePane => log::info!("action: ClosePane"),
            Action::Copy | Action::Paste => { /* handled by platform layer */ }
        }
    }

    fn on_tick(&mut self) -> Vec<(GridSnapshot, f32, f32)> {
        // Process pending PTY output
        self.session_mgr.process_events();

        // Return active session's grid at (0,0) for now
        let mut grids = Vec::new();
        if let Some(id) = self.session_mgr.active() {
            if let Some(terminal) = self.session_mgr.terminal(id) {
                grids.push((terminal.render_grid(), 0.0, 0.0));
            }
        }
        grids
    }

    fn on_resize(&mut self, rows: usize, cols: usize) {
        self.session_mgr.resize_all(rows, cols);
    }

    fn on_scroll(&mut self, delta: i32) {
        if let Some(id) = self.session_mgr.active() {
            if let Some(terminal) = self.session_mgr.terminal_mut(id) {
                if delta > 0 {
                    terminal.scroll_up(delta as usize);
                } else {
                    terminal.scroll_down((-delta) as usize);
                }
            }
        }
    }

    fn on_mouse_press(&mut self, row: usize, col: usize) {
        if let Some(id) = self.session_mgr.active() {
            if let Some(terminal) = self.session_mgr.terminal_mut(id) {
                terminal.selection_start(row, col);
            }
        }
    }

    fn on_mouse_drag(&mut self, row: usize, col: usize) {
        if let Some(id) = self.session_mgr.active() {
            if let Some(terminal) = self.session_mgr.terminal_mut(id) {
                terminal.selection_update(row, col);
            }
        }
    }

    fn on_mouse_release(&mut self) {
        // Selection stays visible until next click or explicit clear
    }

    fn on_copy(&mut self) -> Option<String> {
        let id = self.session_mgr.active()?;
        let terminal = self.session_mgr.terminal(id)?;
        terminal.selected_text()
    }

    fn on_paste(&mut self, text: &str) {
        let _ = self.session_mgr.write_active(text.as_bytes());
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
