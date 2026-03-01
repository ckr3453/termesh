//! Actions that can be triggered by keybindings.

use serde::{Deserialize, Serialize};

/// Actions dispatched by the keybinding engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    /// Split the focused pane horizontally (left/right).
    SplitHorizontal,
    /// Split the focused pane vertically (top/bottom).
    SplitVertical,
    /// Close the focused pane.
    ClosePane,
    /// Toggle the side panel.
    ToggleSidePanel,
    /// Focus pane 1 (top-left / left).
    FocusPane1,
    /// Focus pane 2 (top-right / right).
    FocusPane2,
    /// Focus pane 3 (bottom-left).
    FocusPane3,
    /// Focus pane 4 (bottom-right).
    FocusPane4,
    /// Focus pane 5 (session index 5).
    FocusPane5,
    /// Focus pane 6 (session index 6).
    FocusPane6,
    /// Focus pane 7 (session index 7).
    FocusPane7,
    /// Focus pane 8 (session index 8).
    FocusPane8,
    /// Focus pane 9 (session index 9).
    FocusPane9,
    /// Toggle between Focus and Split mode.
    ToggleMode,
    /// Focus the next pane.
    FocusNext,
    /// Focus the previous pane.
    FocusPrev,
    /// Copy selected text to clipboard.
    Copy,
    /// Paste clipboard contents to PTY.
    Paste,
    /// Spawn a new session (Focus: add to list, Split: split + spawn).
    SpawnSession,
    /// Rename the currently selected session.
    RenameSession,
    /// Toggle the session list panel visibility.
    ToggleSessionList,
    /// Scroll side panel up.
    SidePanelScrollUp,
    /// Scroll side panel down.
    SidePanelScrollDown,
    /// Switch to the next side panel tab.
    SidePanelNextTab,
    /// Switch to the previous side panel tab.
    SidePanelPrevTab,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_equality() {
        assert_eq!(Action::SplitHorizontal, Action::SplitHorizontal);
        assert_ne!(Action::SplitHorizontal, Action::SplitVertical);
    }
}
