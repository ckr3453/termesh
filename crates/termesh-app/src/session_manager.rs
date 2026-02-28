//! Manages multiple PTY sessions and their associated terminals.

use std::collections::HashMap;
use termesh_core::error::PtyError;
use termesh_core::types::SessionId;
use termesh_pty::pty::PtyWriter;
use termesh_pty::session::{Session, SessionConfig, SessionOutput};
use termesh_terminal::terminal::Terminal;
use tokio::sync::mpsc;

/// Default terminal scrollback lines.
const DEFAULT_SCROLLBACK: usize = 10_000;

/// A PTY session paired with its terminal emulator.
struct ManagedSession {
    /// Writer handle for sending input to the PTY.
    writer: PtyWriter,
    /// Terminal emulator that processes PTY output.
    terminal: Terminal,
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
        }
    }

    /// Spawn a new session and return its ID.
    pub fn spawn(&mut self, config: SessionConfig) -> Result<SessionId, PtyError> {
        let rows = config.rows;
        let cols = config.cols;

        let mut session = Session::spawn(config)?;
        let id = session.id;

        // Take the writer before start_reader consumes the session
        let writer = session.take_writer().ok_or_else(|| PtyError::SpawnFailed {
            reason: "failed to take PTY writer".to_string(),
        })?;

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

        self.sessions
            .insert(id, ManagedSession { writer, terminal });

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
    pub fn write_active(&mut self, data: &[u8]) -> Result<(), PtyError> {
        if let Some(id) = self.active {
            if let Some(session) = self.sessions.get_mut(&id) {
                session.writer.write(data)?;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Send input to a specific session's PTY.
    pub fn write_to(&mut self, id: SessionId, data: &[u8]) -> Result<(), PtyError> {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.writer.write(data)?;
        }
        Ok(())
    }

    /// Process pending session output — feed PTY data into terminals.
    ///
    /// Returns the number of events processed.
    pub fn process_events(&mut self) -> usize {
        let mut count = 0;
        let mut exited = Vec::new();

        while let Ok(event) = self.event_rx.try_recv() {
            match event.output {
                SessionOutput::Data(data) => {
                    if let Some(session) = self.sessions.get_mut(&event.session_id) {
                        session.terminal.feed_bytes(&data);
                        count += 1;
                    }
                }
                SessionOutput::Exited(_code) => {
                    exited.push(event.session_id);
                    count += 1;
                }
            }
        }

        for id in exited {
            self.remove(id);
        }

        count
    }

    /// Resize a session's terminal.
    pub fn resize(&mut self, id: SessionId, rows: usize, cols: usize) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.terminal.resize(rows, cols);
        }
    }

    /// Resize all sessions' terminals.
    pub fn resize_all(&mut self, rows: usize, cols: usize) {
        for session in self.sessions.values_mut() {
            session.terminal.resize(rows, cols);
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

        // Wait for output
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Process events should feed data into terminal
        let count = mgr.process_events();
        assert!(count > 0, "expected PTY output events");

        // Terminal should have content
        let grid = mgr.terminal(id).unwrap().render_grid();
        let has_content = grid.cells.iter().any(|c| c.c != ' ' && c.c != '\0');
        assert!(has_content, "terminal should have rendered content");
    }
}
