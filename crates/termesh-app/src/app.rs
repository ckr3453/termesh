//! Application state and lifecycle.

use termesh_core::types::ViewMode;
use termesh_layout::focus_layout::FocusLayout;
use termesh_layout::session_list::SessionEntry;
use termesh_layout::split_layout::SplitLayoutManager;

use termesh_agent::preset::WorkspacePreset;
use termesh_agent::workspace::WorkspaceLoader;
use termesh_core::types::{AgentState, SessionId, SidePanelTab, SplitLayout};
use termesh_input::handler::InputHandler;

/// Application initialization error.
#[derive(Debug)]
pub enum AppError {
    /// Configuration loading failed.
    Config(String),
    /// Workspace preset not found or invalid.
    Workspace(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "config error: {msg}"),
            Self::Workspace(msg) => write!(f, "workspace error: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

/// The main application state.
#[allow(dead_code)]
pub struct App {
    /// Current view mode.
    view_mode: ViewMode,
    /// Focus mode layout (active when view_mode == Focus).
    focus_layout: FocusLayout,
    /// Split mode layout (active when view_mode == Split).
    split_layout: SplitLayoutManager,
    /// Input handler for keybindings.
    input: InputHandler,
    /// Next session ID counter.
    next_session_id: u64,
}

#[allow(dead_code)]
impl App {
    /// Create a new application with default settings.
    pub fn new() -> Self {
        Self {
            view_mode: ViewMode::Focus,
            focus_layout: FocusLayout::new(),
            split_layout: SplitLayoutManager::new(SplitLayout::Quad),
            input: InputHandler::new(),
            next_session_id: 1,
        }
    }

    /// Create an application from a workspace preset.
    pub fn from_preset(preset: &WorkspacePreset) -> Self {
        let view_mode = match preset.default_mode.as_str() {
            "split" => ViewMode::Split,
            _ => ViewMode::Focus,
        };

        let mut focus_layout = match &preset.side_panel {
            Some(panel) if panel == "diff" => FocusLayout::with_side_panel(SidePanelTab::Diff),
            _ => FocusLayout::new(),
        };

        let mut next_id = 1u64;
        for pane in &preset.panes {
            let is_agent = pane
                .command
                .as_deref()
                .map(|c| c.contains("claude") || c.contains("codex"))
                .unwrap_or(false);

            focus_layout.sessions_mut().add(SessionEntry {
                id: SessionId(next_id),
                label: pane.label.clone(),
                is_agent,
                state: if is_agent {
                    AgentState::Idle
                } else {
                    AgentState::None
                },
            });
            next_id += 1;
        }

        Self {
            view_mode,
            focus_layout,
            split_layout: SplitLayoutManager::new(SplitLayout::Quad),
            input: InputHandler::new(),
            next_session_id: next_id,
        }
    }

    /// Load a workspace preset by name and create the app.
    pub fn open_workspace(name: &str) -> Result<Self, AppError> {
        let loader = WorkspaceLoader::default_dir()
            .ok_or_else(|| AppError::Config("cannot determine config directory".into()))?;

        let preset = loader
            .load(name)
            .map_err(|e| AppError::Workspace(e.to_string()))?;

        Ok(Self::from_preset(&preset))
    }

    /// Get the current view mode.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Toggle between Focus and Split mode.
    pub fn toggle_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Focus => ViewMode::Split,
            ViewMode::Split => ViewMode::Focus,
        };
    }

    /// Get the Focus layout.
    pub fn focus_layout(&self) -> &FocusLayout {
        &self.focus_layout
    }

    /// Get a mutable reference to the Focus layout.
    pub fn focus_layout_mut(&mut self) -> &mut FocusLayout {
        &mut self.focus_layout
    }

    /// Get the Split layout.
    pub fn split_layout(&self) -> &SplitLayoutManager {
        &self.split_layout
    }

    /// Get a mutable reference to the Split layout.
    pub fn split_layout_mut(&mut self) -> &mut SplitLayoutManager {
        &mut self.split_layout
    }

    /// Get the input handler.
    pub fn input(&self) -> &InputHandler {
        &self.input
    }

    /// Allocate a new session ID.
    pub fn alloc_session_id(&mut self) -> SessionId {
        let id = SessionId(self.next_session_id);
        self.next_session_id += 1;
        id
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_default() {
        let app = App::new();
        assert_eq!(app.view_mode(), ViewMode::Focus);
        assert!(app.focus_layout().sessions().is_empty());
    }

    #[test]
    fn test_toggle_mode() {
        let mut app = App::new();
        assert_eq!(app.view_mode(), ViewMode::Focus);
        app.toggle_mode();
        assert_eq!(app.view_mode(), ViewMode::Split);
        app.toggle_mode();
        assert_eq!(app.view_mode(), ViewMode::Focus);
    }

    #[test]
    fn test_alloc_session_id() {
        let mut app = App::new();
        let id1 = app.alloc_session_id();
        let id2 = app.alloc_session_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_from_preset() {
        use std::collections::HashMap;
        use termesh_agent::preset::PanePreset;

        let preset = WorkspacePreset {
            name: "Test".to_string(),
            default_mode: "split".to_string(),
            side_panel: Some("diff".to_string()),
            panes: vec![
                PanePreset {
                    label: "Backend".to_string(),
                    cwd: None,
                    command: Some("claude".to_string()),
                    role: Some("API dev".to_string()),
                    env: HashMap::new(),
                },
                PanePreset {
                    label: "Shell".to_string(),
                    cwd: None,
                    command: Some("bash".to_string()),
                    role: None,
                    env: HashMap::new(),
                },
            ],
        };

        let app = App::from_preset(&preset);
        assert_eq!(app.view_mode(), ViewMode::Split);
        assert_eq!(app.focus_layout().sessions().len(), 2);

        let entries = app.focus_layout().sessions().entries();
        assert_eq!(entries[0].label, "Backend");
        assert!(entries[0].is_agent);
        assert_eq!(entries[0].state, AgentState::Idle);

        assert_eq!(entries[1].label, "Shell");
        assert!(!entries[1].is_agent);
        assert_eq!(entries[1].state, AgentState::None);
    }

    #[test]
    fn test_open_workspace_not_found() {
        let result = App::open_workspace("nonexistent_workspace_xyz");
        assert!(result.is_err());
    }
}
