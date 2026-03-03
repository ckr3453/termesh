//! Focus mode layout: manages the 3-panel layout (session list + terminal + side panel).

use crate::pane::PixelRect;
use crate::session_list::SessionList;
use crate::side_panel::SidePanel;
use termesh_core::types::SidePanelTab;

/// Default width of the session list panel in pixels.
const DEFAULT_SESSION_LIST_WIDTH: u32 = 180;

/// Default width of the side panel in pixels.
const DEFAULT_SIDE_PANEL_WIDTH: u32 = 300;

/// Minimum width for the terminal area in pixels.
const MIN_TERMINAL_WIDTH: u32 = 200;

/// Height reserved for the header bar (in rows).
pub const HEADER_HEIGHT: u32 = 0;

/// Height reserved for the status bar (in rows).
pub const STATUS_HEIGHT: u32 = 1;

/// Which region of the Focus mode layout has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRegion {
    /// The session list on the left.
    SessionList,
    /// The main terminal in the center.
    Terminal,
    /// The side panel on the right.
    SidePanel,
}

/// Computed layout rectangles for the Focus mode UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusRegions {
    /// Session list area (left).
    pub session_list: PixelRect,
    /// Main terminal area (center).
    pub terminal: PixelRect,
    /// Side panel area (right). Zero-width if hidden.
    pub side_panel: PixelRect,
}

/// Manages the Focus mode layout state.
#[derive(Debug, Clone)]
pub struct FocusLayout {
    /// Session list state.
    sessions: SessionList,
    /// Side panel state.
    side_panel: SidePanel,
    /// Which region currently has focus.
    focus: FocusRegion,
    /// Session list width in pixels.
    session_list_width: u32,
    /// Side panel width in pixels.
    side_panel_width: u32,
}

impl FocusLayout {
    /// Create a new Focus mode layout with defaults.
    pub fn new() -> Self {
        Self {
            sessions: SessionList::new(),
            side_panel: SidePanel::new(),
            focus: FocusRegion::Terminal,
            session_list_width: DEFAULT_SESSION_LIST_WIDTH,
            side_panel_width: DEFAULT_SIDE_PANEL_WIDTH,
        }
    }

    /// Create a Focus layout with a specific side panel tab shown.
    pub fn with_side_panel(tab: SidePanelTab) -> Self {
        let mut layout = Self::new();
        layout.toggle_side_panel();
        layout.side_panel.set_active(tab);
        layout
    }

    /// Get the session list.
    pub fn sessions(&self) -> &SessionList {
        &self.sessions
    }

    /// Get a mutable reference to the session list.
    pub fn sessions_mut(&mut self) -> &mut SessionList {
        &mut self.sessions
    }

    /// Get the side panel.
    pub fn side_panel(&self) -> &SidePanel {
        &self.side_panel
    }

    /// Get a mutable reference to the side panel.
    pub fn side_panel_mut(&mut self) -> &mut SidePanel {
        &mut self.side_panel
    }

    /// Switch to the next side panel tab.
    pub fn next_side_panel_tab(&mut self) {
        self.side_panel.next_tab();
    }

    /// Switch to the previous side panel tab.
    pub fn prev_side_panel_tab(&mut self) {
        self.side_panel.prev_tab();
    }

    /// Set the active side panel tab.
    pub fn set_side_panel_tab(&mut self, tab: SidePanelTab) {
        self.side_panel.set_active(tab);
    }

    /// Compute layout rectangles for a given screen size.
    ///
    /// All regions occupy the full screen height. Use [`compute_regions_with_bars`]
    /// to reserve space for header and status bars.
    pub fn compute_regions(&self, screen_width: u32, screen_height: u32) -> FocusRegions {
        self.compute_regions_inner(screen_width, screen_height, 0, 0)
    }

    /// Compute layout rectangles, reserving pixel space for header and status bars.
    ///
    /// `header_px` and `status_px` are the pixel heights of the header/status bars.
    /// All panels start at y = `header_px` and their height is reduced accordingly.
    pub fn compute_regions_with_bars(
        &self,
        screen_width: u32,
        screen_height: u32,
        header_px: u32,
        status_px: u32,
    ) -> FocusRegions {
        self.compute_regions_inner(screen_width, screen_height, header_px, status_px)
    }

    /// Inner implementation for region computation.
    fn compute_regions_inner(
        &self,
        screen_width: u32,
        screen_height: u32,
        header_px: u32,
        status_px: u32,
    ) -> FocusRegions {
        let y_offset = header_px;
        let usable_height = screen_height
            .saturating_sub(header_px)
            .saturating_sub(status_px);

        let list_w = self.session_list_width.min(screen_width / 3);

        let panel_w = if self.side_panel.is_visible() {
            let remaining = screen_width.saturating_sub(list_w);
            self.side_panel_width.min(remaining / 2)
        } else {
            0
        };

        let terminal_w = screen_width
            .saturating_sub(list_w)
            .saturating_sub(panel_w)
            .max(MIN_TERMINAL_WIDTH.min(screen_width));

        // Adjust if terminal is too small: shrink side panel first, then list.
        let (list_w, terminal_w, panel_w) =
            Self::adjust_widths(screen_width, list_w, terminal_w, panel_w);

        FocusRegions {
            session_list: PixelRect {
                x: 0,
                y: y_offset,
                width: list_w,
                height: usable_height,
            },
            terminal: PixelRect {
                x: list_w,
                y: y_offset,
                width: terminal_w,
                height: usable_height,
            },
            side_panel: PixelRect {
                x: list_w + terminal_w,
                y: y_offset,
                width: panel_w,
                height: usable_height,
            },
        }
    }

    /// Adjust widths to fit screen, ensuring total == screen_width.
    fn adjust_widths(
        screen_width: u32,
        list_w: u32,
        _terminal_w: u32,
        panel_w: u32,
    ) -> (u32, u32, u32) {
        let total = list_w + panel_w;
        if total <= screen_width {
            // Give remaining space to terminal
            (
                list_w,
                screen_width.saturating_sub(list_w + panel_w),
                panel_w,
            )
        } else {
            // Screen too small — shrink list, drop side panel
            let list_w = list_w.min(screen_width / 2);
            (list_w, screen_width.saturating_sub(list_w), 0)
        }
    }

    /// Get the current focus region.
    pub fn focus_region(&self) -> FocusRegion {
        self.focus
    }

    /// Set the focus region.
    pub fn set_focus(&mut self, region: FocusRegion) {
        self.focus = region;
    }

    /// Cycle focus to the next region (left -> center -> right -> left).
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            FocusRegion::SessionList => FocusRegion::Terminal,
            FocusRegion::Terminal => {
                if self.side_panel.is_visible() {
                    FocusRegion::SidePanel
                } else {
                    FocusRegion::SessionList
                }
            }
            FocusRegion::SidePanel => FocusRegion::SessionList,
        };
    }

    /// Toggle the side panel visibility.
    pub fn toggle_side_panel(&mut self) {
        self.side_panel.toggle();
        // If we hid the panel while it was focused, move to terminal
        if !self.side_panel.is_visible() && self.focus == FocusRegion::SidePanel {
            self.focus = FocusRegion::Terminal;
        }
    }

    /// Set the session list panel width.
    pub fn set_session_list_width(&mut self, width: u32) {
        self.session_list_width = width;
    }

    /// Set the side panel width.
    pub fn set_side_panel_width(&mut self, width: u32) {
        self.side_panel_width = width;
    }

    /// Get the session list panel width.
    pub fn session_list_width(&self) -> u32 {
        self.session_list_width
    }

    /// Get the side panel width.
    pub fn side_panel_width(&self) -> u32 {
        self.side_panel_width
    }
}

impl Default for FocusLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_list::SessionEntry;
    use termesh_core::types::{AgentState, SessionId};

    #[test]
    fn test_default_focus_is_terminal() {
        let layout = FocusLayout::new();
        assert_eq!(layout.focus_region(), FocusRegion::Terminal);
    }

    #[test]
    fn test_compute_regions_no_side_panel() {
        let layout = FocusLayout::new();
        let regions = layout.compute_regions(1280, 800);

        assert_eq!(regions.session_list.x, 0);
        assert_eq!(regions.session_list.width, 180);
        assert_eq!(regions.session_list.height, 800);

        assert_eq!(regions.terminal.x, 180);
        assert_eq!(regions.terminal.width, 1100); // 1280 - 180
        assert_eq!(regions.terminal.height, 800);

        assert_eq!(regions.side_panel.width, 0);
    }

    #[test]
    fn test_compute_regions_with_side_panel() {
        let mut layout = FocusLayout::new();
        layout.toggle_side_panel();

        let regions = layout.compute_regions(1280, 800);

        assert_eq!(regions.session_list.width, 180);
        assert_eq!(regions.side_panel.width, 300);
        assert_eq!(regions.terminal.width, 800); // 1280 - 180 - 300
        assert_eq!(
            regions.session_list.width + regions.terminal.width + regions.side_panel.width,
            1280
        );
    }

    #[test]
    fn test_compute_regions_small_screen() {
        let layout = FocusLayout::new();
        let regions = layout.compute_regions(400, 300);

        // List width should be capped at screen/3
        assert!(regions.session_list.width <= 400 / 3);
        assert_eq!(
            regions.session_list.width + regions.terminal.width + regions.side_panel.width,
            400
        );
    }

    #[test]
    fn test_compute_regions_very_small_screen() {
        let mut layout = FocusLayout::new();
        layout.toggle_side_panel();

        let regions = layout.compute_regions(300, 200);

        // Total should equal screen width
        assert_eq!(
            regions.session_list.width + regions.terminal.width + regions.side_panel.width,
            300
        );
        // Terminal should still have some width
        assert!(regions.terminal.width > 0);
    }

    #[test]
    fn test_toggle_side_panel() {
        let mut layout = FocusLayout::new();
        assert!(!layout.side_panel().is_visible());

        layout.toggle_side_panel();
        assert!(layout.side_panel().is_visible());

        layout.toggle_side_panel();
        assert!(!layout.side_panel().is_visible());
    }

    #[test]
    fn test_toggle_side_panel_moves_focus() {
        let mut layout = FocusLayout::new();
        layout.toggle_side_panel();
        layout.set_focus(FocusRegion::SidePanel);

        layout.toggle_side_panel(); // hide
        assert_eq!(layout.focus_region(), FocusRegion::Terminal);
    }

    #[test]
    fn test_cycle_focus_without_side_panel() {
        let mut layout = FocusLayout::new();

        assert_eq!(layout.focus_region(), FocusRegion::Terminal);
        layout.cycle_focus();
        assert_eq!(layout.focus_region(), FocusRegion::SessionList);
        layout.cycle_focus();
        assert_eq!(layout.focus_region(), FocusRegion::Terminal);
    }

    #[test]
    fn test_cycle_focus_with_side_panel() {
        let mut layout = FocusLayout::new();
        layout.toggle_side_panel();

        assert_eq!(layout.focus_region(), FocusRegion::Terminal);
        layout.cycle_focus();
        assert_eq!(layout.focus_region(), FocusRegion::SidePanel);
        layout.cycle_focus();
        assert_eq!(layout.focus_region(), FocusRegion::SessionList);
        layout.cycle_focus();
        assert_eq!(layout.focus_region(), FocusRegion::Terminal);
    }

    #[test]
    fn test_with_side_panel() {
        let layout = FocusLayout::with_side_panel(SidePanelTab::Diff);
        assert!(layout.side_panel().is_visible());
        assert_eq!(layout.side_panel().active_tab(), Some(SidePanelTab::Diff));
    }

    #[test]
    fn test_session_integration() {
        let mut layout = FocusLayout::new();
        layout.sessions_mut().add(SessionEntry {
            id: SessionId(1),
            label: "Backend".to_string(),
            is_agent: true,
            state: AgentState::Idle,
            project_id: None,
        });
        layout.sessions_mut().add(SessionEntry {
            id: SessionId(2),
            label: "Shell".to_string(),
            is_agent: false,
            state: AgentState::None,
            project_id: None,
        });

        assert_eq!(layout.sessions().len(), 2);
        assert_eq!(layout.sessions().selected_id(), Some(SessionId(1)));

        layout.sessions_mut().select_next();
        assert_eq!(layout.sessions().selected_id(), Some(SessionId(2)));
    }

    #[test]
    fn test_custom_widths() {
        let mut layout = FocusLayout::new();
        layout.set_session_list_width(250);
        layout.set_side_panel_width(400);
        layout.toggle_side_panel();

        let regions = layout.compute_regions(1280, 800);
        assert_eq!(regions.session_list.width, 250);
        assert_eq!(regions.side_panel.width, 400);
        assert_eq!(regions.terminal.width, 630);
    }

    #[test]
    fn test_regions_contiguous() {
        let mut layout = FocusLayout::new();
        layout.toggle_side_panel();

        let regions = layout.compute_regions(1920, 1080);

        // Regions should be contiguous
        assert_eq!(regions.terminal.x, regions.session_list.width);
        assert_eq!(
            regions.side_panel.x,
            regions.session_list.width + regions.terminal.width
        );
        // Total width should equal screen
        assert_eq!(
            regions.session_list.width + regions.terminal.width + regions.side_panel.width,
            1920
        );
        // All heights should be full screen
        assert_eq!(regions.session_list.height, 1080);
        assert_eq!(regions.terminal.height, 1080);
        assert_eq!(regions.side_panel.height, 1080);
    }
}
