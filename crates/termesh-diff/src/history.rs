//! Change history: tracks recent file modifications with content snapshots.

use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Maximum number of history entries to keep.
const MAX_HISTORY_SIZE: usize = 100;

/// Maximum file size to cache (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// A single file change record.
#[derive(Debug, Clone)]
pub struct ChangeRecord {
    /// Path to the changed file.
    pub path: PathBuf,
    /// Content before the change (if available).
    pub old_content: Option<String>,
    /// Content after the change.
    pub new_content: String,
    /// When the change was detected.
    pub timestamp: SystemTime,
}

/// Maximum number of cached files.
const MAX_CACHE_FILES: usize = 500;

/// Summary of a changed file for the file list UI.
#[derive(Debug, Clone)]
pub struct ChangedFile {
    /// File path.
    pub path: PathBuf,
    /// Status character: 'M' modified, 'A' added (no initial snapshot).
    pub status: char,
    /// Number of inserted lines.
    pub insertions: usize,
    /// Number of deleted lines.
    pub deletions: usize,
}

/// Manages file change history and content caching.
#[derive(Debug)]
pub struct ChangeHistory {
    /// Recent change records (newest last).
    records: VecDeque<ChangeRecord>,
    /// Cached file contents for diff generation (latest version).
    cache: HashMap<PathBuf, String>,
    /// Initial file contents at first observation (for cumulative diff).
    initial: HashMap<PathBuf, String>,
    /// Maximum records to keep.
    max_size: usize,
}

impl ChangeHistory {
    /// Create a new change history with default capacity.
    pub fn new() -> Self {
        Self {
            records: VecDeque::new(),
            cache: HashMap::new(),
            initial: HashMap::new(),
            max_size: MAX_HISTORY_SIZE,
        }
    }

    /// Create a new change history with custom capacity.
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            records: VecDeque::new(),
            cache: HashMap::new(),
            initial: HashMap::new(),
            max_size: max_size.max(1),
        }
    }

    /// Snapshot a file's current content for future diff comparison.
    ///
    /// Returns `false` if the file is too large or unreadable.
    pub fn snapshot_file(&mut self, path: &Path) -> bool {
        match read_file_if_small(path) {
            Some(content) => {
                if !self.initial.contains_key(path) {
                    self.initial.insert(path.to_path_buf(), content.clone());
                }
                self.cache.insert(path.to_path_buf(), content);
                true
            }
            None => false,
        }
    }

    /// Record a file change. Compares against cached content.
    ///
    /// Returns `true` if the change was recorded (file is text and not too large).
    pub fn record_change(&mut self, path: &Path) -> bool {
        let new_content = match read_file_if_small(path) {
            Some(c) => c,
            None => return false,
        };

        // Compare before cloning to avoid unnecessary allocation
        if self.cache.get(path).map(|c| c.as_str()) == Some(new_content.as_str()) {
            return false;
        }

        // Save initial content on first observation (for cumulative diff)
        if !self.initial.contains_key(path) {
            if let Some(cached) = self.cache.get(path) {
                self.initial.insert(path.to_path_buf(), cached.clone());
            }
            // If no cache entry exists, this is a newly created file — no initial snapshot
        }

        // Evict oldest if cache is full and this is a new key
        if !self.cache.contains_key(path) && self.cache.len() >= MAX_CACHE_FILES {
            if let Some(oldest_key) = self.cache.keys().next().cloned() {
                self.cache.remove(&oldest_key);
            }
        }

        // insert() returns the previous value — avoids cloning old_content
        let old_content = self.cache.insert(path.to_path_buf(), new_content.clone());

        let record = ChangeRecord {
            path: path.to_path_buf(),
            old_content,
            new_content,
            timestamp: SystemTime::now(),
        };

        // Add record, evict oldest if at capacity
        if self.records.len() >= self.max_size {
            self.records.pop_front();
        }
        self.records.push_back(record);

        true
    }

    /// Get all change records (oldest first).
    pub fn records(&self) -> &VecDeque<ChangeRecord> {
        &self.records
    }

    /// Get the most recent change records (up to `count`).
    pub fn recent(&self, count: usize) -> Vec<&ChangeRecord> {
        self.records.iter().rev().take(count).collect()
    }

    /// Get the last change record for a specific file.
    pub fn last_change_for(&self, path: &Path) -> Option<&ChangeRecord> {
        self.records.iter().rev().find(|r| r.path == path)
    }

    /// Number of change records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Clear all records and cached content.
    pub fn clear(&mut self) {
        self.records.clear();
        self.cache.clear();
        self.initial.clear();
    }

    /// Number of cached file snapshots.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Get a list of all files that have changed since their initial observation.
    ///
    /// Compares initial snapshots against the latest cached content.
    /// Files without an initial snapshot are marked as 'A' (added).
    pub fn changed_files(&self) -> Vec<ChangedFile> {
        use crate::diff_generator;

        let mut files = Vec::new();
        for (path, current) in &self.cache {
            match self.initial.get(path) {
                Some(initial) if initial == current => {
                    // File reverted to initial state — not changed
                }
                Some(initial) => {
                    let diff = diff_generator::diff_texts(initial, current);
                    files.push(ChangedFile {
                        path: path.clone(),
                        status: 'M',
                        insertions: diff.insertions,
                        deletions: diff.deletions,
                    });
                }
                None => {
                    // No initial snapshot — file was created after watching started
                    let lines = current.lines().count();
                    files.push(ChangedFile {
                        path: path.clone(),
                        status: 'A',
                        insertions: lines,
                        deletions: 0,
                    });
                }
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        files
    }

    /// Get the initial content for a file (at first observation).
    pub fn initial_content(&self, path: &Path) -> Option<&str> {
        self.initial.get(path).map(|s| s.as_str())
    }

    /// Get the current (latest) cached content for a file.
    pub fn current_content(&self, path: &Path) -> Option<&str> {
        self.cache.get(path).map(|s| s.as_str())
    }

    /// Get the cumulative diff for a specific file (initial vs current).
    pub fn diff_for_file(&self, path: &Path) -> Option<crate::diff_generator::DiffResult> {
        use crate::diff_generator;

        let current = self.cache.get(path)?;
        let initial = self.initial.get(path).map(|s| s.as_str()).unwrap_or("");
        let result = diff_generator::diff_texts(initial, current);
        if result.is_empty() && !initial.is_empty() {
            return None; // File unchanged
        }
        Some(result)
    }
}

impl Default for ChangeHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Read a file if it's small enough and appears to be text.
///
/// Uses `Read::take` to avoid TOCTOU races between size check and read.
fn read_file_if_small(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut limited = file.take(MAX_FILE_SIZE + 1);
    let mut buf = String::new();
    if limited.read_to_string(&mut buf).is_err() {
        // Not valid UTF-8 or I/O error — skip (likely binary)
        return None;
    }
    if buf.len() as u64 > MAX_FILE_SIZE {
        log::debug!(
            "Skipping large file: {} (>{MAX_FILE_SIZE} bytes)",
            path.display()
        );
        return None;
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("termesh_test_history");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_snapshot_and_record() {
        let path = temp_file("snap_test.txt", "original content");
        let mut history = ChangeHistory::new();

        assert!(history.snapshot_file(&path));
        assert_eq!(history.cache_size(), 1);

        // Modify the file
        std::fs::write(&path, "modified content").unwrap();

        assert!(history.record_change(&path));
        assert_eq!(history.len(), 1);

        let record = history.records().back().unwrap();
        assert_eq!(record.old_content.as_deref(), Some("original content"));
        assert_eq!(record.new_content, "modified content");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_record_without_snapshot() {
        let path = temp_file("no_snap.txt", "some content");
        let mut history = ChangeHistory::new();

        // No snapshot taken, old_content should be None
        assert!(history.record_change(&path));
        let record = history.records().back().unwrap();
        assert!(record.old_content.is_none());
        assert_eq!(record.new_content, "some content");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_skip_identical_content() {
        let path = temp_file("identical.txt", "same content");
        let mut history = ChangeHistory::new();

        history.snapshot_file(&path);
        // File unchanged, should not record
        assert!(!history.record_change(&path));
        assert!(history.is_empty());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_eviction() {
        let mut history = ChangeHistory::with_capacity(3);
        let paths: Vec<_> = (0..5)
            .map(|i| temp_file(&format!("evict_{i}.txt"), &format!("content {i}")))
            .collect();

        for path in &paths {
            history.record_change(path);
        }

        assert_eq!(history.len(), 3);
        // Oldest records should be evicted
        let records: Vec<_> = history.records().iter().collect();
        assert!(records[0].path.ends_with("evict_2.txt"));

        for path in &paths {
            std::fs::remove_file(path).ok();
        }
    }

    #[test]
    fn test_recent() {
        let mut history = ChangeHistory::new();
        let paths: Vec<_> = (0..5)
            .map(|i| temp_file(&format!("recent_{i}.txt"), &format!("content {i}")))
            .collect();

        for path in &paths {
            history.record_change(path);
        }

        let recent = history.recent(2);
        assert_eq!(recent.len(), 2);
        assert!(recent[0].path.ends_with("recent_4.txt"));
        assert!(recent[1].path.ends_with("recent_3.txt"));

        for path in &paths {
            std::fs::remove_file(path).ok();
        }
    }

    #[test]
    fn test_last_change_for() {
        let path = temp_file("last_change.txt", "v1");
        let mut history = ChangeHistory::new();

        history.record_change(&path);
        std::fs::write(&path, "v2").unwrap();
        history.record_change(&path);

        let last = history.last_change_for(&path).unwrap();
        assert_eq!(last.new_content, "v2");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_clear() {
        let path = temp_file("clear_test.txt", "content");
        let mut history = ChangeHistory::new();

        history.snapshot_file(&path);
        std::fs::write(&path, "new").unwrap();
        history.record_change(&path);

        assert!(!history.is_empty());
        assert!(history.cache_size() > 0);

        history.clear();
        assert!(history.is_empty());
        assert_eq!(history.cache_size(), 0);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_nonexistent_file() {
        let mut history = ChangeHistory::new();
        let path = PathBuf::from("/nonexistent/file.txt");
        assert!(!history.snapshot_file(&path));
        assert!(!history.record_change(&path));
    }

    #[test]
    fn test_changed_files_modified() {
        let path = temp_file("cf_mod.txt", "original\n");
        let mut history = ChangeHistory::new();

        history.snapshot_file(&path);
        std::fs::write(&path, "modified\n").unwrap();
        history.record_change(&path);

        let files = history.changed_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, 'M');
        assert_eq!(files[0].insertions, 1);
        assert_eq!(files[0].deletions, 1);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_changed_files_added() {
        let path = temp_file("cf_add.txt", "new content\nline2\n");
        let mut history = ChangeHistory::new();

        // No snapshot — file treated as added
        history.record_change(&path);

        let files = history.changed_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, 'A');
        assert_eq!(files[0].insertions, 2);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_changed_files_reverted() {
        let path = temp_file("cf_revert.txt", "original\n");
        let mut history = ChangeHistory::new();

        history.snapshot_file(&path);
        std::fs::write(&path, "modified\n").unwrap();
        history.record_change(&path);

        // Revert to original
        std::fs::write(&path, "original\n").unwrap();
        history.record_change(&path);

        let files = history.changed_files();
        assert!(files.is_empty(), "reverted file should not appear");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_diff_for_file() {
        let path = temp_file("df_test.txt", "line1\nline2\n");
        let mut history = ChangeHistory::new();

        history.snapshot_file(&path);
        std::fs::write(&path, "line1\nchanged\nline3\n").unwrap();
        history.record_change(&path);

        let diff = history.diff_for_file(&path).unwrap();
        assert!(diff.insertions > 0);
        assert!(diff.deletions > 0);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_diff_for_file_added() {
        let path = temp_file("df_add.txt", "new\n");
        let mut history = ChangeHistory::new();

        // No snapshot — diff against empty
        history.record_change(&path);

        let diff = history.diff_for_file(&path).unwrap();
        assert_eq!(diff.insertions, 1);
        assert_eq!(diff.deletions, 0);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_clear_resets_initial() {
        let path = temp_file("clear_init.txt", "content\n");
        let mut history = ChangeHistory::new();

        history.snapshot_file(&path);
        std::fs::write(&path, "new\n").unwrap();
        history.record_change(&path);

        history.clear();
        assert!(history.changed_files().is_empty());

        std::fs::remove_file(&path).ok();
    }
}
