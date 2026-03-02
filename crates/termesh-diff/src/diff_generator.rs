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

/// Diff display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffMode {
    /// Standard unified diff (one column, +/- prefixes).
    Unified,
    /// Side-by-side: left = old, right = new.
    SideBySide,
}

/// A single row in side-by-side diff view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideBySideLine {
    /// Left (old) content. `None` for inserted lines.
    pub left: Option<String>,
    /// Right (new) content. `None` for deleted lines.
    pub right: Option<String>,
    /// Tag describing the change.
    pub tag: DiffTag,
}

/// Generate side-by-side line pairs from old and new text.
///
/// Equal lines appear on both sides. Deletions appear on the left only,
/// insertions on the right only. Adjacent delete+insert pairs are aligned
/// on the same row.
pub fn side_by_side_diff(old: &str, new: &str) -> Vec<SideBySideLine> {
    let diff = TextDiff::from_lines(old, new);
    let changes: Vec<_> = diff.iter_all_changes().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < changes.len() {
        match changes[i].tag() {
            ChangeTag::Equal => {
                let content = changes[i].to_string_lossy().to_string();
                result.push(SideBySideLine {
                    left: Some(content.clone()),
                    right: Some(content),
                    tag: DiffTag::Equal,
                });
                i += 1;
            }
            ChangeTag::Delete => {
                // Collect consecutive deletes
                let mut deletes = Vec::new();
                while i < changes.len() && changes[i].tag() == ChangeTag::Delete {
                    deletes.push(changes[i].to_string_lossy().to_string());
                    i += 1;
                }
                // Collect consecutive inserts that follow
                let mut inserts = Vec::new();
                while i < changes.len() && changes[i].tag() == ChangeTag::Insert {
                    inserts.push(changes[i].to_string_lossy().to_string());
                    i += 1;
                }
                // Pair them up
                let max_len = deletes.len().max(inserts.len());
                for j in 0..max_len {
                    let left = deletes.get(j).cloned();
                    let right = inserts.get(j).cloned();
                    let tag = match (&left, &right) {
                        (Some(_), Some(_)) => DiffTag::Delete, // modification
                        (Some(_), None) => DiffTag::Delete,
                        (None, Some(_)) => DiffTag::Insert,
                        (None, None) => unreachable!(),
                    };
                    result.push(SideBySideLine { left, right, tag });
                }
            }
            ChangeTag::Insert => {
                // Standalone insert (not preceded by delete)
                result.push(SideBySideLine {
                    left: None,
                    right: Some(changes[i].to_string_lossy().to_string()),
                    tag: DiffTag::Insert,
                });
                i += 1;
            }
        }
    }

    result
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

    // ── Side-by-side tests ────────────────────────────────────────────────

    #[test]
    fn test_sbs_identical() {
        let lines = side_by_side_diff("a\nb\n", "a\nb\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].tag, DiffTag::Equal);
        assert_eq!(lines[0].left.as_deref(), Some("a\n"));
        assert_eq!(lines[0].right.as_deref(), Some("a\n"));
    }

    #[test]
    fn test_sbs_modification() {
        let lines = side_by_side_diff("old\n", "new\n");
        assert_eq!(lines.len(), 1);
        // delete+insert paired on same row
        assert_eq!(lines[0].left.as_deref(), Some("old\n"));
        assert_eq!(lines[0].right.as_deref(), Some("new\n"));
    }

    #[test]
    fn test_sbs_insertion() {
        let lines = side_by_side_diff("a\nc\n", "a\nb\nc\n");
        // a=equal, b=insert, c=equal
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].tag, DiffTag::Equal);
        assert_eq!(lines[1].tag, DiffTag::Insert);
        assert!(lines[1].left.is_none());
        assert_eq!(lines[1].right.as_deref(), Some("b\n"));
        assert_eq!(lines[2].tag, DiffTag::Equal);
    }

    #[test]
    fn test_sbs_deletion() {
        let lines = side_by_side_diff("a\nb\nc\n", "a\nc\n");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].tag, DiffTag::Equal);
        assert_eq!(lines[1].tag, DiffTag::Delete);
        assert_eq!(lines[1].left.as_deref(), Some("b\n"));
        assert!(lines[1].right.is_none());
        assert_eq!(lines[2].tag, DiffTag::Equal);
    }

    #[test]
    fn test_sbs_multi_line_change() {
        let lines = side_by_side_diff("a\nb\n", "c\nd\ne\n");
        // a→c, b→d paired, e standalone insert
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].left.as_deref(), Some("a\n"));
        assert_eq!(lines[0].right.as_deref(), Some("c\n"));
        assert_eq!(lines[1].left.as_deref(), Some("b\n"));
        assert_eq!(lines[1].right.as_deref(), Some("d\n"));
        assert!(lines[2].left.is_none());
        assert_eq!(lines[2].right.as_deref(), Some("e\n"));
    }
}
