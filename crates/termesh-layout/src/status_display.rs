//! Agent status display: visual representation of agent states with spinner animation.

use std::time::{Duration, Instant};
use termesh_core::types::AgentState;

/// Braille spinner frames for active states.
const SPINNER_FRAMES: &[char] = &[
    '\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}',
    '\u{2807}', '\u{280F}',
];

/// How often the spinner advances (100ms).
const SPINNER_INTERVAL: Duration = Duration::from_millis(100);

/// Status display for a single agent session.
#[derive(Debug, Clone)]
pub struct StatusDisplay {
    /// Current agent state.
    state: AgentState,
    /// When the state last changed.
    state_changed_at: Instant,
    /// Current spinner frame index.
    spinner_frame: usize,
    /// When the spinner last advanced.
    last_spin: Instant,
    /// Last file changed by the agent.
    last_changed_file: Option<String>,
    /// Last command run by the agent.
    last_command: Option<CommandResult>,
}

/// Result of the last command run by the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    /// The command string.
    pub command: String,
    /// Whether it succeeded.
    pub success: bool,
}

impl StatusDisplay {
    /// Create a new status display.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            state: AgentState::None,
            state_changed_at: now,
            spinner_frame: 0,
            last_spin: now,
            last_changed_file: None,
            last_command: None,
        }
    }

    /// Update the agent state.
    pub fn set_state(&mut self, state: AgentState) {
        if self.state != state {
            self.state = state;
            self.state_changed_at = Instant::now();
            self.spinner_frame = 0;
        }
    }

    /// Get the current state.
    pub fn state(&self) -> &AgentState {
        &self.state
    }

    /// Record the last file changed.
    pub fn set_last_file(&mut self, path: String) {
        self.last_changed_file = Some(path);
    }

    /// Get the last changed file.
    pub fn last_file(&self) -> Option<&str> {
        self.last_changed_file.as_deref()
    }

    /// Record the last command result.
    pub fn set_last_command(&mut self, command: String, success: bool) {
        self.last_command = Some(CommandResult { command, success });
    }

    /// Get the last command result.
    pub fn last_command(&self) -> Option<&CommandResult> {
        self.last_command.as_ref()
    }

    /// Whether the current state should show a spinner animation.
    pub fn is_spinning(&self) -> bool {
        matches!(
            self.state,
            AgentState::Thinking | AgentState::WritingCode | AgentState::RunningCommand
        )
    }

    /// Advance the spinner if enough time has passed. Returns `true` if the frame changed.
    pub fn tick(&mut self) -> bool {
        if !self.is_spinning() {
            return false;
        }
        let now = Instant::now();
        if now.duration_since(self.last_spin) >= SPINNER_INTERVAL {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
            self.last_spin = now;
            return true;
        }
        false
    }

    /// Get the current spinner character (if spinning).
    pub fn spinner_char(&self) -> Option<char> {
        if self.is_spinning() {
            Some(SPINNER_FRAMES[self.spinner_frame])
        } else {
            None
        }
    }

    /// Get the status icon for the current state.
    pub fn status_icon(&self) -> &str {
        match self.state {
            AgentState::None => "",
            AgentState::Idle => "\u{1F4A4}",                   // 💤
            AgentState::Thinking => "\u{231B}",                // ⌛
            AgentState::WritingCode => "\u{270D}\u{FE0F}",     // ✍️
            AgentState::RunningCommand => "\u{25B6}\u{FE0F}",  // ▶️
            AgentState::WaitingForInput => "\u{23F8}\u{FE0F}", // ⏸️
            AgentState::Success => "\u{2705}",                 // ✅
            AgentState::Error => "\u{2717}",                   // ✗
        }
    }

    /// Get the duration since the last state change.
    pub fn state_duration(&self) -> Duration {
        self.state_changed_at.elapsed()
    }

    /// Format the compact status line for the pane header bar.
    pub fn compact_line(&self) -> String {
        let icon = self.status_icon();
        if let Some(spinner) = self.spinner_char() {
            format!("{icon} {spinner}")
        } else if icon.is_empty() {
            String::new()
        } else {
            icon.to_string()
        }
    }

    /// Format detailed status lines for the Focus mode side panel.
    pub fn detail_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        // State + duration
        let dur = self.state_duration().as_secs();
        let state_name = match self.state {
            AgentState::None => "Shell",
            AgentState::Idle => "Idle",
            AgentState::Thinking => "Thinking",
            AgentState::WritingCode => "Writing Code",
            AgentState::RunningCommand => "Running Command",
            AgentState::WaitingForInput => "Waiting for Input",
            AgentState::Success => "Completed",
            AgentState::Error => "Error",
        };
        lines.push(format!("{} {} ({}s)", self.status_icon(), state_name, dur));

        // Last file
        if let Some(file) = &self.last_changed_file {
            lines.push(format!("Last file: {file}"));
        }

        // Last command
        if let Some(cmd) = &self.last_command {
            let result = if cmd.success { "ok" } else { "FAIL" };
            lines.push(format!("Last cmd: {} [{}]", cmd.command, result));
        }

        lines
    }
}

impl Default for StatusDisplay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_default_state() {
        let status = StatusDisplay::new();
        assert_eq!(*status.state(), AgentState::None);
        assert!(!status.is_spinning());
        assert!(status.spinner_char().is_none());
    }

    #[test]
    fn test_set_state() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::Thinking);
        assert_eq!(*status.state(), AgentState::Thinking);
        assert!(status.is_spinning());
    }

    #[test]
    fn test_spinning_states() {
        let mut status = StatusDisplay::new();

        for state in [
            AgentState::Thinking,
            AgentState::WritingCode,
            AgentState::RunningCommand,
        ] {
            status.set_state(state);
            assert!(status.is_spinning(), "should spin for {state:?}");
        }

        for state in [
            AgentState::None,
            AgentState::Idle,
            AgentState::WaitingForInput,
            AgentState::Success,
            AgentState::Error,
        ] {
            status.set_state(state);
            assert!(!status.is_spinning(), "should not spin for {state:?}");
        }
    }

    #[test]
    fn test_spinner_char_when_spinning() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::Thinking);
        let ch = status.spinner_char().unwrap();
        assert!(SPINNER_FRAMES.contains(&ch));
    }

    #[test]
    fn test_tick_no_advance_when_not_spinning() {
        let mut status = StatusDisplay::new();
        assert!(!status.tick());
    }

    #[test]
    fn test_tick_advances_spinner() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::Thinking);
        // Force last_spin to be old enough
        status.last_spin = Instant::now() - Duration::from_millis(200);
        assert!(status.tick());
        assert_eq!(status.spinner_frame, 1);
    }

    #[test]
    fn test_tick_wraps_spinner() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::Thinking);
        status.spinner_frame = SPINNER_FRAMES.len() - 1;
        status.last_spin = Instant::now() - Duration::from_millis(200);
        status.tick();
        assert_eq!(status.spinner_frame, 0);
    }

    #[test]
    fn test_status_icon() {
        let mut status = StatusDisplay::new();
        assert_eq!(status.status_icon(), "");

        status.set_state(AgentState::Success);
        assert!(!status.status_icon().is_empty());

        status.set_state(AgentState::Error);
        assert!(status.status_icon().contains('\u{2717}'));
    }

    #[test]
    fn test_last_file() {
        let mut status = StatusDisplay::new();
        assert!(status.last_file().is_none());

        status.set_last_file("src/main.rs".to_string());
        assert_eq!(status.last_file(), Some("src/main.rs"));
    }

    #[test]
    fn test_last_command() {
        let mut status = StatusDisplay::new();
        assert!(status.last_command().is_none());

        status.set_last_command("cargo test".to_string(), true);
        let cmd = status.last_command().unwrap();
        assert_eq!(cmd.command, "cargo test");
        assert!(cmd.success);
    }

    #[test]
    fn test_compact_line_no_state() {
        let status = StatusDisplay::new();
        assert_eq!(status.compact_line(), "");
    }

    #[test]
    fn test_compact_line_with_spinner() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::Thinking);
        let line = status.compact_line();
        assert!(!line.is_empty());
    }

    #[test]
    fn test_compact_line_static_state() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::Success);
        let line = status.compact_line();
        assert!(!line.is_empty());
        assert!(!line.contains(SPINNER_FRAMES[0]));
    }

    #[test]
    fn test_detail_lines() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::WritingCode);
        status.set_last_file("lib.rs".to_string());
        status.set_last_command("cargo build".to_string(), false);

        let lines = status.detail_lines();
        assert!(lines.len() >= 3);
        assert!(lines[0].contains("Writing Code"));
        assert!(lines[1].contains("lib.rs"));
        assert!(lines[2].contains("FAIL"));
    }

    #[test]
    fn test_detail_lines_minimal() {
        let status = StatusDisplay::new();
        let lines = status.detail_lines();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Shell"));
    }

    #[test]
    fn test_state_change_resets_spinner() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::Thinking);
        status.spinner_frame = 5;

        status.set_state(AgentState::WritingCode);
        assert_eq!(status.spinner_frame, 0);
    }

    #[test]
    fn test_same_state_no_reset() {
        let mut status = StatusDisplay::new();
        status.set_state(AgentState::Thinking);
        status.spinner_frame = 5;
        let ts = status.state_changed_at;

        status.set_state(AgentState::Thinking);
        assert_eq!(status.spinner_frame, 5); // not reset
        assert_eq!(status.state_changed_at, ts); // not updated
    }
}
