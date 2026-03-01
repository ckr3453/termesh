//! Manages multiple PTY sessions and their associated terminals.

use std::collections::HashMap;
use termesh_agent::registry::AdapterRegistry;
use termesh_core::error::PtyError;
use termesh_core::types::{AgentState, SessionId};
use termesh_pty::pty::{PtyResizer, PtyWriter};
use termesh_pty::session::{Session, SessionConfig, SessionOutput};
use termesh_terminal::terminal::Terminal;
use tokio::sync::mpsc;

/// Default terminal scrollback lines.
const DEFAULT_SCROLLBACK: usize = 10_000;

/// Strip ANSI escape sequences from text for agent state analysis.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // CSI sequence: ESC [ ... final_byte
            if chars.peek() == Some(&'[') {
                chars.next();
                // Consume until a letter (@ through ~) is found
                for c2 in chars.by_ref() {
                    if c2.is_ascii_alphabetic() || c2 == '~' || c2 == '@' {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequence: ESC ] ... ST (ESC \ or BEL)
                chars.next();
                for c2 in chars.by_ref() {
                    if c2 == '\x07' {
                        break;
                    }
                    if c2 == '\x1b' {
                        chars.next(); // consume '\'
                        break;
                    }
                }
            } else {
                // Other ESC sequence: skip next char
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A PTY session paired with its terminal emulator.
struct ManagedSession {
    /// Writer handle for sending input to the PTY.
    writer: PtyWriter,
    /// Resizer handle for notifying the PTY of terminal size changes.
    resizer: PtyResizer,
    /// Terminal emulator that processes PTY output.
    terminal: Terminal,
    /// Which agent adapter (if any) handles this session.
    adapter_id: Option<String>,
    /// Current detected agent state.
    agent_state: AgentState,
}

/// Output received from any session.
pub struct SessionEvent {
    /// Which session produced this output.
    pub session_id: SessionId,
    /// The output data.
    pub output: SessionOutput,
}

/// Manages multiple sessions, each with a PTY + Terminal pair.
pub struct SessionManager {
    sessions: HashMap<SessionId, ManagedSession>,
    /// Active session that receives keyboard input.
    active: Option<SessionId>,
    /// Aggregated output receiver from all sessions.
    event_rx: mpsc::Receiver<SessionEvent>,
    /// Sender clone for spawning new session readers.
    event_tx: mpsc::Sender<SessionEvent>,
    /// Agent adapter registry for detecting agent state from PTY output.
    registry: AdapterRegistry,
}

impl SessionManager {
    /// Create a new empty session manager.
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(1024);
        Self {
            sessions: HashMap::new(),
            active: None,
            event_rx,
            event_tx,
            registry: AdapterRegistry::with_defaults(),
        }
    }

    /// Spawn a new session and return its ID.
    pub fn spawn(&mut self, config: SessionConfig) -> Result<SessionId, PtyError> {
        let rows = config.rows;
        let cols = config.cols;
        let command = config.command.clone();
        let agent_hint = config.agent.clone();
        let config_name = config.name.clone();

        let mut session = Session::spawn(config)?;
        let id = session.id;

        // Take the writer and resizer before start_reader consumes the session
        let writer = session.take_writer().ok_or_else(|| PtyError::SpawnFailed {
            reason: "failed to take PTY writer".to_string(),
        })?;
        let resizer = session.resizer();

        let terminal = Terminal::new(rows as usize, cols as usize, DEFAULT_SCROLLBACK);

        // Start the background reader thread
        let (_handle, mut output_rx) = session.start_reader();
        let event_tx = self.event_tx.clone();
        let session_id = id;

        // Forward PTY output to the aggregated event channel
        tokio::spawn(async move {
            while let Some(output) = output_rx.recv().await {
                let is_exit = matches!(output, SessionOutput::Exited(_));
                let _ = event_tx.send(SessionEvent { session_id, output }).await;
                if is_exit {
                    break;
                }
            }
        });

        // Detect if this session is an agent.
        // Prefer the config.agent hint (handles Windows cmd.exe wrapping),
        // fall back to command-based detection.
        let adapter_id = if agent_hint != "none" {
            // Try config.agent as adapter id first, then config.name
            self.registry
                .get(&agent_hint)
                .map(|_| agent_hint.clone())
                .or_else(|| {
                    self.registry
                        .detect_agent(&config_name)
                        .map(|s| s.to_string())
                })
                .or_else(|| self.registry.detect_agent(&command).map(|s| s.to_string()))
        } else {
            self.registry.detect_agent(&command).map(|s| s.to_string())
        };
        let agent_state = if adapter_id.is_some() {
            AgentState::Idle
        } else {
            AgentState::None
        };
        log::info!("session {id}: command={command:?}, agent={agent_hint}, adapter={adapter_id:?}");

        self.sessions.insert(
            id,
            ManagedSession {
                writer,
                resizer,
                terminal,
                adapter_id,
                agent_state,
            },
        );

        // Auto-activate the first session
        if self.active.is_none() {
            self.active = Some(id);
        }

        Ok(id)
    }

    /// Remove a session and clean up resources.
    pub fn remove(&mut self, id: SessionId) {
        self.sessions.remove(&id);
        if self.active == Some(id) {
            self.active = self.sessions.keys().next().copied();
        }
    }

    /// Get the active session ID.
    pub fn active(&self) -> Option<SessionId> {
        self.active
    }

    /// Set the active session.
    pub fn set_active(&mut self, id: SessionId) {
        if self.sessions.contains_key(&id) {
            self.active = Some(id);
        }
    }

    /// Get a reference to a session's terminal.
    pub fn terminal(&self, id: SessionId) -> Option<&Terminal> {
        self.sessions.get(&id).map(|s| &s.terminal)
    }

    /// Get a mutable reference to a session's terminal.
    pub fn terminal_mut(&mut self, id: SessionId) -> Option<&mut Terminal> {
        self.sessions.get_mut(&id).map(|s| &mut s.terminal)
    }

    /// Send input to the active session's PTY.
    ///
    /// Automatically scrolls the viewport to the bottom so the user
    /// sees the latest output after typing.
    pub fn write_active(&mut self, data: &[u8]) -> Result<(), PtyError> {
        if let Some(id) = self.active {
            if let Some(session) = self.sessions.get_mut(&id) {
                session.terminal.scroll_to_bottom();
                session.writer.write(data)?;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Send input to a specific session's PTY.
    ///
    /// Automatically scrolls the viewport to the bottom.
    pub fn write_to(&mut self, id: SessionId, data: &[u8]) -> Result<(), PtyError> {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.terminal.scroll_to_bottom();
            session.writer.write(data)?;
        }
        Ok(())
    }

    /// Process pending session output — feed PTY data into terminals.
    ///
    /// Returns the IDs of sessions that exited during this tick.
    pub fn process_events(&mut self) -> Vec<SessionId> {
        let mut exited = Vec::new();
        // Collect adapter analysis results: (session_id, adapter_id, data_text)
        let mut analyze_queue: Vec<(SessionId, String, String)> = Vec::new();

        while let Ok(event) = self.event_rx.try_recv() {
            match event.output {
                SessionOutput::Data(data) => {
                    if let Some(session) = self.sessions.get_mut(&event.session_id) {
                        session.terminal.feed_bytes(&data);
                        // Queue for agent analysis if this session has an adapter
                        if let Some(adapter_id) = &session.adapter_id {
                            let text = String::from_utf8_lossy(&data);
                            if !text.trim().is_empty() {
                                analyze_queue.push((
                                    event.session_id,
                                    adapter_id.clone(),
                                    text.into_owned(),
                                ));
                            }
                        }
                    }
                }
                SessionOutput::Exited(_code) => {
                    exited.push(event.session_id);
                }
            }
        }

        // Apply agent state analysis (strip ANSI escapes first)
        for (session_id, adapter_id, raw_text) in analyze_queue {
            let text = strip_ansi(&raw_text);
            // Log non-empty stripped text for pattern debugging
            for line in text.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() && trimmed.len() > 1 {
                    log::debug!("agent-pty [{session_id}]: {trimmed:?}");
                }
            }
            if let Some(adapter) = self.registry.get(&adapter_id) {
                if let Some(new_state) = adapter.analyze_output(&text) {
                    if let Some(session) = self.sessions.get_mut(&session_id) {
                        if session.agent_state != new_state {
                            log::info!(
                                "session {session_id}: agent state {:?} → {new_state:?}",
                                session.agent_state
                            );
                            session.agent_state = new_state;
                        }
                    }
                }
            }
        }

        for &id in &exited {
            log::info!("session {id} exited, removing");
            self.remove(id);
        }

        exited
    }

    /// Resize a session's terminal and notify the PTY.
    pub fn resize(&mut self, id: SessionId, rows: usize, cols: usize) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.terminal.resize(rows, cols);
            if let Err(e) = session.resizer.resize(rows as u16, cols as u16) {
                log::warn!("failed to resize PTY for session {id}: {e}");
            }
        }
    }

    /// Resize all sessions' terminals and notify their PTYs.
    pub fn resize_all(&mut self, rows: usize, cols: usize) {
        for (id, session) in &mut self.sessions {
            session.terminal.resize(rows, cols);
            if let Err(e) = session.resizer.resize(rows as u16, cols as u16) {
                log::warn!("failed to resize PTY for session {id}: {e}");
            }
        }
    }

    /// Number of active sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether there are no sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Get all session IDs.
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().copied().collect()
    }

    /// Get the agent state for a session.
    pub fn agent_state(&self, id: SessionId) -> AgentState {
        self.sessions
            .get(&id)
            .map(|s| s.agent_state)
            .unwrap_or(AgentState::None)
    }

    /// Check if a session is an agent session.
    pub fn is_agent(&self, id: SessionId) -> bool {
        self.sessions
            .get(&id)
            .map(|s| s.adapter_id.is_some())
            .unwrap_or(false)
    }

    /// Get the agent kind (adapter_id) for a session, or "shell" if none.
    pub fn agent_kind(&self, id: SessionId) -> &str {
        self.sessions
            .get(&id)
            .and_then(|s| s.adapter_id.as_deref())
            .unwrap_or("shell")
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SessionConfig {
        SessionConfig {
            #[cfg(windows)]
            command: "cmd.exe".to_string(),
            #[cfg(not(windows))]
            command: "sh".to_string(),
            rows: 24,
            cols: 80,
            ..Default::default()
        }
    }

    #[test]
    fn test_new_session_manager() {
        let mgr = SessionManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.active(), None);
    }

    #[tokio::test]
    async fn test_spawn_session() {
        let mut mgr = SessionManager::new();
        let id = mgr.spawn(test_config()).unwrap();
        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.active(), Some(id));
        assert!(mgr.terminal(id).is_some());
    }

    #[tokio::test]
    async fn test_spawn_multiple_sessions() {
        let mut mgr = SessionManager::new();
        let id1 = mgr.spawn(test_config()).unwrap();
        let id2 = mgr.spawn(test_config()).unwrap();
        assert_eq!(mgr.len(), 2);
        assert_eq!(mgr.active(), Some(id1));

        mgr.set_active(id2);
        assert_eq!(mgr.active(), Some(id2));
    }

    #[tokio::test]
    async fn test_remove_session() {
        let mut mgr = SessionManager::new();
        let id1 = mgr.spawn(test_config()).unwrap();
        let id2 = mgr.spawn(test_config()).unwrap();

        mgr.remove(id1);
        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.active(), Some(id2));
    }

    #[tokio::test]
    async fn test_remove_last_session() {
        let mut mgr = SessionManager::new();
        let id = mgr.spawn(test_config()).unwrap();
        mgr.remove(id);
        assert!(mgr.is_empty());
        assert_eq!(mgr.active(), None);
    }

    #[tokio::test]
    async fn test_resize_session() {
        let mut mgr = SessionManager::new();
        let id = mgr.spawn(test_config()).unwrap();
        mgr.resize(id, 40, 120);
        let term = mgr.terminal(id).unwrap();
        assert_eq!(term.rows(), 40);
        assert_eq!(term.cols(), 120);
    }

    #[tokio::test]
    async fn test_set_active_invalid() {
        let mut mgr = SessionManager::new();
        let id = mgr.spawn(test_config()).unwrap();
        mgr.set_active(SessionId(9999));
        assert_eq!(mgr.active(), Some(id));
    }

    #[tokio::test]
    async fn test_session_ids() {
        let mut mgr = SessionManager::new();
        let id1 = mgr.spawn(test_config()).unwrap();
        let id2 = mgr.spawn(test_config()).unwrap();
        let ids = mgr.session_ids();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[tokio::test]
    async fn test_write_and_receive_output() {
        let mut mgr = SessionManager::new();
        let id = mgr.spawn(test_config()).unwrap();

        // Write a command
        #[cfg(windows)]
        mgr.write_active(b"echo hello\r\n").unwrap();
        #[cfg(not(windows))]
        mgr.write_active(b"echo hello\n").unwrap();

        // Wait for output (cmd.exe can be slow on Windows)
        for _ in 0..10 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let _exited = mgr.process_events();
            let grid = mgr.terminal(id).unwrap().render_grid();
            if grid.cells.iter().any(|c| c.c != ' ' && c.c != '\0') {
                return; // success
            }
        }
        panic!("terminal should have rendered content after 2s");
    }
}
