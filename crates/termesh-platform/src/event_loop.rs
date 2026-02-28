//! Main event loop integrating winit, wgpu renderer, and terminal.

use crate::window::default_window_attributes;
use std::sync::Arc;
use termesh_renderer::renderer::Renderer;
use termesh_terminal::terminal::Terminal;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

/// Configuration for launching the platform event loop.
pub struct PlatformConfig {
    /// Font size in points.
    pub font_size: f32,
    /// Terminal scrollback lines.
    pub scrollback: usize,
}

impl Default for PlatformConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            scrollback: 10_000,
        }
    }
}

/// Application state managed by the winit event loop.
struct App {
    config: PlatformConfig,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    terminal: Option<Terminal>,
}

impl App {
    fn new(config: PlatformConfig) -> Self {
        Self {
            config,
            window: None,
            renderer: None,
            terminal: None,
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for App {
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

        // Initialize renderer
        let renderer = pollster::block_on(Renderer::new(
            window.clone(),
            width,
            height,
            self.config.font_size,
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

            WindowEvent::Resized(new_size) => {
                let width = new_size.width.max(1);
                let height = new_size.height.max(1);

                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(width, height);

                    let (rows, cols) = renderer.grid_size();
                    if let Some(terminal) = &mut self.terminal {
                        terminal.resize(rows, cols);
                    }
                }
                self.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                if let (Some(renderer), Some(terminal)) = (&mut self.renderer, &self.terminal) {
                    let grid = terminal.render_grid();
                    match renderer.render(&grid) {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost) => {
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
                if event.state == ElementState::Pressed {
                    if let Some(text) = &event.text {
                        if let Some(terminal) = &mut self.terminal {
                            terminal.feed_bytes(text.as_bytes());
                            self.request_redraw();
                        }
                    }
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
