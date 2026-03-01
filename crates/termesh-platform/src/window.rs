//! Window configuration and creation utilities.

use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

/// Default window width in logical pixels.
pub const DEFAULT_WIDTH: u32 = 1280;

/// Default window height in logical pixels.
pub const DEFAULT_HEIGHT: u32 = 800;

/// Minimum window width in logical pixels.
pub const MIN_WIDTH: u32 = 400;

/// Minimum window height in logical pixels.
pub const MIN_HEIGHT: u32 = 300;

/// Create window attributes with termesh defaults.
pub fn default_window_attributes() -> WindowAttributes {
    WindowAttributes::default()
        .with_title("Termesh")
        .with_inner_size(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT))
        .with_min_inner_size(LogicalSize::new(MIN_WIDTH, MIN_HEIGHT))
}

/// Create a window from the event loop with default attributes.
pub fn create_window(event_loop: &ActiveEventLoop) -> Result<Window, winit::error::OsError> {
    let attrs = default_window_attributes();
    event_loop.create_window(attrs)
}

// Compile-time validation that defaults exceed minimums.
const _: () = {
    assert!(DEFAULT_WIDTH > MIN_WIDTH);
    assert!(DEFAULT_HEIGHT > MIN_HEIGHT);
};
