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
    /// Navigate to the pane on the left.
    NavigateLeft,
    /// Navigate to the pane below.
    NavigateDown,
    /// Navigate to the pane above.
    NavigateUp,
    /// Navigate to the pane on the right.
    NavigateRight,
    /// Toggle between Focus and Split mode.
    ToggleMode,
    /// Focus the next pane.
    FocusNext,
    /// Focus the previous pane.
    FocusPrev,
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
