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
    Preview,
    TestLog,
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
}
