//! Low-level PTY wrapper around portable-pty.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use termesh_core::error::PtyError;

/// A separated PTY writer handle.
///
/// Allows writing to the PTY from a different thread than the reader.
pub struct PtyWriter {
    writer: Box<dyn Write + Send>,
}

impl PtyWriter {
    /// Write bytes to the PTY (user input → process stdin).
    pub fn write(&mut self, data: &[u8]) -> Result<usize, PtyError> {
        self.writer.write(data).map_err(PtyError::Io)
    }
}

/// A handle for resizing the PTY from any thread.
///
/// Thread-safe via internal Mutex.
#[derive(Clone)]
pub struct PtyResizer {
    master: Arc<std::sync::Mutex<Box<dyn MasterPty + Send>>>,
}

impl PtyResizer {
    /// Resize the PTY terminal.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let master = self.master.lock().map_err(|_| PtyError::SpawnFailed {
            reason: "PTY master lock poisoned".to_string(),
        })?;
        master
            .resize(size)
            .map_err(|e| PtyError::Io(std::io::Error::other(e)))
    }
}

/// Wraps a portable-pty master/child pair.
pub struct Pty {
    master: Arc<std::sync::Mutex<Box<dyn MasterPty + Send>>>,
    child: Box<dyn Child + Send + Sync>,
    reader: Box<dyn Read + Send>,
    writer: Option<Box<dyn Write + Send>>,
}

impl Pty {
    /// Spawn a new PTY process.
    ///
    /// # Arguments
    /// - `command`: The program to execute (e.g., "bash", "zsh", "claude").
    /// - `args`: Arguments to pass to the program.
    /// - `cwd`: Working directory for the process. If `None`, uses current dir.
    /// - `rows`: Initial terminal height.
    /// - `cols`: Initial terminal width.
    pub fn spawn(
        command: &str,
        args: &[String],
        cwd: Option<&Path>,
        rows: u16,
        cols: u16,
    ) -> Result<Self, PtyError> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| PtyError::SpawnFailed {
                reason: format!("failed to open PTY: {e}"),
            })?;

        let mut cmd = CommandBuilder::new(command);
        for arg in args {
            cmd.arg(arg);
        }
        if let Some(dir) = cwd {
            cmd.cwd(dir);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::SpawnFailed {
                reason: format!("failed to spawn command '{command}': {e}"),
            })?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::SpawnFailed {
                reason: format!("failed to clone PTY reader: {e}"),
            })?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::SpawnFailed {
                reason: format!("failed to take PTY writer: {e}"),
            })?;

        Ok(Self {
            master: Arc::new(std::sync::Mutex::new(pair.master)),
            child,
            reader,
            writer: Some(writer),
        })
    }

    /// Create a resizer handle that can be used from any thread.
    pub fn resizer(&self) -> PtyResizer {
        PtyResizer {
            master: Arc::clone(&self.master),
        }
    }

    /// Resize the PTY terminal.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let master = self.master.lock().map_err(|_| PtyError::SpawnFailed {
            reason: "PTY master lock poisoned".to_string(),
        })?;
        master
            .resize(size)
            .map_err(|e| PtyError::Io(std::io::Error::other(e)))
    }

    /// Write bytes to the PTY (user input → process stdin).
    ///
    /// Returns an error if the writer has been taken via `take_writer()`.
    pub fn write_input(&mut self, data: &[u8]) -> Result<usize, PtyError> {
        match &mut self.writer {
            Some(w) => w.write(data).map_err(PtyError::Io),
            None => Err(PtyError::SpawnFailed {
                reason: "PTY writer has been taken".to_string(),
            }),
        }
    }

    /// Take the writer handle, separating it from the PTY.
    ///
    /// After this, `write_input()` will fail. Use the returned `PtyWriter`
    /// to send input from a different thread.
    pub fn take_writer(&mut self) -> Option<PtyWriter> {
        self.writer.take().map(|w| PtyWriter { writer: w })
    }

    /// Read bytes from the PTY (process stdout → display).
    ///
    /// This is a blocking call. Use in a dedicated thread.
    pub fn read_output(&mut self, buf: &mut [u8]) -> Result<usize, PtyError> {
        self.reader.read(buf).map_err(PtyError::Io)
    }

    /// Check if the child process has exited.
    ///
    /// Returns `Some(exit_code)` if exited, `None` if still running.
    pub fn try_wait(&mut self) -> Result<Option<u32>, PtyError> {
        match self.child.try_wait() {
            Ok(Some(status)) => Ok(Some(status.exit_code())),
            Ok(None) => Ok(None),
            Err(e) => Err(PtyError::Io(std::io::Error::other(e))),
        }
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<(), PtyError> {
        self.child
            .kill()
            .map_err(|e| PtyError::Io(std::io::Error::other(e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shell() -> &'static str {
        #[cfg(windows)]
        {
            "cmd.exe"
        }
        #[cfg(not(windows))]
        {
            "sh"
        }
    }

    #[test]
    fn test_spawn_and_kill() {
        let mut pty = Pty::spawn(test_shell(), &[], None, 24, 80).unwrap();
        // Process should be running
        assert!(pty.try_wait().unwrap().is_none());
        // Kill it
        pty.kill().unwrap();
    }

    #[test]
    fn test_spawn_with_invalid_command() {
        let result = Pty::spawn("nonexistent_command_xyz", &[], None, 24, 80);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_and_read() {
        let mut pty = Pty::spawn(test_shell(), &[], None, 24, 80).unwrap();

        // Write a command
        #[cfg(windows)]
        let cmd = b"echo hello\r\n";
        #[cfg(not(windows))]
        let cmd = b"echo hello\n";

        pty.write_input(cmd).unwrap();

        // Read output (give it a moment)
        std::thread::sleep(std::time::Duration::from_millis(500));
        let mut buf = [0u8; 4096];
        let n = pty.read_output(&mut buf).unwrap();
        assert!(n > 0);

        pty.kill().unwrap();
    }

    #[test]
    fn test_resize() {
        let pty = Pty::spawn(test_shell(), &[], None, 24, 80).unwrap();
        assert!(pty.resize(40, 120).is_ok());
    }
}
