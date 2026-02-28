//! TOML-based configuration system.

use crate::error::ConfigError;
use crate::types::{SidePanelTab, SplitLayout, ViewMode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub terminal: TerminalConfig,

    #[serde(default)]
    pub keybindings: KeybindingsConfig,

    #[serde(default)]
    pub daemon: DaemonConfig,
}

/// Terminal emulation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Default shell to spawn (e.g., "zsh", "bash").
    #[serde(default = "default_shell")]
    pub default_shell: String,

    /// Font size in points.
    #[serde(default = "default_font_size")]
    pub font_size: u16,

    /// Color theme name.
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Number of scrollback lines to keep.
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: u32,

    /// Enable GPU-accelerated rendering.
    #[serde(default = "default_true")]
    pub gpu_rendering: bool,

    /// Default view mode.
    #[serde(default)]
    pub default_mode: ViewMode,

    /// Default split layout.
    #[serde(default)]
    pub split_layout: SplitLayout,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            default_shell: default_shell(),
            font_size: default_font_size(),
            theme: default_theme(),
            scrollback_lines: default_scrollback_lines(),
            gpu_rendering: true,
            default_mode: ViewMode::default(),
            split_layout: SplitLayout::default(),
        }
    }
}

/// Keybinding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    #[serde(default = "default_bind_new_pane_h")]
    pub new_pane_horizontal: String,

    #[serde(default = "default_bind_new_pane_v")]
    pub new_pane_vertical: String,

    #[serde(default = "default_bind_close_pane")]
    pub close_pane: String,

    #[serde(default = "default_bind_toggle_side_panel")]
    pub toggle_side_panel: String,

    #[serde(default = "default_bind_nav_left")]
    pub navigate_left: String,

    #[serde(default = "default_bind_nav_down")]
    pub navigate_down: String,

    #[serde(default = "default_bind_nav_up")]
    pub navigate_up: String,

    #[serde(default = "default_bind_nav_right")]
    pub navigate_right: String,

    #[serde(default = "default_bind_toggle_mode")]
    pub toggle_mode: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            new_pane_horizontal: default_bind_new_pane_h(),
            new_pane_vertical: default_bind_new_pane_v(),
            close_pane: default_bind_close_pane(),
            toggle_side_panel: default_bind_toggle_side_panel(),
            navigate_left: default_bind_nav_left(),
            navigate_down: default_bind_nav_down(),
            navigate_up: default_bind_nav_up(),
            navigate_right: default_bind_nav_right(),
            toggle_mode: default_bind_toggle_mode(),
        }
    }
}

/// Daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Path to the Unix domain socket.
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
        }
    }
}

/// Workspace preset for launching multiple sessions at once.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePreset {
    pub workspace: WorkspaceConfig,
}

/// Workspace configuration section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,

    #[serde(default)]
    pub default_mode: ViewMode,

    #[serde(default)]
    pub split_layout: SplitLayout,

    #[serde(default)]
    pub panes: Vec<PanePreset>,

    #[serde(default)]
    pub side_panel: Option<SidePanelConfig>,
}

/// Pane preset within a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanePreset {
    pub name: String,

    #[serde(default = "default_agent_none")]
    pub agent: String,

    #[serde(default)]
    pub cwd: Option<String>,

    pub command: String,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub role: Option<String>,
}

/// Side panel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidePanelConfig {
    #[serde(default = "default_true")]
    pub show: bool,

    #[serde(default)]
    pub panels: Vec<SidePanelTab>,
}

// --- Config loading ---

impl Config {
    /// Load configuration from the default path.
    ///
    /// Looks for `~/.config/termesh/config.toml`. Falls back to defaults
    /// if the file does not exist.
    pub fn load() -> Result<Self, ConfigError> {
        let path = config_dir().join("config.toml");
        if path.exists() {
            Self::load_from(&path)
        } else {
            Ok(Self::default())
        }
    }

    /// Load configuration from a specific file path.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|_| ConfigError::NotFound {
            path: path.to_path_buf(),
        })?;
        toml::from_str(&content).map_err(|e| ConfigError::Parse { source: e })
    }
}

impl WorkspacePreset {
    /// Load a workspace preset by name from `~/.config/termesh/workspaces/`.
    pub fn load_by_name(name: &str) -> Result<Self, ConfigError> {
        let path = config_dir().join("workspaces").join(format!("{name}.toml"));
        Self::load_from(&path)
    }

    /// Load a workspace preset from a specific file path.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|_| ConfigError::NotFound {
            path: path.to_path_buf(),
        })?;
        toml::from_str(&content).map_err(|e| ConfigError::Parse { source: e })
    }
}

/// Returns the termesh config directory (`~/.config/termesh/`).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("termesh")
}

// --- Default value functions ---

fn default_shell() -> String {
    #[cfg(windows)]
    {
        "powershell".to_string()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
    }
}

fn default_font_size() -> u16 {
    14
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_scrollback_lines() -> u32 {
    10_000
}

fn default_true() -> bool {
    true
}

fn default_socket_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".termesh")
        .join("termesh.sock")
}

fn default_agent_none() -> String {
    "none".to_string()
}

fn default_bind_new_pane_h() -> String {
    "Cmd+T".to_string()
}
fn default_bind_new_pane_v() -> String {
    "Cmd+Shift+T".to_string()
}
fn default_bind_close_pane() -> String {
    "Cmd+W".to_string()
}
fn default_bind_toggle_side_panel() -> String {
    "Cmd+E".to_string()
}
fn default_bind_nav_left() -> String {
    "Cmd+H".to_string()
}
fn default_bind_nav_down() -> String {
    "Cmd+J".to_string()
}
fn default_bind_nav_up() -> String {
    "Cmd+K".to_string()
}
fn default_bind_nav_right() -> String {
    "Cmd+L".to_string()
}
fn default_bind_toggle_mode() -> String {
    "Cmd+Enter".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.terminal.font_size, 14);
        assert_eq!(config.terminal.scrollback_lines, 10_000);
        assert!(config.terminal.gpu_rendering);
        assert_eq!(config.terminal.default_mode, ViewMode::Focus);
    }

    #[test]
    fn test_parse_minimal_toml() {
        let toml_str = r#"
[terminal]
font_size = 16
theme = "light"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.terminal.font_size, 16);
        assert_eq!(config.terminal.theme, "light");
        // defaults should be applied for missing fields
        assert_eq!(config.terminal.scrollback_lines, 10_000);
    }

    #[test]
    fn test_parse_full_toml() {
        let toml_str = r#"
[terminal]
default_shell = "fish"
font_size = 12
theme = "nord"
scrollback_lines = 5000
gpu_rendering = false
default_mode = "Split"
split_layout = "dual"

[keybindings]
new_pane_horizontal = "Ctrl+T"
close_pane = "Ctrl+W"

[daemon]
socket_path = "/tmp/termesh.sock"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.terminal.default_shell, "fish");
        assert_eq!(config.terminal.font_size, 12);
        assert!(!config.terminal.gpu_rendering);
        assert_eq!(config.terminal.default_mode, ViewMode::Split);
        assert_eq!(config.terminal.split_layout, SplitLayout::Dual);
        assert_eq!(config.keybindings.new_pane_horizontal, "Ctrl+T");
        assert_eq!(
            config.daemon.socket_path,
            PathBuf::from("/tmp/termesh.sock")
        );
    }

    #[test]
    fn test_parse_workspace_preset() {
        let toml_str = r#"
[workspace]
name = "sigma-ai"
default_mode = "Focus"
split_layout = "quad"

[[workspace.panes]]
name = "backend"
agent = "claude"
cwd = "~/projects/sigma-ai/backend"
command = "claude"
role = "백엔드 API 개발"

[[workspace.panes]]
name = "shell"
agent = "none"
command = "zsh"
"#;
        let preset: WorkspacePreset = toml::from_str(toml_str).unwrap();
        assert_eq!(preset.workspace.name, "sigma-ai");
        assert_eq!(preset.workspace.panes.len(), 2);
        assert_eq!(preset.workspace.panes[0].agent, "claude");
        assert_eq!(preset.workspace.panes[1].agent, "none");
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let config = Config::load();
        // Should return Ok with defaults even if file doesn't exist
        assert!(config.is_ok());
    }

    #[test]
    fn test_load_from_invalid_path() {
        let result = Config::load_from(Path::new("/nonexistent/config.toml"));
        assert!(result.is_err());
    }
}
