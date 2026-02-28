//! Diff generation using the `similar` crate.

use similar::{ChangeTag, TextDiff};

/// A single line change in a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub tag: DiffTag,
    pub content: String,
}

/// Type of diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffTag {
    /// Unchanged context line.
    Equal,
    /// Line was removed (old only).
    Delete,
    /// Line was added (new only).
    Insert,
}

/// Result of diffing two texts.
#[derive(Debug, Clone)]
pub struct DiffResult {
    /// Individual line changes.
    pub lines: Vec<DiffLine>,
    /// Number of insertions.
    pub insertions: usize,
    /// Number of deletions.
    pub deletions: usize,
}

impl DiffResult {
    /// Whether the two texts are identical.
    pub fn is_empty(&self) -> bool {
        self.insertions == 0 && self.deletions == 0
    }
}

/// Generate a line-level diff between old and new text.
pub fn diff_texts(old: &str, new: &str) -> DiffResult {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    let mut insertions = 0;
    let mut deletions = 0;

    for change in diff.iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Equal => DiffTag::Equal,
            ChangeTag::Delete => {
                deletions += 1;
                DiffTag::Delete
            }
            ChangeTag::Insert => {
                insertions += 1;
                DiffTag::Insert
            }
        };
        lines.push(DiffLine {
            tag,
            content: change.to_string_lossy().to_string(),
        });
    }

    DiffResult {
        lines,
        insertions,
        deletions,
    }
}

/// Generate a unified diff string (standard format).
pub fn unified_diff(old: &str, new: &str, old_label: &str, new_label: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    diff.unified_diff()
        .header(old_label, new_label)
        .context_radius(3)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_identical() {
        let result = diff_texts("hello\nworld\n", "hello\nworld\n");
        assert!(result.is_empty());
        assert_eq!(result.insertions, 0);
        assert_eq!(result.deletions, 0);
    }

    #[test]
    fn test_diff_insertion() {
        let result = diff_texts("line1\nline3\n", "line1\nline2\nline3\n");
        assert_eq!(result.insertions, 1);
        assert_eq!(result.deletions, 0);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_diff_deletion() {
        let result = diff_texts("line1\nline2\nline3\n", "line1\nline3\n");
        assert_eq!(result.insertions, 0);
        assert_eq!(result.deletions, 1);
    }

    #[test]
    fn test_diff_modification() {
        let result = diff_texts("old line\n", "new line\n");
        assert_eq!(result.insertions, 1);
        assert_eq!(result.deletions, 1);
    }

    #[test]
    fn test_diff_empty_to_content() {
        let result = diff_texts("", "new content\n");
        assert_eq!(result.insertions, 1);
        assert_eq!(result.deletions, 0);
    }

    #[test]
    fn test_diff_content_to_empty() {
        let result = diff_texts("old content\n", "");
        assert_eq!(result.insertions, 0);
        assert_eq!(result.deletions, 1);
    }

    #[test]
    fn test_unified_diff_format() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";
        let output = unified_diff(old, new, "a/file.rs", "b/file.rs");
        assert!(output.contains("--- a/file.rs"));
        assert!(output.contains("+++ b/file.rs"));
        assert!(output.contains("-line2"));
        assert!(output.contains("+modified"));
    }

    #[test]
    fn test_unified_diff_no_changes() {
        let text = "same\n";
        let output = unified_diff(text, text, "a/f", "b/f");
        assert!(output.is_empty());
    }
}
