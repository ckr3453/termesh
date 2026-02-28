//! Terminal session lifecycle management.

use crate::pty::Pty;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use termesh_core::error::PtyError;
use termesh_core::types::{AgentState, SessionId};
use tokio::sync::mpsc;

/// Global session ID counter.
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Configuration for spawning a new session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Display name for this session.
    pub name: String,
    /// Command to execute.
    pub command: String,
    /// Arguments for the command.
    pub args: Vec<String>,
    /// Working directory.
    pub cwd: Option<PathBuf>,
    /// Agent type (e.g., "claude", "none").
    pub agent: String,
    /// Initial terminal rows.
    pub rows: u16,
    /// Initial terminal columns.
    pub cols: u16,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            name: "shell".to_string(),
            command: default_shell(),
            args: Vec::new(),
            cwd: None,
            agent: "none".to_string(),
            rows: 24,
            cols: 80,
        }
    }
}

/// A terminal session wrapping a PTY process.
pub struct Session {
    /// Unique session identifier.
    pub id: SessionId,
    /// Session configuration.
    pub config: SessionConfig,
    /// Current agent state.
    pub agent_state: AgentState,
    /// Underlying PTY handle.
    pty: Pty,
}

/// Output message from a session's read thread.
#[derive(Debug)]
pub enum SessionOutput {
    /// Raw bytes from the PTY.
    Data(Vec<u8>),
    /// The process has exited.
    Exited(Option<u32>),
}

impl Session {
    /// Spawn a new session with the given configuration.
    pub fn spawn(config: SessionConfig) -> Result<Self, PtyError> {
        let id = SessionId(NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed));

        let pty = Pty::spawn(
            &config.command,
            &config.args,
            config.cwd.as_deref(),
            config.rows,
            config.cols,
        )?;

        let agent_state = if config.agent == "none" {
            AgentState::None
        } else {
            AgentState::Idle
        };

        Ok(Self {
            id,
            config,
            agent_state,
            pty,
        })
    }

    /// Write user input to the PTY.
    pub fn write(&mut self, data: &[u8]) -> Result<usize, PtyError> {
        self.pty.write_input(data)
    }

    /// Read output from the PTY (blocking).
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        self.pty.read_output(buf)
    }

    /// Resize the terminal.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        self.pty.resize(rows, cols)
    }

    /// Check if the child process has exited.
    pub fn try_wait(&mut self) -> Result<Option<u32>, PtyError> {
        self.pty.try_wait()
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<(), PtyError> {
        self.pty.kill()
    }

    /// Kill the current process and spawn a new one with the same config.
    pub fn restart(&mut self) -> Result<(), PtyError> {
        let _ = self.pty.kill();
        self.pty = Pty::spawn(
            &self.config.command,
            &self.config.args,
            self.config.cwd.as_deref(),
            self.config.rows,
            self.config.cols,
        )?;
        if self.config.agent != "none" {
            self.agent_state = AgentState::Idle;
        }
        Ok(())
    }

    /// Start a background reader thread that sends output through a channel.
    ///
    /// Returns a receiver for `SessionOutput` messages.
    /// The reader thread runs until the PTY closes or an error occurs.
    pub fn start_reader(mut self) -> (SessionHandle, mpsc::Receiver<SessionOutput>) {
        let (tx, rx) = mpsc::channel(256);
        let id = self.id;

        let join = std::thread::Builder::new()
            .name(format!("pty-reader-{}", id))
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match self.pty.read_output(&mut buf) {
                        Ok(0) => {
                            let exit_code = self.pty.try_wait().ok().flatten();
                            let _ = tx.blocking_send(SessionOutput::Exited(exit_code));
                            break;
                        }
                        Ok(n) => {
                            if tx
                                .blocking_send(SessionOutput::Data(buf[..n].to_vec()))
                                .is_err()
                            {
                                break; // Receiver dropped
                            }
                        }
                        Err(_) => {
                            let exit_code = self.pty.try_wait().ok().flatten();
                            let _ = tx.blocking_send(SessionOutput::Exited(exit_code));
                            break;
                        }
                    }
                }
                self
            })
            .expect("failed to spawn PTY reader thread");

        let handle = SessionHandle { id, join };
        (handle, rx)
    }
}

/// Handle to a session whose reader is running in a background thread.
pub struct SessionHandle {
    /// Session identifier.
    pub id: SessionId,
    join: std::thread::JoinHandle<Session>,
}

impl SessionHandle {
    /// Wait for the reader thread to finish and reclaim the session.
    pub fn join(self) -> Result<Session, PtyError> {
        self.join.join().map_err(|_| PtyError::SpawnFailed {
            reason: "reader thread panicked".to_string(),
        })
    }
}

fn default_shell() -> String {
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
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
            ..Default::default()
        }
    }

    #[test]
    fn test_spawn_session() {
        let mut session = Session::spawn(test_config()).unwrap();
        assert_eq!(session.agent_state, AgentState::None);
        assert!(session.try_wait().unwrap().is_none());
        session.kill().unwrap();
    }

    #[test]
    fn test_session_ids_are_unique() {
        let s1 = Session::spawn(test_config()).unwrap();
        let s2 = Session::spawn(test_config()).unwrap();
        assert_ne!(s1.id, s2.id);

        let mut s1 = s1;
        let mut s2 = s2;
        s1.kill().unwrap();
        s2.kill().unwrap();
    }

    #[test]
    fn test_session_write_and_read() {
        let mut session = Session::spawn(test_config()).unwrap();

        #[cfg(windows)]
        let cmd = b"echo hello\r\n";
        #[cfg(not(windows))]
        let cmd = b"echo hello\n";

        session.write(cmd).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));

        let mut buf = [0u8; 4096];
        let n = session.read(&mut buf).unwrap();
        assert!(n > 0);

        session.kill().unwrap();
    }

    #[test]
    fn test_session_resize() {
        let mut session = Session::spawn(test_config()).unwrap();
        assert!(session.resize(40, 120).is_ok());
        session.kill().unwrap();
    }

    #[test]
    fn test_session_restart() {
        let mut session = Session::spawn(test_config()).unwrap();
        let original_id = session.id;
        session.restart().unwrap();
        // ID should stay the same after restart
        assert_eq!(session.id, original_id);
        assert!(session.try_wait().unwrap().is_none());
        session.kill().unwrap();
    }

    #[test]
    fn test_agent_session() {
        let config = SessionConfig {
            agent: "claude".to_string(),
            ..test_config()
        };
        let mut session = Session::spawn(config).unwrap();
        assert_eq!(session.agent_state, AgentState::Idle);
        session.kill().unwrap();
    }

    #[tokio::test]
    async fn test_start_reader() {
        let config = test_config();
        let session = Session::spawn(config).unwrap();
        let (handle, mut rx) = session.start_reader();

        // Should receive some initial output
        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await;

        // Either we got data or the process exited — both are valid
        assert!(msg.is_ok());

        drop(rx);
        let mut session = handle.join().unwrap();
        session.kill().ok();
    }
}
