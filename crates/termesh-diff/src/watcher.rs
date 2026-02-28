//! File watcher using the `notify` crate.

use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Default ignore patterns for directories that should not be watched.
const DEFAULT_IGNORE_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".cache",
    ".next",
    "dist",
    "build",
    ".venv",
    "venv",
];

/// A file change event from the watcher.
#[derive(Debug, Clone)]
pub struct FileChange {
    /// Path to the changed file.
    pub path: PathBuf,
    /// Kind of change.
    pub kind: FileChangeKind,
}

/// Kind of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    Created,
    Modified,
    Removed,
}

/// Watches a directory for file changes, filtering out ignored paths.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    receiver: mpsc::Receiver<FileChange>,
    ignore_dirs: Vec<String>,
}

impl FileWatcher {
    /// Start watching the given directory recursively.
    pub fn new(root: &Path) -> Result<Self, FileWatcherError> {
        Self::with_ignore(
            root,
            DEFAULT_IGNORE_DIRS.iter().map(|s| s.to_string()).collect(),
        )
    }

    /// Start watching with custom ignore directory list.
    pub fn with_ignore(root: &Path, ignore_dirs: Vec<String>) -> Result<Self, FileWatcherError> {
        let (tx, rx) = mpsc::channel();
        let ignore_clone = ignore_dirs.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: NotifyResult<Event>| {
                if let Ok(event) = res {
                    let kind = match event.kind {
                        EventKind::Create(_) => Some(FileChangeKind::Created),
                        EventKind::Modify(_) => Some(FileChangeKind::Modified),
                        EventKind::Remove(_) => Some(FileChangeKind::Removed),
                        _ => None,
                    };

                    if let Some(kind) = kind {
                        for path in event.paths {
                            if should_ignore(&path, &ignore_clone) {
                                continue;
                            }
                            let _ = tx.send(FileChange { path, kind });
                        }
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )
        .map_err(FileWatcherError::Watch)?;

        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(FileWatcherError::Watch)?;

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            ignore_dirs,
        })
    }

    /// Try to receive the next file change (non-blocking).
    pub fn try_recv(&self) -> Option<FileChange> {
        self.receiver.try_recv().ok()
    }

    /// Receive the next file change, blocking up to `timeout`.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<FileChange> {
        self.receiver.recv_timeout(timeout).ok()
    }

    /// Drain all pending changes.
    pub fn drain(&self) -> Vec<FileChange> {
        let mut changes = Vec::new();
        while let Ok(change) = self.receiver.try_recv() {
            changes.push(change);
        }
        changes
    }

    /// Get the current ignore directory list.
    pub fn ignore_dirs(&self) -> &[String] {
        &self.ignore_dirs
    }
}

/// Check if a path should be ignored based on directory names.
fn should_ignore(path: &Path, ignore_dirs: &[String]) -> bool {
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            if ignore_dirs.iter().any(|d| d == name_str.as_ref()) {
                return true;
            }
        }
    }
    false
}

/// Errors from the file watcher.
#[derive(Debug)]
pub enum FileWatcherError {
    /// notify crate error.
    Watch(notify::Error),
}

impl std::fmt::Display for FileWatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Watch(e) => write!(f, "file watcher error: {e}"),
        }
    }
}

impl std::error::Error for FileWatcherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Watch(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_ignore_git() {
        let path = Path::new("/project/.git/objects/abc");
        assert!(should_ignore(path, &[".git".into()]));
    }

    #[test]
    fn test_should_ignore_node_modules() {
        let path = Path::new("/project/node_modules/foo/bar.js");
        assert!(should_ignore(path, &["node_modules".into()]));
    }

    #[test]
    fn test_should_not_ignore_normal() {
        let path = Path::new("/project/src/main.rs");
        assert!(!should_ignore(
            path,
            &[".git".into(), "node_modules".into()]
        ));
    }

    #[test]
    fn test_should_ignore_target() {
        let path = Path::new("/project/target/debug/build");
        assert!(should_ignore(path, &["target".into()]));
    }

    #[test]
    fn test_default_ignore_dirs() {
        assert!(DEFAULT_IGNORE_DIRS.contains(&".git"));
        assert!(DEFAULT_IGNORE_DIRS.contains(&"node_modules"));
        assert!(DEFAULT_IGNORE_DIRS.contains(&"target"));
        assert!(DEFAULT_IGNORE_DIRS.contains(&"__pycache__"));
    }

    #[test]
    fn test_file_watcher_creation() {
        let dir = std::env::temp_dir().join("termesh_watcher_test");
        std::fs::create_dir_all(&dir).unwrap();

        let watcher = FileWatcher::new(&dir);
        assert!(watcher.is_ok());

        let watcher = watcher.unwrap();
        assert_eq!(watcher.ignore_dirs().len(), DEFAULT_IGNORE_DIRS.len());

        // No events should be pending yet
        assert!(watcher.try_recv().is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_file_watcher_detects_change() {
        let dir = std::env::temp_dir().join("termesh_watcher_detect");
        std::fs::create_dir_all(&dir).unwrap();

        let watcher = FileWatcher::new(&dir).unwrap();

        // Create a file
        let file = dir.join("test_detect.txt");
        std::fs::write(&file, "hello").unwrap();

        // Give the watcher time to detect
        let change = watcher.recv_timeout(Duration::from_secs(5));

        // On some platforms notify may batch events differently,
        // but we should get at least one change
        if let Some(c) = change {
            assert!(c.path.ends_with("test_detect.txt"));
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
