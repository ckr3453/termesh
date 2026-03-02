//! Git-based file change tracking using `git status` and `git diff`.
//!
//! Tracks changes scoped to a session's working directory, filtering out
//! files that were already dirty when the session started (baseline).

use crate::history::ChangedFile;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Minimum interval between git polls (seconds).
const POLL_INTERVAL_SECS: u64 = 2;

/// Maximum file size to read for diff display (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Tracks file changes in a git repository using `git status` and `git diff`.
///
/// Only shows changes within `scope_prefix` that were NOT present at session start.
pub struct GitChangeTracker {
    /// Root of the git repository.
    git_root: PathBuf,
    /// Relative path prefix to scope file tracking (e.g. "backend/").
    /// Empty string means the entire repo.
    scope_prefix: String,
    /// Files that were already dirty when the session started.
    /// Stored as git-relative paths (e.g. "src/main.rs").
    baseline: HashSet<String>,
    /// Cached list of changed files from the last poll.
    cached_files: Vec<ChangedFile>,
    /// Last time we polled git.
    last_poll: Instant,
}

impl GitChangeTracker {
    /// Create a new tracker for the git repository containing `cwd`.
    ///
    /// Changes are scoped to `cwd` (not the entire git root).
    /// Files already dirty at creation time are recorded as baseline and excluded.
    ///
    /// Returns `None` if `cwd` is not inside a git repository.
    pub fn new(cwd: &Path) -> Option<Self> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(cwd)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let root = String::from_utf8(output.stdout).ok()?;
        let root = PathBuf::from(root.trim());

        if !root.is_dir() {
            return None;
        }

        // Canonicalize to resolve symlinks (e.g., /var → /private/var on macOS)
        let root = root.canonicalize().unwrap_or(root);

        // Compute scope prefix: cwd relative to git root
        let scope_prefix = cwd
            .canonicalize()
            .ok()
            .and_then(|abs_cwd| {
                root.canonicalize()
                    .ok()
                    .and_then(|abs_root| abs_cwd.strip_prefix(&abs_root).ok().map(|p| p.to_owned()))
            })
            .map(|p| {
                let s = p.to_string_lossy().replace('\\', "/");
                if s.is_empty() {
                    s
                } else {
                    format!("{s}/")
                }
            })
            .unwrap_or_default();

        // Snapshot current dirty files as baseline
        let baseline = git_status_paths(&root)
            .into_iter()
            .filter(|p| scope_prefix.is_empty() || p.starts_with(&scope_prefix))
            .collect::<HashSet<_>>();

        log::info!(
            "git change tracker started: {} (scope={}, baseline={})",
            root.display(),
            if scope_prefix.is_empty() {
                "/"
            } else {
                &scope_prefix
            },
            baseline.len()
        );

        Some(Self {
            git_root: root,
            scope_prefix,
            baseline,
            cached_files: Vec::new(),
            last_poll: Instant::now() - std::time::Duration::from_secs(POLL_INTERVAL_SECS + 1),
        })
    }

    /// Poll git for changes. Returns `true` if the file list changed.
    ///
    /// Skips polling if less than `POLL_INTERVAL_SECS` have elapsed.
    pub fn poll(&mut self) -> bool {
        if self.last_poll.elapsed().as_secs() < POLL_INTERVAL_SECS {
            return false;
        }
        self.last_poll = Instant::now();

        let new_files = self.query_changed_files();
        if new_files == self.cached_files {
            return false;
        }
        self.cached_files = new_files;
        true
    }

    /// Get the cached list of changed files.
    pub fn changed_files(&self) -> &[ChangedFile] {
        &self.cached_files
    }

    /// Get the old and new content for a file, suitable for diff generation.
    ///
    /// - Modified files: old = content at session start, new = working tree
    /// - Added/Untracked files: old = `""`, new = working tree
    pub fn file_diff(&self, path: &Path) -> Option<(String, String)> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let rel = self.relative_path(&canonical)?;
        let status = self
            .cached_files
            .iter()
            .find(|f| f.path == canonical)?
            .status;

        let new_content = read_file_if_small(path)?;

        let old_content = match status {
            'M' => {
                // For baseline files that reappeared (content changed further),
                // show diff against HEAD. For new modifications, also against HEAD.
                let output = Command::new("git")
                    .args(["show", &format!("HEAD:{rel}")])
                    .current_dir(&self.git_root)
                    .output()
                    .ok()?;
                if output.status.success() {
                    String::from_utf8(output.stdout).unwrap_or_default()
                } else {
                    String::new()
                }
            }
            _ => String::new(), // 'A' or '?' — new file
        };

        Some((old_content, new_content))
    }

    /// Get the git repository root.
    pub fn git_root(&self) -> &Path {
        &self.git_root
    }

    /// Query git for the current list of changed files, filtered by scope and baseline.
    fn query_changed_files(&self) -> Vec<ChangedFile> {
        let status_files = self.git_status();
        let numstat = self.git_diff_numstat();

        let mut files: Vec<ChangedFile> = status_files
            .into_iter()
            .filter(|(_, path)| {
                // Scope: only files under session cwd
                (self.scope_prefix.is_empty() || path.starts_with(&self.scope_prefix))
                    // Baseline: exclude files that were already dirty at session start
                    && !self.baseline.contains(path)
            })
            .map(|(status, path)| {
                let (insertions, deletions) = if status == '?' || status == 'A' {
                    let full_path = self.git_root.join(&path);
                    let lines = read_file_if_small(&full_path)
                        .map(|c| c.lines().count())
                        .unwrap_or(0);
                    (lines, 0)
                } else {
                    numstat
                        .iter()
                        .find(|(p, _, _)| p == &path)
                        .map(|(_, ins, del)| (*ins, *del))
                        .unwrap_or((0, 0))
                };

                let display_status = if status == '?' { 'A' } else { status };
                ChangedFile {
                    path: self.git_root.join(&path),
                    status: display_status,
                    insertions,
                    deletions,
                }
            })
            .collect();

        files.sort_by(|a, b| a.path.cmp(&b.path));
        files
    }

    /// Run `git status --porcelain` and parse results.
    fn git_status(&self) -> Vec<(char, String)> {
        parse_git_status(&self.git_root)
    }

    /// Run `git diff --numstat` and parse insertions/deletions.
    fn git_diff_numstat(&self) -> Vec<(String, usize, usize)> {
        let output = Command::new("git")
            .args(["diff", "--numstat"])
            .current_dir(&self.git_root)
            .output();

        let output = match output {
            Ok(o) if o.status.success() => o,
            _ => return Vec::new(),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() != 3 {
                continue;
            }
            let ins = parts[0].parse::<usize>().unwrap_or(0);
            let del = parts[1].parse::<usize>().unwrap_or(0);
            let path = parts[2].to_string();
            results.push((path, ins, del));
        }

        results
    }

    /// Convert an absolute path to a git-relative path string.
    fn relative_path(&self, path: &Path) -> Option<String> {
        path.strip_prefix(&self.git_root)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
    }
}

/// Run `git status --porcelain` and return (status_char, relative_path) pairs.
fn parse_git_status(git_root: &Path) -> Vec<(char, String)> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(git_root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let xy = &line[..2];
        let path = line[3..].to_string();

        let status = match xy {
            "??" => '?',
            _ => {
                let y = xy.as_bytes()[1];
                let x = xy.as_bytes()[0];
                if y == b'M' || y == b'D' {
                    y as char
                } else if x == b'M' || x == b'A' || x == b'D' {
                    x as char
                } else {
                    continue;
                }
            }
        };

        results.push((status, path));
    }

    results
}

/// Collect just the paths from `git status --porcelain` (for baseline snapshot).
fn git_status_paths(git_root: &Path) -> Vec<String> {
    parse_git_status(git_root)
        .into_iter()
        .map(|(_, path)| path)
        .collect()
}

/// Read a file if it's small enough and appears to be text.
fn read_file_if_small(path: &Path) -> Option<String> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut limited = file.take(MAX_FILE_SIZE + 1);
    let mut buf = String::new();
    if limited.read_to_string(&mut buf).is_err() {
        return None;
    }
    if buf.len() as u64 > MAX_FILE_SIZE {
        return None;
    }
    Some(buf)
}

impl PartialEq for ChangedFile {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.status == other.status
            && self.insertions == other.insertions
            && self.deletions == other.deletions
    }
}

impl Eq for ChangedFile {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Create a temporary git repo with an initial commit.
    fn setup_git_repo() -> Option<tempfile::TempDir> {
        let dir = tempfile::tempdir().ok()?;
        let init = Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .ok()?;
        if !init.status.success() {
            return None;
        }
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .ok()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir.path())
            .output()
            .ok()?;
        // Initial commit so HEAD exists
        let readme = dir.path().join("README.md");
        std::fs::write(&readme, "init\n").ok()?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .ok()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir.path())
            .output()
            .ok()?;
        Some(dir)
    }

    #[test]
    fn test_new_in_git_repo() {
        let dir = match setup_git_repo() {
            Some(d) => d,
            None => return,
        };
        let tracker = GitChangeTracker::new(dir.path());
        assert!(tracker.is_some());
        assert!(tracker.unwrap().baseline.is_empty());
    }

    #[test]
    fn test_new_outside_git_repo() {
        let dir = tempfile::tempdir().unwrap();
        let tracker = GitChangeTracker::new(dir.path());
        assert!(tracker.is_none());
    }

    #[test]
    fn test_baseline_excludes_pre_existing_changes() {
        let dir = match setup_git_repo() {
            Some(d) => d,
            None => return,
        };

        // Create a dirty file BEFORE tracker starts
        let pre_existing = dir.path().join("old_change.txt");
        std::fs::write(&pre_existing, "pre-existing\n").unwrap();

        // Start tracker — old_change.txt should be in baseline
        let mut tracker = GitChangeTracker::new(dir.path()).unwrap();
        assert!(tracker.baseline.contains("old_change.txt"));

        // Create a NEW file after tracker started
        let new_file = dir.path().join("new_file.txt");
        std::fs::write(&new_file, "new\n").unwrap();

        assert!(tracker.poll());
        let files = tracker.changed_files();
        // Only new_file.txt should appear, not old_change.txt
        assert_eq!(files.len(), 1);
        assert!(files[0].path.ends_with("new_file.txt"));
        assert_eq!(files[0].status, 'A');
    }

    #[test]
    fn test_scope_limits_to_subdirectory() {
        let dir = match setup_git_repo() {
            Some(d) => d,
            None => return,
        };

        // Create subdirectories
        let backend = dir.path().join("backend");
        let frontend = dir.path().join("frontend");
        std::fs::create_dir_all(&backend).unwrap();
        std::fs::create_dir_all(&frontend).unwrap();

        // Start tracker scoped to backend/
        let mut tracker = GitChangeTracker::new(&backend).unwrap();

        // Create files in both directories
        std::fs::write(backend.join("api.rs"), "fn main() {}\n").unwrap();
        std::fs::write(frontend.join("app.js"), "console.log('hi')\n").unwrap();

        assert!(tracker.poll());
        let files = tracker.changed_files();
        // Only backend/api.rs should appear
        assert_eq!(files.len(), 1, "files: {files:?}");
        let name = files[0].path.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, "api.rs");
    }

    #[test]
    fn test_modified_file_detected() {
        let dir = match setup_git_repo() {
            Some(d) => d,
            None => return,
        };

        // Start tracker on clean state
        let mut tracker = GitChangeTracker::new(dir.path()).unwrap();

        // Modify a committed file
        std::fs::write(dir.path().join("README.md"), "modified\n").unwrap();

        assert!(tracker.poll());
        let files = tracker.changed_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, 'M');
    }

    #[test]
    fn test_file_diff_modified() {
        let dir = match setup_git_repo() {
            Some(d) => d,
            None => return,
        };

        let mut tracker = GitChangeTracker::new(dir.path()).unwrap();
        std::fs::write(dir.path().join("README.md"), "modified\n").unwrap();
        tracker.poll();

        let file = dir.path().join("README.md");
        let (old, new) = tracker.file_diff(&file).unwrap();
        assert_eq!(old, "init\n");
        assert_eq!(new, "modified\n");
    }

    #[test]
    fn test_file_diff_untracked() {
        let dir = match setup_git_repo() {
            Some(d) => d,
            None => return,
        };

        let mut tracker = GitChangeTracker::new(dir.path()).unwrap();
        let file = dir.path().join("new.txt");
        std::fs::write(&file, "new content\n").unwrap();
        tracker.poll();

        let (old, new) = tracker.file_diff(&file).unwrap();
        assert!(old.is_empty());
        assert_eq!(new, "new content\n");
    }

    #[test]
    fn test_no_change_returns_false() {
        let dir = match setup_git_repo() {
            Some(d) => d,
            None => return,
        };

        let mut tracker = GitChangeTracker::new(dir.path()).unwrap();
        tracker.poll();
        tracker.last_poll = Instant::now() - std::time::Duration::from_secs(POLL_INTERVAL_SECS + 1);
        assert!(!tracker.poll());
    }
}
