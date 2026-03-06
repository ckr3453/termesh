//! Main event loop integrating winit, wgpu renderer, and terminal.

use crate::input_bridge;
use crate::window::default_window_attributes;
use std::sync::Arc;
use std::time::{Duration, Instant};
use termesh_input::action::Action;
use termesh_input::handler::InputHandler;
use termesh_renderer::renderer::Renderer;
use termesh_terminal::terminal::Terminal;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
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

    /// Returns divider lines for pane borders.
    /// Each entry is (x, y, length, is_vertical, color_rgba).
    fn dividers(&self) -> Vec<(f32, f32, f32, bool, [f32; 4])>;

    /// Called when the window is resized.
    /// `rows`/`cols` are grid dimensions, `width`/`height` are pixel dimensions.
    /// `cell_w`/`cell_h` are font cell dimensions in pixels for per-pane grid calculation.
    fn on_resize(
        &mut self,
        rows: usize,
        cols: usize,
        width: u32,
        height: u32,
        cell_w: f32,
        cell_h: f32,
    );

    /// Called when the user scrolls (mouse wheel / trackpad).
    /// Positive delta = scroll up (view older output), negative = scroll down.
    fn on_scroll(&mut self, delta: i32);

    /// Called when the mouse button is pressed at a pixel coordinate.
    fn on_mouse_press(&mut self, x: f64, y: f64);

    /// Called when the mouse is dragged to a pixel coordinate (selection update).
    fn on_mouse_drag(&mut self, x: f64, y: f64);

    /// Called when the mouse button is released.
    fn on_mouse_release(&mut self);

    /// Called to copy selection text. Returns the selected text if any.
    fn on_copy(&mut self) -> Option<String>;

    /// Called to paste text from clipboard.
    fn on_paste(&mut self, text: &str);

    /// Returns true if the application should exit (e.g., no sessions left).
    fn should_exit(&self) -> bool;

    /// Returns true if there is pending PTY output that needs rendering.
    ///
    /// Used by the event loop to decide whether to schedule a redraw
    /// even when no user input has occurred.
    fn has_pending_output(&self) -> bool;

    /// Called when IME preedit text changes.
    /// Empty text means preedit ended.
    fn on_preedit(&mut self, _text: &str, _cursor_pos: Option<(usize, usize)>) {}

    /// Returns the pixel position and size of the focused cursor for IME positioning.
    /// Format: (x, y, width, height) in physical pixels.
    fn ime_cursor_area(&self) -> Option<(f32, f32, f32, f32)> {
        None
    }
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

/// Cursor blink interval for scheduling periodic redraws.
const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);

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
    /// Whether the UI needs a redraw due to user input or state change.
    dirty: bool,
    /// Current IME preedit text (None when not composing).
    preedit_text: Option<String>,
}

impl App {
    fn new(mut config: PlatformConfig) -> Self {
        let callbacks = config.callbacks.take();
        // Lazy-initialize clipboard on first copy/paste to avoid
        // triggering macOS TCC permission prompts at app startup.
        let clipboard = None;
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
            dirty: true,
            preedit_text: None,
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
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
                    if self.clipboard.is_none() {
                        match arboard::Clipboard::new() {
                            Ok(cb) => self.clipboard = Some(cb),
                            Err(e) => log::warn!("failed to init clipboard: {e}"),
                        }
                    }
                    if let Some(clipboard) = &mut self.clipboard {
                        if let Err(e) = clipboard.set_text(&text) {
                            log::warn!("failed to copy to clipboard: {e}");
                        }
                    }
                }
            }
            Action::Paste => {
                if self.clipboard.is_none() {
                    match arboard::Clipboard::new() {
                        Ok(cb) => self.clipboard = Some(cb),
                        Err(e) => log::warn!("failed to init clipboard: {e}"),
                    }
                }
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
        self.dirty = true;
        self.request_redraw();
    }
}

impl ApplicationHandler for App {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let has_pending = self
            .callbacks
            .as_ref()
            .map_or(false, |cb| cb.has_pending_output());

        if self.dirty || has_pending {
            self.request_redraw();
        }

        // Schedule a periodic redraw for cursor blink
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + CURSOR_BLINK_INTERVAL,
        ));
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
        log::info!(
            "DPI: scale_factor={:.2}, font={:.1}→{:.1}pt, window={}x{}",
            scale_factor,
            self.config.font_size,
            physical_font_size,
            width,
            height
        );

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

        window.set_ime_allowed(true);
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
                        let metrics = renderer.font_metrics();
                        cb.on_resize(
                            rows,
                            cols,
                            width,
                            height,
                            metrics.cell_width,
                            metrics.cell_height,
                        );
                    } else if let Some(terminal) = &mut self.terminal {
                        terminal.resize(rows, cols);
                    }
                }
                self.dirty = true;
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

                // Pre-compute preedit cursor position for app-managed mode
                // (standalone mode derives it from the same render_grid call below)
                let preedit_app_pos: Option<(f32, f32)> =
                    match (&self.preedit_text, &self.callbacks) {
                        (Some(text), _) if text.is_empty() => None,
                        (Some(_), Some(cb)) => {
                            cb.ime_cursor_area().map(|(x, y, _, _)| (x, y))
                        }
                        _ => None,
                    };

                if let Some(renderer) = &mut self.renderer {
                    let result = if let Some(cb) = &mut self.callbacks {
                        // App-managed rendering: get grids from callbacks
                        let preedit_overlay = self
                            .preedit_text
                            .as_ref()
                            .filter(|t| !t.is_empty())
                            .zip(preedit_app_pos)
                            .map(|(text, (x, y))| termesh_renderer::renderer::PreeditOverlay {
                                text: text.clone(),
                                x,
                                y,
                            });
                        let grids = cb.on_tick();
                        let dividers = cb.dividers();
                        let refs: Vec<(&termesh_terminal::grid::GridSnapshot, f32, f32)> =
                            grids.iter().map(|(g, x, y)| (g, *x, *y)).collect();
                        renderer.render_grids(&refs, &dividers, preedit_overlay.as_ref())
                    } else if let Some(terminal) = &mut self.terminal {
                        // Standalone mode: single render_grid call for both cursor pos and rendering
                        let grid = terminal.render_grid();
                        let preedit_overlay = self
                            .preedit_text
                            .as_ref()
                            .filter(|t| !t.is_empty())
                            .map(|text| {
                                let metrics = renderer.font_metrics();
                                termesh_renderer::renderer::PreeditOverlay {
                                    text: text.clone(),
                                    x: grid.cursor.col as f32 * metrics.cell_width,
                                    y: grid.cursor.row as f32 * metrics.cell_height,
                                }
                            });
                        renderer.render_grids(
                            &[(&grid, 0.0, 0.0)],
                            &[],
                            preedit_overlay.as_ref(),
                        )
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
                self.dirty = false;
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Try keybinding action first
                let modifiers = input_bridge::convert_modifiers(&self.current_modifiers);
                let has_modifier = modifiers.ctrl || modifiers.alt || modifiers.logo;

                log::debug!(
                    "KEY: {:?} phys={:?} has_mod={} mods={:?}",
                    event.logical_key,
                    event.physical_key,
                    has_modifier,
                    modifiers
                );

                if has_modifier {
                    // Try keybinding lookup
                    let key = input_bridge::convert_key(&event.logical_key)
                        .or_else(|| input_bridge::convert_physical_key(&event.physical_key));
                    if let Some(key) = key {
                        let action = self.config.input_handler.handle_key(modifiers, key);
                        if let Some(action) = action {
                            log::info!("keybinding matched → {action:?}");
                            self.dispatch_action(action);
                            return;
                        }
                    }

                    // Cmd/Logo key: if no binding matched, drop it.
                    // This is standard macOS behavior — Cmd+key is always for
                    // the app, never forwarded to the PTY.
                    if modifiers.logo {
                        return;
                    }
                }

                // xterm modifier parameter: 1 + (shift?1) + (alt?2) + (ctrl?4)
                let mod_param: u8 = 1
                    + if modifiers.shift { 1 } else { 0 }
                    + if modifiers.alt { 2 } else { 0 }
                    + if modifiers.ctrl { 4 } else { 0 };

                // Special keys with modifier encoding (xterm convention)
                let input: Option<Vec<u8>> = match &event.logical_key {
                    winit::keyboard::Key::Named(named) => match named {
                        winit::keyboard::NamedKey::Enter => {
                            if modifiers.alt {
                                Some(b"\x1b\r".to_vec())
                            } else {
                                Some(b"\r".to_vec())
                            }
                        }
                        winit::keyboard::NamedKey::Backspace => Some(b"\x7f".to_vec()),
                        winit::keyboard::NamedKey::Tab => {
                            if modifiers.shift {
                                Some(b"\x1b[Z".to_vec())
                            } else {
                                Some(b"\t".to_vec())
                            }
                        }
                        winit::keyboard::NamedKey::Escape => Some(b"\x1b".to_vec()),
                        winit::keyboard::NamedKey::ArrowUp => Some(csi_key(b'A', mod_param)),
                        winit::keyboard::NamedKey::ArrowDown => Some(csi_key(b'B', mod_param)),
                        winit::keyboard::NamedKey::ArrowRight => Some(csi_key(b'C', mod_param)),
                        winit::keyboard::NamedKey::ArrowLeft => Some(csi_key(b'D', mod_param)),
                        winit::keyboard::NamedKey::Home => Some(csi_key(b'H', mod_param)),
                        winit::keyboard::NamedKey::End => Some(csi_key(b'F', mod_param)),
                        winit::keyboard::NamedKey::Delete => Some(csi_tilde(3, mod_param)),
                        winit::keyboard::NamedKey::PageUp => Some(csi_tilde(5, mod_param)),
                        winit::keyboard::NamedKey::PageDown => Some(csi_tilde(6, mod_param)),
                        winit::keyboard::NamedKey::Insert => Some(csi_tilde(2, mod_param)),
                        _ => None,
                    },
                    _ => None,
                };

                // If special key was handled, send it and stop
                let input = if input.is_some() {
                    input
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
                        // Ctrl+letter: recover letter from physical key
                        match input_bridge::convert_physical_key(&event.physical_key) {
                            Some(termesh_input::keymap::Key::Char(c))
                                if c.is_ascii_alphabetic() =>
                            {
                                Some(vec![c as u8 - b'a' + 1])
                            }
                            _ => None,
                        }
                    }
                } else if modifiers.alt {
                    // Alt+letter → ESC prefix + letter (standard terminal meta key)
                    match input_bridge::convert_physical_key(&event.physical_key) {
                        Some(termesh_input::keymap::Key::Char(c)) => {
                            Some(vec![0x1b, c as u8])
                        }
                        _ => None,
                    }
                } else {
                    // Regular character: let Ime::Commit handle it.
                    // Do NOT use event.text here to avoid double input
                    // (winit fires both KeyboardInput and Ime::Commit when IME is active).
                    None
                };

                if let Some(bytes) = input {
                    if let Some(cb) = &mut self.callbacks {
                        cb.on_input(&bytes);
                    } else if let Some(terminal) = &mut self.terminal {
                        terminal.feed_bytes(&bytes);
                    }
                    self.dirty = true;
                    self.request_redraw();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = (position.x, position.y);

                if self.mouse_pressed {
                    if let Some(cb) = &mut self.callbacks {
                        cb.on_mouse_drag(position.x, position.y);
                    }
                    self.dirty = true;
                    self.request_redraw();
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == winit::event::MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            self.mouse_pressed = true;
                            let (x, y) = self.cursor_position;
                            if let Some(cb) = &mut self.callbacks {
                                cb.on_mouse_press(x, y);
                            }
                            self.dirty = true;
                            self.request_redraw();
                        }
                        ElementState::Released => {
                            self.mouse_pressed = false;
                            if let Some(cb) = &mut self.callbacks {
                                cb.on_mouse_release();
                            }
                            self.dirty = true;
                            self.request_redraw();
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
                    self.dirty = true;
                    self.request_redraw();
                }
            }

            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.preedit_text = None;
                if let Some(cb) = &mut self.callbacks {
                    cb.on_preedit("", None);
                }
                let bytes = text.as_bytes().to_vec();
                if let Some(cb) = &mut self.callbacks {
                    cb.on_input(&bytes);
                } else if let Some(terminal) = &mut self.terminal {
                    terminal.feed_bytes(&bytes);
                }
                self.dirty = true;
                self.request_redraw();
            }

            WindowEvent::Ime(winit::event::Ime::Preedit(text, cursor_pos)) => {
                if text.is_empty() {
                    self.preedit_text = None;
                } else {
                    self.preedit_text = Some(text.clone());
                }
                if let Some(cb) = &mut self.callbacks {
                    cb.on_preedit(&text, cursor_pos);
                }
                // Sync OS IME candidate window position
                let area = if let Some(cb) = &self.callbacks {
                    cb.ime_cursor_area()
                } else if let (Some(terminal), Some(renderer)) =
                    (&mut self.terminal, &self.renderer)
                {
                    let metrics = renderer.font_metrics();
                    let grid = terminal.render_grid();
                    Some((
                        grid.cursor.col as f32 * metrics.cell_width,
                        grid.cursor.row as f32 * metrics.cell_height,
                        metrics.cell_width,
                        metrics.cell_height,
                    ))
                } else {
                    None
                };
                if let (Some(window), Some((x, y, w, h))) = (&self.window, area) {
                    use winit::dpi::{PhysicalPosition, PhysicalSize};
                    window.set_ime_cursor_area(
                        PhysicalPosition::new(x as f64, y as f64),
                        PhysicalSize::new(w as f64, h as f64),
                    );
                }
                self.dirty = true;
                self.request_redraw();
            }

            WindowEvent::Ime(_) => {}

            _ => {}
        }
    }
}

/// Encode a CSI key sequence with optional xterm modifier.
///
/// Plain: `\x1b[{suffix}`, Modified: `\x1b[1;{mod}{suffix}`
fn csi_key(suffix: u8, mod_param: u8) -> Vec<u8> {
    if mod_param > 1 {
        format!("\x1b[1;{}{}", mod_param, suffix as char).into_bytes()
    } else {
        vec![0x1b, b'[', suffix]
    }
}

/// Encode a CSI tilde key sequence with optional xterm modifier.
///
/// Plain: `\x1b[{num}~`, Modified: `\x1b[{num};{mod}~`
fn csi_tilde(num: u8, mod_param: u8) -> Vec<u8> {
    if mod_param > 1 {
        format!("\x1b[{num};{mod_param}~").into_bytes()
    } else {
        format!("\x1b[{num}~").into_bytes()
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
