//! Pane representation with position, size, and session binding.

use serde::{Deserialize, Serialize};
use termesh_core::types::{PaneId, SessionId};

/// A rectangular region of the screen assigned to a terminal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pane {
    /// Unique pane identifier.
    pub id: PaneId,
    /// Bound session (if any).
    pub session_id: Option<SessionId>,
    /// Normalized X position (0.0 = left edge, 1.0 = right edge).
    pub x: f32,
    /// Normalized Y position (0.0 = top edge, 1.0 = bottom edge).
    pub y: f32,
    /// Normalized width (fraction of total width).
    pub width: f32,
    /// Normalized height (fraction of total height).
    pub height: f32,
}

impl Pane {
    /// Create a new pane with the given bounds.
    pub fn new(id: PaneId, x: f32, y: f32, width: f32, height: f32) -> Self {
        debug_assert!((0.0..=1.0).contains(&x), "x out of range: {x}");
        debug_assert!((0.0..=1.0).contains(&y), "y out of range: {y}");
        debug_assert!(width > 0.0 && width <= 1.0, "width out of range: {width}");
        debug_assert!(
            height > 0.0 && height <= 1.0,
            "height out of range: {height}"
        );
        Self {
            id,
            session_id: None,
            x,
            y,
            width,
            height,
        }
    }

    /// Create a full-screen pane.
    pub fn fullscreen(id: PaneId) -> Self {
        Self::new(id, 0.0, 0.0, 1.0, 1.0)
    }

    /// Bind a session to this pane.
    pub fn bind_session(&mut self, session_id: SessionId) {
        self.session_id = Some(session_id);
    }

    /// Unbind the current session.
    pub fn unbind_session(&mut self) {
        self.session_id = None;
    }

    /// Calculate pixel bounds given a total screen size.
    pub fn pixel_rect(&self, screen_width: u32, screen_height: u32) -> PixelRect {
        let sw = screen_width as f32;
        let sh = screen_height as f32;
        PixelRect {
            x: (self.x * sw) as u32,
            y: (self.y * sh) as u32,
            width: (self.width * sw) as u32,
            height: (self.height * sh) as u32,
        }
    }

    /// Calculate terminal grid dimensions given cell size.
    pub fn grid_size(
        &self,
        screen_width: u32,
        screen_height: u32,
        cell_w: f32,
        cell_h: f32,
    ) -> (usize, usize) {
        if cell_w <= 0.0 || cell_h <= 0.0 {
            return (1, 1);
        }
        let rect = self.pixel_rect(screen_width, screen_height);
        let cols = (rect.width as f32 / cell_w).floor() as usize;
        let rows = (rect.height as f32 / cell_h).floor() as usize;
        (rows.max(1), cols.max(1))
    }
}

/// Pixel rectangle on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    /// Check if a pixel coordinate falls within this rectangle.
    pub fn contains(&self, px: f64, py: f64) -> bool {
        let x = self.x as f64;
        let y = self.y as f64;
        px >= x && px < x + self.width as f64 && py >= y && py < y + self.height as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fullscreen_pane() {
        let pane = Pane::fullscreen(PaneId(0));
        assert_eq!(pane.x, 0.0);
        assert_eq!(pane.y, 0.0);
        assert_eq!(pane.width, 1.0);
        assert_eq!(pane.height, 1.0);
        assert!(pane.session_id.is_none());
    }

    #[test]
    fn test_bind_session() {
        let mut pane = Pane::fullscreen(PaneId(0));
        pane.bind_session(SessionId(42));
        assert_eq!(pane.session_id, Some(SessionId(42)));

        pane.unbind_session();
        assert!(pane.session_id.is_none());
    }

    #[test]
    fn test_pixel_rect() {
        let pane = Pane::new(PaneId(0), 0.5, 0.0, 0.5, 1.0);
        let rect = pane.pixel_rect(1280, 800);
        assert_eq!(rect.x, 640);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 640);
        assert_eq!(rect.height, 800);
    }

    #[test]
    fn test_grid_size() {
        let pane = Pane::fullscreen(PaneId(0));
        let (rows, cols) = pane.grid_size(1280, 800, 8.0, 16.0);
        assert_eq!(cols, 160); // 1280 / 8
        assert_eq!(rows, 50); // 800 / 16
    }

    #[test]
    fn test_pixel_rect_contains() {
        let rect = PixelRect {
            x: 100,
            y: 50,
            width: 200,
            height: 100,
        };
        assert!(rect.contains(100.0, 50.0)); // top-left corner
        assert!(rect.contains(200.0, 100.0)); // middle
        assert!(!rect.contains(99.0, 50.0)); // just outside left
        assert!(!rect.contains(300.0, 50.0)); // right edge (exclusive)
        assert!(!rect.contains(100.0, 150.0)); // bottom edge (exclusive)
    }

    #[test]
    fn test_grid_size_half_pane() {
        let pane = Pane::new(PaneId(0), 0.0, 0.0, 0.5, 0.5);
        let (rows, cols) = pane.grid_size(1280, 800, 8.0, 16.0);
        assert_eq!(cols, 80); // 640 / 8
        assert_eq!(rows, 25); // 400 / 16
    }
}
