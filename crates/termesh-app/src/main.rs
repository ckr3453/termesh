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
}

fn main() {
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
