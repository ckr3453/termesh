//! Layout engine for pane arrangement.

use crate::pane::Pane;
use termesh_core::types::{PaneId, SplitLayout};

/// Manages pane arrangement within the terminal window.
#[derive(Debug, Clone)]
pub struct LayoutManager {
    /// All panes in the layout.
    panes: Vec<Pane>,
    /// Currently focused pane index.
    focused: usize,
    /// Current layout mode.
    mode: SplitLayout,
    /// Next pane ID counter.
    next_id: u64,
}

impl LayoutManager {
    /// Create a new layout manager with a single full-screen pane.
    pub fn new() -> Self {
        Self {
            panes: vec![Pane::fullscreen(PaneId(0))],
            focused: 0,
            mode: SplitLayout::Quad,
            next_id: 1,
        }
    }

    /// Get the current layout mode.
    pub fn mode(&self) -> SplitLayout {
        self.mode
    }

    /// Get all panes.
    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    /// Get the focused pane.
    pub fn focused_pane(&self) -> &Pane {
        debug_assert!(
            self.focused < self.panes.len(),
            "focused index out of bounds"
        );
        &self.panes[self.focused]
    }

    /// Get the focused pane index.
    pub fn focused_index(&self) -> usize {
        self.focused
    }

    /// Get a pane by ID.
    pub fn pane_by_id(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Get a mutable pane by ID.
    pub fn pane_by_id_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// Number of panes.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Focus a pane by index.
    pub fn focus_index(&mut self, index: usize) {
        if index < self.panes.len() {
            self.focused = index;
        }
    }

    /// Focus a pane by ID.
    pub fn focus_pane(&mut self, id: PaneId) {
        if let Some(idx) = self.panes.iter().position(|p| p.id == id) {
            self.focused = idx;
        }
    }

    /// Focus the next pane (wrapping around).
    pub fn focus_next(&mut self) {
        if !self.panes.is_empty() {
            self.focused = (self.focused + 1) % self.panes.len();
        }
    }

    /// Focus the previous pane (wrapping around).
    pub fn focus_prev(&mut self) {
        if !self.panes.is_empty() {
            self.focused = if self.focused == 0 {
                self.panes.len() - 1
            } else {
                self.focused - 1
            };
        }
    }

    /// Apply a predefined split layout, replacing all panes.
    pub fn apply_layout(&mut self, layout: SplitLayout) {
        self.mode = layout;
        self.panes.clear();

        let panes: Vec<(f32, f32, f32, f32)> = match layout {
            SplitLayout::Dual => vec![(0.0, 0.0, 0.5, 1.0), (0.5, 0.0, 0.5, 1.0)],
            SplitLayout::Triple => vec![
                (0.0, 0.0, 0.5, 1.0),
                (0.5, 0.0, 0.5, 0.5),
                (0.5, 0.5, 0.5, 0.5),
            ],
            SplitLayout::Quad => vec![
                (0.0, 0.0, 0.5, 0.5),
                (0.5, 0.0, 0.5, 0.5),
                (0.0, 0.5, 0.5, 0.5),
                (0.5, 0.5, 0.5, 0.5),
            ],
        };

        for (x, y, w, h) in panes {
            let id = self.alloc_id();
            self.panes.push(Pane::new(id, x, y, w, h));
        }

        self.focused = 0;
    }

    /// Split the focused pane horizontally (left/right).
    ///
    /// The focused pane shrinks to the left half, a new pane takes the right half.
    /// Returns the new pane's ID.
    pub fn split_horizontal(&mut self) -> PaneId {
        let src = &self.panes[self.focused];
        let half_w = src.width / 2.0;
        let new_x = src.x + half_w;
        let y = src.y;
        let h = src.height;

        self.panes[self.focused].width = half_w;

        let new_id = self.alloc_id();
        self.panes.push(Pane::new(new_id, new_x, y, half_w, h));
        new_id
    }

    /// Split the focused pane vertically (top/bottom).
    ///
    /// The focused pane shrinks to the top half, a new pane takes the bottom half.
    /// Returns the new pane's ID.
    pub fn split_vertical(&mut self) -> PaneId {
        let src = &self.panes[self.focused];
        let half_h = src.height / 2.0;
        let x = src.x;
        let new_y = src.y + half_h;
        let w = src.width;

        self.panes[self.focused].height = half_h;

        let new_id = self.alloc_id();
        self.panes.push(Pane::new(new_id, x, new_y, w, half_h));
        new_id
    }

    /// Close a pane by ID. Returns `true` if removed.
    ///
    /// The last remaining pane cannot be closed.
    pub fn close_pane(&mut self, id: PaneId) -> bool {
        if self.panes.len() <= 1 {
            return false;
        }

        if let Some(idx) = self.panes.iter().position(|p| p.id == id) {
            self.panes.remove(idx);
            if self.focused >= self.panes.len() {
                self.focused = self.panes.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Reset to a single full-screen pane.
    pub fn reset_single(&mut self) {
        self.panes.clear();
        let id = self.alloc_id();
        self.panes.push(Pane::fullscreen(id));
        self.focused = 0;
    }

    /// Allocate a new unique pane ID.
    fn alloc_id(&mut self) -> PaneId {
        let id = PaneId(self.next_id);
        self.next_id += 1;
        id
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termesh_core::types::SessionId;

    #[test]
    fn test_new_has_single_pane() {
        let layout = LayoutManager::new();
        assert_eq!(layout.pane_count(), 1);
        assert_eq!(layout.focused_index(), 0);

        let pane = layout.focused_pane();
        assert_eq!(pane.width, 1.0);
        assert_eq!(pane.height, 1.0);
    }

    #[test]
    fn test_apply_dual_layout() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Dual);

        assert_eq!(layout.pane_count(), 2);
        assert_eq!(layout.mode(), SplitLayout::Dual);

        let panes = layout.panes();
        assert_eq!(panes[0].x, 0.0);
        assert_eq!(panes[0].width, 0.5);
        assert_eq!(panes[1].x, 0.5);
        assert_eq!(panes[1].width, 0.5);
    }

    #[test]
    fn test_apply_triple_layout() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Triple);

        assert_eq!(layout.pane_count(), 3);

        let panes = layout.panes();
        // Left pane: full height
        assert_eq!(panes[0].width, 0.5);
        assert_eq!(panes[0].height, 1.0);
        // Top-right
        assert_eq!(panes[1].x, 0.5);
        assert_eq!(panes[1].height, 0.5);
        // Bottom-right
        assert_eq!(panes[2].x, 0.5);
        assert_eq!(panes[2].y, 0.5);
    }

    #[test]
    fn test_apply_quad_layout() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Quad);

        assert_eq!(layout.pane_count(), 4);
        assert_eq!(layout.mode(), SplitLayout::Quad);

        let panes = layout.panes();
        // All panes should be 0.5 x 0.5
        for pane in panes {
            assert_eq!(pane.width, 0.5);
            assert_eq!(pane.height, 0.5);
        }
    }

    #[test]
    fn test_split_horizontal() {
        let mut layout = LayoutManager::new();
        let new_id = layout.split_horizontal();

        assert_eq!(layout.pane_count(), 2);

        let left = layout.focused_pane();
        assert_eq!(left.width, 0.5);

        let right = layout.pane_by_id(new_id).unwrap();
        assert_eq!(right.x, 0.5);
        assert_eq!(right.width, 0.5);
    }

    #[test]
    fn test_split_vertical() {
        let mut layout = LayoutManager::new();
        let new_id = layout.split_vertical();

        assert_eq!(layout.pane_count(), 2);

        let top = layout.focused_pane();
        assert_eq!(top.height, 0.5);

        let bottom = layout.pane_by_id(new_id).unwrap();
        assert_eq!(bottom.y, 0.5);
        assert_eq!(bottom.height, 0.5);
    }

    #[test]
    fn test_focus_cycle() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Quad);

        assert_eq!(layout.focused_index(), 0);

        layout.focus_next();
        assert_eq!(layout.focused_index(), 1);

        layout.focus_next();
        assert_eq!(layout.focused_index(), 2);

        layout.focus_next();
        assert_eq!(layout.focused_index(), 3);

        // Wrap around
        layout.focus_next();
        assert_eq!(layout.focused_index(), 0);
    }

    #[test]
    fn test_focus_prev_cycle() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Dual);

        assert_eq!(layout.focused_index(), 0);

        // Wrap around backwards
        layout.focus_prev();
        assert_eq!(layout.focused_index(), 1);

        layout.focus_prev();
        assert_eq!(layout.focused_index(), 0);
    }

    #[test]
    fn test_focus_pane_by_id() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Quad);

        let target_id = layout.panes()[2].id;
        layout.focus_pane(target_id);
        assert_eq!(layout.focused_index(), 2);
    }

    #[test]
    fn test_close_pane() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Quad);

        let id = layout.panes()[1].id;
        assert!(layout.close_pane(id));
        assert_eq!(layout.pane_count(), 3);
    }

    #[test]
    fn test_cannot_close_last_pane() {
        let mut layout = LayoutManager::new();
        let id = layout.panes()[0].id;
        assert!(!layout.close_pane(id));
        assert_eq!(layout.pane_count(), 1);
    }

    #[test]
    fn test_close_pane_adjusts_focus() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Quad);
        layout.focus_index(3);

        let id = layout.panes()[3].id;
        layout.close_pane(id);
        // Focus should clamp to last valid index
        assert!(layout.focused_index() < layout.pane_count());
    }

    #[test]
    fn test_reset_single() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Quad);
        assert_eq!(layout.pane_count(), 4);

        layout.reset_single();
        assert_eq!(layout.pane_count(), 1);

        let pane = layout.focused_pane();
        assert_eq!(pane.width, 1.0);
        assert_eq!(pane.height, 1.0);
    }

    #[test]
    fn test_bind_session_to_pane() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Dual);

        let pane_id = layout.panes()[0].id;
        layout
            .pane_by_id_mut(pane_id)
            .unwrap()
            .bind_session(SessionId(1));

        let pane = layout.pane_by_id(pane_id).unwrap();
        assert_eq!(pane.session_id, Some(SessionId(1)));
    }

    #[test]
    fn test_split_preserves_position() {
        let mut layout = LayoutManager::new();
        layout.apply_layout(SplitLayout::Dual);
        // Focus the right pane (x=0.5, w=0.5)
        layout.focus_index(1);

        let new_id = layout.split_vertical();
        let top = layout.focused_pane();
        assert_eq!(top.x, 0.5);
        assert_eq!(top.height, 0.5);

        let bottom = layout.pane_by_id(new_id).unwrap();
        assert_eq!(bottom.x, 0.5);
        assert_eq!(bottom.y, 0.5);
        assert_eq!(bottom.height, 0.5);
    }
}
