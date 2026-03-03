//! Shared types used across all termesh crates.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a terminal session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u64);

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

/// Unique identifier for a pane within a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u64);

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pane-{}", self.0)
    }
}

/// Unique identifier for a project (derived from its canonical path).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub u64);

impl ProjectId {
    /// Create a deterministic project ID from a path.
    ///
    /// Uses FNV-1a over the UTF-8 path bytes for a stable, reproducible
    /// identifier that does not change across Rust versions or process restarts.
    pub fn from_path(path: &std::path::Path) -> Self {
        Self(fnv1a_64(path.as_os_str().as_encoded_bytes()))
    }
}

/// FNV-1a 64-bit hash — deterministic across Rust versions and platforms.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001B3;
    let mut hash = OFFSET_BASIS;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "project-{}", self.0)
    }
}

/// Current state of an AI agent session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    /// No agent attached (plain shell).
    #[default]
    None,
    /// Agent is idle, waiting for user input.
    Idle,
    /// Agent is thinking / analyzing.
    Thinking,
    /// Agent is writing code to a file.
    WritingCode,
    /// Agent is running a shell command.
    RunningCommand,
    /// Agent is waiting for user confirmation (y/n).
    WaitingForInput,
    /// Agent completed successfully.
    Success,
    /// Agent encountered an error.
    Error,
}

impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let icon = match self {
            Self::None => " ",
            Self::Idle => "\u{00B7}",    // · (middle dot)
            Self::Thinking => "*",       // spinner placeholder
            Self::WritingCode => "*",    // spinner placeholder
            Self::RunningCommand => "*", // spinner placeholder
            Self::WaitingForInput => "?",
            Self::Success => "\u{2713}", // ✓
            Self::Error => "\u{2717}",   // ✗
        };
        write!(f, "{icon}")
    }
}

/// Braille spinner frames for animating active agent states.
pub const SPINNER_FRAMES: &[char] = &[
    '\u{280B}', // ⠋
    '\u{2819}', // ⠙
    '\u{2839}', // ⠹
    '\u{2838}', // ⠸
    '\u{283C}', // ⠼
    '\u{2834}', // ⠴
    '\u{2826}', // ⠦
    '\u{2827}', // ⠧
    '\u{2807}', // ⠇
    '\u{280F}', // ⠏
];

impl AgentState {
    /// Whether this state should display an animated spinner.
    pub fn is_spinning(&self) -> bool {
        matches!(
            self,
            Self::Thinking | Self::WritingCode | Self::RunningCommand
        )
    }
}

/// View mode for the UI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    /// Codex-style: session list + full-size terminal + side panel.
    #[default]
    Focus,
    /// tmux-style: 2-4 panes displayed simultaneously.
    Split,
}

/// Split layout arrangement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitLayout {
    /// Two panes side by side.
    Dual,
    /// Four panes in a 2x2 grid.
    #[default]
    Quad,
}

/// Side panel tab types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidePanelTab {
    Diff,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_display() {
        let id = SessionId(42);
        assert_eq!(id.to_string(), "session-42");
    }

    #[test]
    fn test_pane_id_display() {
        let id = PaneId(7);
        assert_eq!(id.to_string(), "pane-7");
    }

    #[test]
    fn test_agent_state_default() {
        assert_eq!(AgentState::default(), AgentState::None);
    }

    #[test]
    fn test_view_mode_default() {
        assert_eq!(ViewMode::default(), ViewMode::Focus);
    }

    #[test]
    fn test_split_layout_default() {
        assert_eq!(SplitLayout::default(), SplitLayout::Quad);
    }

    #[test]
    fn test_project_id_from_path_deterministic() {
        let path = std::path::Path::new("/Users/test/projects/termesh");
        let id1 = ProjectId::from_path(path);
        let id2 = ProjectId::from_path(path);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_project_id_from_path_different() {
        let id1 = ProjectId::from_path(std::path::Path::new("/Users/test/project-a"));
        let id2 = ProjectId::from_path(std::path::Path::new("/Users/test/project-b"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_project_id_display() {
        let id = ProjectId(123);
        assert_eq!(id.to_string(), "project-123");
    }
}
