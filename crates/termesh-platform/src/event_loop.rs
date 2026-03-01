//! Main event loop integrating winit, wgpu renderer, and terminal.

use crate::input_bridge;
use crate::window::default_window_attributes;
use std::sync::Arc;
use termesh_input::action::Action;
use termesh_input::handler::InputHandler;
use termesh_renderer::renderer::Renderer;
use termesh_terminal::terminal::Terminal;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

/// Callback trait for delegating event handling to the application layer.
///
/// The platform event loop calls these methods; the application implements
/// them to wire up PTY sessions, layout, and rendering.
pub trait AppCallbacks {
    /// Called when a non-modifier key is pressed without an action match.
    /// The application should forward the bytes to the active PTY.
    fn on_input(&mut self, text: &[u8]);

    /// Called when a keybinding action is matched.
    fn on_action(&mut self, action: Action);

    /// Called on each frame tick (before rendering).
    /// The application should process pending PTY output here.
    /// Returns a list of (grid_snapshot, x_offset, y_offset) for rendering.
    fn on_tick(&mut self) -> Vec<(termesh_terminal::grid::GridSnapshot, f32, f32)>;

    /// Called when the window is resized.
    /// `rows`/`cols` are grid dimensions, `width`/`height` are pixel dimensions.
    fn on_resize(&mut self, rows: usize, cols: usize, width: u32, height: u32);

    /// Called when the user scrolls (mouse wheel / trackpad).
    /// Positive delta = scroll up (view older output), negative = scroll down.
    fn on_scroll(&mut self, delta: i32);

    /// Called when the mouse button is pressed at a grid coordinate.
    fn on_mouse_press(&mut self, row: usize, col: usize);

    /// Called when the mouse is dragged to a grid coordinate (selection update).
    fn on_mouse_drag(&mut self, row: usize, col: usize);

    /// Called when the mouse button is released.
    fn on_mouse_release(&mut self);

    /// Called to copy selection text. Returns the selected text if any.
    fn on_copy(&mut self) -> Option<String>;

    /// Called to paste text from clipboard.
    fn on_paste(&mut self, text: &str);

    /// Returns true if the application should exit (e.g., no sessions left).
    fn should_exit(&self) -> bool;
}

/// Configuration for launching the platform event loop.
pub struct PlatformConfig {
    /// Font size in points.
    pub font_size: f32,
    /// Terminal scrollback lines.
    pub scrollback: usize,
    /// Input handler for keybinding dispatch.
    pub input_handler: InputHandler,
    /// Application callbacks (optional — if None, uses standalone terminal).
    pub callbacks: Option<Box<dyn AppCallbacks>>,
}

impl Default for PlatformConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            scrollback: 10_000,
            input_handler: InputHandler::new(),
            callbacks: None,
        }
    }
}

/// Application state managed by the winit event loop.
struct App {
    config: PlatformConfig,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    terminal: Option<Terminal>,
    /// Cached winit modifiers state, updated on each ModifiersChanged event.
    current_modifiers: winit::event::Modifiers,
    /// Application callbacks for PTY/session integration.
    callbacks: Option<Box<dyn AppCallbacks>>,
    /// Whether the left mouse button is currently held (for drag selection).
    mouse_pressed: bool,
    /// Cached cursor position in pixels.
    cursor_position: (f64, f64),
    /// System clipboard.
    clipboard: Option<arboard::Clipboard>,
}

impl App {
    fn new(mut config: PlatformConfig) -> Self {
        let callbacks = config.callbacks.take();
        let clipboard = arboard::Clipboard::new().ok();
        Self {
            config,
            window: None,
            renderer: None,
            terminal: None,
            current_modifiers: winit::event::Modifiers::default(),
            callbacks,
            mouse_pressed: false,
            cursor_position: (0.0, 0.0),
            clipboard,
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Convert pixel position to grid (row, col) using font metrics.
    fn pixel_to_grid(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let renderer = self.renderer.as_ref()?;
        let metrics = renderer.font_metrics();
        let col = (x as f32 / metrics.cell_width) as usize;
        let row = (y as f32 / metrics.cell_height) as usize;
        Some((row, col))
    }

    /// Dispatch a keybinding action.
    fn dispatch_action(&mut self, action: Action) {
        match action {
            Action::Copy => {
                let text = if let Some(cb) = &mut self.callbacks {
                    cb.on_copy()
                } else {
                    None
                };
                if let Some(text) = text {
                    if let Some(clipboard) = &mut self.clipboard {
                        let _ = clipboard.set_text(&text);
                    }
                }
            }
            Action::Paste => {
                let text = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok());
                if let Some(text) = text {
                    if let Some(cb) = &mut self.callbacks {
                        cb.on_paste(&text);
                    }
                }
            }
            _ => {
                if let Some(cb) = &mut self.callbacks {
                    cb.on_action(action);
                } else {
                    log::info!("action: {action:?}");
                }
            }
        }
        self.request_redraw();
    }
}

impl ApplicationHandler for App {
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request a redraw every frame to pick up PTY output
        self.request_redraw();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = default_window_attributes();
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        // Scale font size by DPI factor for crisp rendering on HiDPI displays
        let scale_factor = window.scale_factor() as f32;
        let physical_font_size = self.config.font_size * scale_factor;

        // Initialize renderer
        let renderer = pollster::block_on(Renderer::new(
            window.clone(),
            width,
            height,
            physical_font_size,
        ));

        match renderer {
            Ok(renderer) => {
                let (rows, cols) = renderer.grid_size();
                let terminal = Terminal::new(rows, cols, self.config.scrollback);

                self.renderer = Some(renderer);
                self.terminal = Some(terminal);
            }
            Err(e) => {
                log::error!("failed to initialize renderer: {e}");
                event_loop.exit();
                return;
            }
        }

        self.window = Some(window);
        self.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.current_modifiers = modifiers;
            }

            WindowEvent::Resized(new_size) => {
                let width = new_size.width.max(1);
                let height = new_size.height.max(1);

                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(width, height);
                    let (rows, cols) = renderer.grid_size();

                    if let Some(cb) = &mut self.callbacks {
                        cb.on_resize(rows, cols, width, height);
                    } else if let Some(terminal) = &mut self.terminal {
                        terminal.resize(rows, cols);
                    }
                }
                self.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                // Check if the application wants to exit (e.g., all sessions closed)
                if let Some(cb) = &self.callbacks {
                    if cb.should_exit() {
                        event_loop.exit();
                        return;
                    }
                }

                if let Some(renderer) = &mut self.renderer {
                    let result = if let Some(cb) = &mut self.callbacks {
                        // App-managed rendering: get grids from callbacks
                        let grids = cb.on_tick();
                        let refs: Vec<(&termesh_terminal::grid::GridSnapshot, f32, f32)> =
                            grids.iter().map(|(g, x, y)| (g, *x, *y)).collect();
                        renderer.render_grids(&refs)
                    } else if let Some(terminal) = &self.terminal {
                        // Standalone mode: render single terminal
                        let grid = terminal.render_grid();
                        renderer.render(&grid)
                    } else {
                        Ok(())
                    };

                    match result {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            let (w, h) = renderer.size();
                            renderer.resize(w, h);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::error!("GPU out of memory");
                            event_loop.exit();
                        }
                        Err(e) => {
                            log::warn!("render error: {e}");
                        }
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Try keybinding action first
                let modifiers = input_bridge::convert_modifiers(&self.current_modifiers);
                let has_modifier = modifiers.ctrl || modifiers.alt || modifiers.logo;

                if has_modifier {
                    if let Some(key) = input_bridge::convert_key(&event.logical_key) {
                        if let Some(action) = self.config.input_handler.handle_key(modifiers, key) {
                            self.dispatch_action(action);
                            return;
                        }
                    }
                }

                // Try special keys first (Enter, Backspace, arrows, etc.)
                let special_bytes = match &event.logical_key {
                    winit::keyboard::Key::Named(named) => match named {
                        winit::keyboard::NamedKey::Enter => Some(b"\r".as_slice()),
                        winit::keyboard::NamedKey::Backspace => Some(b"\x7f".as_slice()),
                        winit::keyboard::NamedKey::Tab => Some(b"\t".as_slice()),
                        winit::keyboard::NamedKey::Escape => Some(b"\x1b".as_slice()),
                        winit::keyboard::NamedKey::ArrowUp => Some(b"\x1b[A".as_slice()),
                        winit::keyboard::NamedKey::ArrowDown => Some(b"\x1b[B".as_slice()),
                        winit::keyboard::NamedKey::ArrowRight => Some(b"\x1b[C".as_slice()),
                        winit::keyboard::NamedKey::ArrowLeft => Some(b"\x1b[D".as_slice()),
                        winit::keyboard::NamedKey::Home => Some(b"\x1b[H".as_slice()),
                        winit::keyboard::NamedKey::End => Some(b"\x1b[F".as_slice()),
                        winit::keyboard::NamedKey::Delete => Some(b"\x1b[3~".as_slice()),
                        winit::keyboard::NamedKey::PageUp => Some(b"\x1b[5~".as_slice()),
                        winit::keyboard::NamedKey::PageDown => Some(b"\x1b[6~".as_slice()),
                        winit::keyboard::NamedKey::Insert => Some(b"\x1b[2~".as_slice()),
                        _ => None,
                    },
                    _ => None,
                };

                let input = if let Some(bytes) = special_bytes {
                    Some(bytes.to_vec())
                } else if modifiers.ctrl {
                    // Ctrl+letter → control character (e.g., Ctrl+C = 0x03)
                    if let winit::keyboard::Key::Character(c) = &event.logical_key {
                        let ch = c.as_str().chars().next().unwrap_or('\0');
                        if ch.is_ascii_lowercase() {
                            Some(vec![ch as u8 - b'a' + 1])
                        } else if ch.is_ascii_uppercase() {
                            Some(vec![ch as u8 - b'A' + 1])
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    event.text.as_ref().map(|text| text.as_bytes().to_vec())
                };

                if let Some(bytes) = input {
                    if let Some(cb) = &mut self.callbacks {
                        cb.on_input(&bytes);
                    } else if let Some(terminal) = &mut self.terminal {
                        terminal.feed_bytes(&bytes);
                    }
                    self.request_redraw();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = (position.x, position.y);

                if self.mouse_pressed {
                    if let Some((row, col)) = self.pixel_to_grid(position.x, position.y) {
                        if let Some(cb) = &mut self.callbacks {
                            cb.on_mouse_drag(row, col);
                        }
                        self.request_redraw();
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == winit::event::MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            self.mouse_pressed = true;
                            let (x, y) = self.cursor_position;
                            if let Some((row, col)) = self.pixel_to_grid(x, y) {
                                if let Some(cb) = &mut self.callbacks {
                                    cb.on_mouse_press(row, col);
                                }
                                self.request_redraw();
                            }
                        }
                        ElementState::Released => {
                            self.mouse_pressed = false;
                            if let Some(cb) = &mut self.callbacks {
                                cb.on_mouse_release();
                            }
                        }
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        // Convert pixel delta to lines (roughly cell_height per line)
                        (pos.y / 20.0) as i32
                    }
                };

                if lines != 0 {
                    if let Some(cb) = &mut self.callbacks {
                        cb.on_scroll(lines);
                    } else if let Some(terminal) = &mut self.terminal {
                        if lines > 0 {
                            terminal.scroll_up(lines as usize);
                        } else {
                            terminal.scroll_down((-lines) as usize);
                        }
                    }
                    self.request_redraw();
                }
            }

            _ => {}
        }
    }
}

/// Run the platform event loop.
///
/// This is the main entry point that blocks until the window is closed.
pub fn run(config: PlatformConfig) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new(config);
    event_loop.run_app(&mut app)?;
    Ok(())
}
