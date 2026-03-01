//! Split mode layout: manages multi-pane tmux-style view with zoom and dividers.

use crate::layout::LayoutManager;
use crate::pane::PixelRect;
use termesh_core::types::{PaneId, SplitLayout};

/// A divider line between panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Divider {
    /// Start X position.
    pub x: u32,
    /// Start Y position.
    pub y: u32,
    /// Length in pixels.
    pub length: u32,
    /// Orientation.
    pub orientation: DividerOrientation,
}

/// Divider orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerOrientation {
    /// Vertical line (separating left/right panes).
    Vertical,
    /// Horizontal line (separating top/bottom panes).
    Horizontal,
}

/// Manages the Split mode layout state.
#[derive(Debug, Clone)]
pub struct SplitLayoutManager {
    /// The underlying pane layout engine.
    layout: LayoutManager,
    /// If set, the pane that is currently zoomed to fullscreen.
    zoomed_pane: Option<PaneId>,
}

impl SplitLayoutManager {
    /// Create a new Split layout with a specific arrangement.
    pub fn new(split: SplitLayout) -> Self {
        let mut layout = LayoutManager::new();
        layout.apply_layout(split);
        Self {
            layout,
            zoomed_pane: None,
        }
    }

    /// Get the underlying layout manager.
    pub fn layout(&self) -> &LayoutManager {
        &self.layout
    }

    /// Get mutable access to the underlying layout manager.
    pub fn layout_mut(&mut self) -> &mut LayoutManager {
        &mut self.layout
    }

    /// Bind a session to a pane.
    pub fn bind_session(&mut self, pane_id: PaneId, session_id: termesh_core::types::SessionId) {
        if let Some(pane) = self.layout.pane_by_id_mut(pane_id) {
            pane.bind_session(session_id);
        }
    }

    /// Focus the next pane.
    pub fn focus_next(&mut self) {
        self.layout.focus_next();
    }

    /// Focus the previous pane.
    pub fn focus_prev(&mut self) {
        self.layout.focus_prev();
    }

    /// Focus a pane by index (0-based).
    pub fn focus_index(&mut self, index: usize) {
        self.layout.focus_index(index);
    }

    /// Focus a pane by ID.
    pub fn focus_pane(&mut self, id: PaneId) {
        self.layout.focus_pane(id);
    }

    /// Switch to a different split arrangement.
    pub fn set_split(&mut self, split: SplitLayout) {
        self.zoomed_pane = None;
        self.layout.apply_layout(split);
    }

    /// Toggle zoom on the focused pane.
    ///
    /// If no pane is zoomed, zoom the focused pane to fullscreen.
    /// If the focused pane is already zoomed, restore the split layout.
    pub fn toggle_zoom(&mut self) {
        if self.zoomed_pane.is_some() {
            self.zoomed_pane = None;
        } else {
            self.zoomed_pane = Some(self.layout.focused_pane().id);
        }
    }

    /// Whether a pane is currently zoomed.
    pub fn is_zoomed(&self) -> bool {
        self.zoomed_pane.is_some()
    }

    /// Get the zoomed pane ID (if any).
    pub fn zoomed_pane(&self) -> Option<PaneId> {
        self.zoomed_pane
    }

    /// Compute the pixel rect for a pane, considering zoom state.
    pub fn pane_rect(
        &self,
        pane_id: PaneId,
        screen_width: u32,
        screen_height: u32,
    ) -> Option<PixelRect> {
        // If zoomed, only the zoomed pane is visible
        if let Some(zoomed) = self.zoomed_pane {
            if pane_id == zoomed {
                return Some(PixelRect {
                    x: 0,
                    y: 0,
                    width: screen_width,
                    height: screen_height,
                });
            }
            // Hidden but exists — return zero rect; nonexistent — return None
            return if self.layout.pane_by_id(pane_id).is_some() {
                Some(PixelRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                })
            } else {
                None
            };
        }

        self.layout
            .pane_by_id(pane_id)
            .map(|pane| pane.pixel_rect(screen_width, screen_height))
    }

    /// Whether a given pane is visible (not hidden by zoom).
    pub fn is_pane_visible(&self, pane_id: PaneId) -> bool {
        match self.zoomed_pane {
            Some(zoomed) => pane_id == zoomed,
            None => self.layout.pane_by_id(pane_id).is_some(),
        }
    }

    /// Compute divider lines for the current layout.
    pub fn compute_dividers(&self, screen_width: u32, screen_height: u32) -> Vec<Divider> {
        // No dividers when zoomed
        if self.zoomed_pane.is_some() {
            return Vec::new();
        }

        let panes = self.layout.panes();
        let mut dividers = Vec::new();

        // Deduplicate dividers by checking for shared edges between panes
        for (i, a) in panes.iter().enumerate() {
            for b in panes.iter().skip(i + 1) {
                let ar = a.pixel_rect(screen_width, screen_height);
                let br = b.pixel_rect(screen_width, screen_height);

                // Vertical divider: a's right edge == b's left edge
                if ar.x + ar.width == br.x {
                    let top = ar.y.max(br.y);
                    let bottom = (ar.y + ar.height).min(br.y + br.height);
                    if bottom > top {
                        dividers.push(Divider {
                            x: br.x,
                            y: top,
                            length: bottom - top,
                            orientation: DividerOrientation::Vertical,
                        });
                    }
                }

                // Horizontal divider: a's bottom edge == b's top edge
                if ar.y + ar.height == br.y {
                    let left = ar.x.max(br.x);
                    let right = (ar.x + ar.width).min(br.x + br.width);
                    if right > left {
                        dividers.push(Divider {
                            x: left,
                            y: br.y,
                            length: right - left,
                            orientation: DividerOrientation::Horizontal,
                        });
                    }
                }
            }
        }

        dividers
    }

    /// Focus the pane in the given direction relative to the focused pane.
    pub fn focus_direction(&mut self, direction: Direction, screen_w: u32, screen_h: u32) {
        if self.zoomed_pane.is_some() {
            return;
        }

        let focused = self.layout.focused_pane();
        let fr = focused.pixel_rect(screen_w, screen_h);
        let fc_x = fr.x + fr.width / 2;
        let fc_y = fr.y + fr.height / 2;
        let focused_id = focused.id;

        let mut best: Option<(PaneId, u32)> = None;

        for pane in self.layout.panes() {
            if pane.id == focused_id {
                continue;
            }
            let pr = pane.pixel_rect(screen_w, screen_h);
            let pc_x = pr.x + pr.width / 2;
            let pc_y = pr.y + pr.height / 2;

            let valid = match direction {
                Direction::Left => pc_x < fc_x,
                Direction::Right => pc_x > fc_x,
                Direction::Up => pc_y < fc_y,
                Direction::Down => pc_y > fc_y,
            };

            if valid {
                let dist = fc_x.abs_diff(pc_x) + fc_y.abs_diff(pc_y);
                if best.is_none_or(|(_, d)| dist < d) {
                    best = Some((pane.id, dist));
                }
            }
        }

        if let Some((id, _)) = best {
            self.layout.focus_pane(id);
        }
    }
}

/// Direction for focus navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_dual() {
        let split = SplitLayoutManager::new(SplitLayout::Dual);
        assert_eq!(split.layout().pane_count(), 2);
        assert!(!split.is_zoomed());
    }

    #[test]
    fn test_new_quad() {
        let split = SplitLayoutManager::new(SplitLayout::Quad);
        assert_eq!(split.layout().pane_count(), 4);
    }

    #[test]
    fn test_toggle_zoom() {
        let mut split = SplitLayoutManager::new(SplitLayout::Dual);
        let focused_id = split.layout().focused_pane().id;

        split.toggle_zoom();
        assert!(split.is_zoomed());
        assert_eq!(split.zoomed_pane(), Some(focused_id));

        split.toggle_zoom();
        assert!(!split.is_zoomed());
        assert_eq!(split.zoomed_pane(), None);
    }

    #[test]
    fn test_pane_rect_normal() {
        let split = SplitLayoutManager::new(SplitLayout::Dual);
        let panes = split.layout().panes();
        let left_id = panes[0].id;
        let right_id = panes[1].id;

        let left = split.pane_rect(left_id, 1280, 800).unwrap();
        assert_eq!(left.x, 0);
        assert_eq!(left.width, 640);

        let right = split.pane_rect(right_id, 1280, 800).unwrap();
        assert_eq!(right.x, 640);
        assert_eq!(right.width, 640);
    }

    #[test]
    fn test_pane_rect_zoomed() {
        let mut split = SplitLayoutManager::new(SplitLayout::Dual);
        let panes = split.layout().panes();
        let left_id = panes[0].id;
        let right_id = panes[1].id;

        // Zoom the left pane
        split.toggle_zoom();

        let left = split.pane_rect(left_id, 1280, 800).unwrap();
        assert_eq!(left.width, 1280);
        assert_eq!(left.height, 800);

        let right = split.pane_rect(right_id, 1280, 800).unwrap();
        assert_eq!(right.width, 0);
        assert_eq!(right.height, 0);
    }

    #[test]
    fn test_is_pane_visible() {
        let mut split = SplitLayoutManager::new(SplitLayout::Dual);
        let panes = split.layout().panes();
        let left_id = panes[0].id;
        let right_id = panes[1].id;

        assert!(split.is_pane_visible(left_id));
        assert!(split.is_pane_visible(right_id));

        split.toggle_zoom(); // zoom left
        assert!(split.is_pane_visible(left_id));
        assert!(!split.is_pane_visible(right_id));
    }

    #[test]
    fn test_dividers_dual() {
        let split = SplitLayoutManager::new(SplitLayout::Dual);
        let dividers = split.compute_dividers(1280, 800);

        assert_eq!(dividers.len(), 1);
        assert_eq!(dividers[0].orientation, DividerOrientation::Vertical);
        assert_eq!(dividers[0].length, 800);
    }

    #[test]
    fn test_dividers_quad() {
        let split = SplitLayoutManager::new(SplitLayout::Quad);
        let dividers = split.compute_dividers(1280, 800);

        // Quad: 1 vertical center + 2 horizontal (left half + right half)
        // Actually: 4 panes in 2x2 grid creates:
        //   - vertical divider between left/right columns
        //   - horizontal dividers between top/bottom rows
        assert!(dividers.len() >= 2);

        let vertical = dividers
            .iter()
            .filter(|d| d.orientation == DividerOrientation::Vertical)
            .count();
        let horizontal = dividers
            .iter()
            .filter(|d| d.orientation == DividerOrientation::Horizontal)
            .count();
        assert!(vertical >= 1);
        assert!(horizontal >= 1);
    }

    #[test]
    fn test_dividers_zoomed() {
        let mut split = SplitLayoutManager::new(SplitLayout::Quad);
        split.toggle_zoom();
        let dividers = split.compute_dividers(1280, 800);
        assert!(dividers.is_empty());
    }

    #[test]
    fn test_focus_direction_dual() {
        let mut split = SplitLayoutManager::new(SplitLayout::Dual);
        let right_id = split.layout().panes()[1].id;

        // Start at left pane, move right
        split.focus_direction(Direction::Right, 1280, 800);
        assert_eq!(split.layout().focused_pane().id, right_id);

        // Move left back
        let left_id = split.layout().panes()[0].id;
        split.focus_direction(Direction::Left, 1280, 800);
        assert_eq!(split.layout().focused_pane().id, left_id);
    }

    #[test]
    fn test_focus_direction_quad() {
        let mut split = SplitLayoutManager::new(SplitLayout::Quad);
        // Quad layout: [0]=top-left, [1]=top-right, [2]=bottom-left, [3]=bottom-right
        let panes: Vec<PaneId> = split.layout().panes().iter().map(|p| p.id).collect();

        // Start at top-left, go right
        split.focus_direction(Direction::Right, 1280, 800);
        assert_eq!(split.layout().focused_pane().id, panes[1]);

        // Go down
        split.focus_direction(Direction::Down, 1280, 800);
        assert_eq!(split.layout().focused_pane().id, panes[3]);

        // Go left
        split.focus_direction(Direction::Left, 1280, 800);
        assert_eq!(split.layout().focused_pane().id, panes[2]);

        // Go up
        split.focus_direction(Direction::Up, 1280, 800);
        assert_eq!(split.layout().focused_pane().id, panes[0]);
    }

    #[test]
    fn test_focus_direction_no_move_when_zoomed() {
        let mut split = SplitLayoutManager::new(SplitLayout::Dual);
        let focused_before = split.layout().focused_pane().id;
        split.toggle_zoom();

        split.focus_direction(Direction::Right, 1280, 800);
        assert_eq!(split.layout().focused_pane().id, focused_before);
    }

    #[test]
    fn test_set_split_clears_zoom() {
        let mut split = SplitLayoutManager::new(SplitLayout::Dual);
        split.toggle_zoom();
        assert!(split.is_zoomed());

        split.set_split(SplitLayout::Quad);
        assert!(!split.is_zoomed());
        assert_eq!(split.layout().pane_count(), 4);
    }
}
