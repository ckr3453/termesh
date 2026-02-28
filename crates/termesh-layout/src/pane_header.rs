//! Pane header bar: displays session name and status icon above each pane.

use termesh_core::types::AgentState;

/// Height of the pane header bar in pixels.
pub const HEADER_HEIGHT: u32 = 24;

/// Header bar content for a single pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneHeader {
    /// Session display label.
    pub label: String,
    /// Agent state icon.
    pub state: AgentState,
    /// Whether this pane currently has focus.
    pub focused: bool,
}

impl PaneHeader {
    /// Create a new header.
    pub fn new(label: String, state: AgentState, focused: bool) -> Self {
        Self {
            label,
            state,
            focused,
        }
    }

    /// Format the header text for display.
    pub fn display_text(&self) -> String {
        match self.state {
            AgentState::None => self.label.clone(),
            ref state => format!("{} {state}", self.label),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_display_shell() {
        let header = PaneHeader::new("Shell".to_string(), AgentState::None, false);
        assert_eq!(header.display_text(), "Shell");
    }

    #[test]
    fn test_header_display_agent() {
        let header = PaneHeader::new("Backend".to_string(), AgentState::Thinking, true);
        let text = header.display_text();
        assert!(text.starts_with("Backend"));
        // Should contain the Thinking icon from AgentState::Display
        assert!(text.len() > "Backend".len());
    }

    #[test]
    fn test_header_focused() {
        let header = PaneHeader::new("Dev".to_string(), AgentState::Idle, true);
        assert!(header.focused);
    }
}
