//! File change watching and diff generation for termesh.
//!
//! Monitors workspace files for changes and generates unified diffs
//! for display in the side panel.

pub mod diff_generator;
pub mod git_changes;
pub mod history;
pub mod watcher;
