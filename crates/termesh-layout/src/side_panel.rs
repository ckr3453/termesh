//! Side panel: manages the right-side panel with Diff/Preview/TestLog tabs.

use termesh_core::types::SidePanelTab;

/// Manages the side panel state in Focus mode.
#[derive(Debug, Clone)]
pub struct SidePanel {
    /// Whether the panel is visible.
    visible: bool,
    /// Available tabs.
    tabs: Vec<SidePanelTab>,
    /// Currently active tab index.
    active_tab: usize,
}

impl SidePanel {
    /// Create a new side panel with default tabs (Diff only).
    pub fn new() -> Self {
        Self {
            visible: false,
            tabs: vec![SidePanelTab::Diff],
            active_tab: 0,
        }
    }

    /// Create a side panel with specific tabs and initial visibility.
    pub fn with_tabs(tabs: Vec<SidePanelTab>, visible: bool) -> Self {
        Self {
            visible,
            tabs,
            active_tab: 0,
        }
    }

    /// Toggle panel visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Show the panel.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the panel.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Whether the panel is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Switch to the next tab (wrapping around).
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    /// Switch to the previous tab (wrapping around).
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = if self.active_tab == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab - 1
            };
        }
    }

    /// Set the active tab by type.
    pub fn set_active(&mut self, tab: SidePanelTab) {
        if let Some(idx) = self.tabs.iter().position(|t| *t == tab) {
            self.active_tab = idx;
        }
    }

    /// Get the currently active tab.
    pub fn active_tab(&self) -> Option<SidePanelTab> {
        self.tabs.get(self.active_tab).copied()
    }

    /// Get all available tabs.
    pub fn tabs(&self) -> &[SidePanelTab] {
        &self.tabs
    }

    /// Get the active tab index.
    pub fn active_index(&self) -> usize {
        self.active_tab
    }
}

impl Default for SidePanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_hidden() {
        let panel = SidePanel::new();
        assert!(!panel.is_visible());
    }

    #[test]
    fn test_default_tabs() {
        let panel = SidePanel::new();
        assert_eq!(panel.tabs().len(), 1);
        assert_eq!(panel.active_tab(), Some(SidePanelTab::Diff));
    }

    #[test]
    fn test_toggle() {
        let mut panel = SidePanel::new();
        assert!(!panel.is_visible());
        panel.toggle();
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
    }

    #[test]
    fn test_show_hide() {
        let mut panel = SidePanel::new();
        panel.show();
        assert!(panel.is_visible());
        panel.hide();
        assert!(!panel.is_visible());
    }

    #[test]
    fn test_next_tab_wraps() {
        let mut panel =
            SidePanel::with_tabs(vec![SidePanelTab::Diff, SidePanelTab::Preview], false);
        assert_eq!(panel.active_tab(), Some(SidePanelTab::Diff));
        panel.next_tab();
        assert_eq!(panel.active_tab(), Some(SidePanelTab::Preview));
        panel.next_tab();
        assert_eq!(panel.active_tab(), Some(SidePanelTab::Diff));
    }

    #[test]
    fn test_prev_tab_wraps() {
        let mut panel = SidePanel::with_tabs(
            vec![
                SidePanelTab::Diff,
                SidePanelTab::Preview,
                SidePanelTab::TestLog,
            ],
            false,
        );
        panel.prev_tab();
        assert_eq!(panel.active_tab(), Some(SidePanelTab::TestLog));
        panel.prev_tab();
        assert_eq!(panel.active_tab(), Some(SidePanelTab::Preview));
    }

    #[test]
    fn test_set_active() {
        let mut panel = SidePanel::with_tabs(
            vec![
                SidePanelTab::Diff,
                SidePanelTab::Preview,
                SidePanelTab::TestLog,
            ],
            false,
        );
        panel.set_active(SidePanelTab::TestLog);
        assert_eq!(panel.active_index(), 2);
        assert_eq!(panel.active_tab(), Some(SidePanelTab::TestLog));
    }

    #[test]
    fn test_with_tabs() {
        let panel = SidePanel::with_tabs(vec![SidePanelTab::Diff, SidePanelTab::Preview], true);
        assert!(panel.is_visible());
        assert_eq!(panel.tabs().len(), 2);
    }

    #[test]
    fn test_empty_tabs() {
        let mut panel = SidePanel::with_tabs(vec![], false);
        assert!(panel.active_tab().is_none());
        panel.next_tab();
        panel.prev_tab();
        assert!(panel.active_tab().is_none());
    }
}
