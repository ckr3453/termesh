//! Claude Code adapter: infers agent state from Claude Code terminal output.
//!
//! NOTE: This adapter is stateless (per-line analysis). False positives are
//! possible when agent-spawned command output contains matching substrings.
//! Phase 2 will introduce a stateful adapter that tracks the current state
//! to suppress false transitions during command execution.

use crate::adapter::AgentAdapter;
use regex::Regex;
use termesh_core::types::AgentState;

/// Pattern set for detecting Claude Code state transitions.
///
/// Patterns are anchored or use word boundaries to reduce false positives
/// from normal terminal output.
struct StatePatterns {
    thinking: Regex,
    writing_code: Regex,
    running_command: Regex,
    waiting_for_input: Regex,
    error: Regex,
    success: Regex,
}

impl StatePatterns {
    fn compile() -> Self {
        Self {
            // Anchored to line start to avoid matching mid-line occurrences.
            thinking: Regex::new(r"(?i)^(⏳|>\s*(thinking|analyzing|reading|searching))")
                .expect("invalid thinking regex"),
            writing_code: Regex::new(
                r"(?i)^(writing to |creating |updating |edit(ing|ed) |wrote )",
            )
            .expect("invalid writing_code regex"),
            // "Running:" and "Executing:" are Claude Code prefixes.
            // "$" prompt only at line start.
            running_command: Regex::new(r"(?i)^(running: |executing: |\$\s+\S)")
                .expect("invalid running_command regex"),
            waiting_for_input: Regex::new(
                r"(?i)(would you like|do you want|\by/n\b|\[Y/n\]|\[y/N\])",
            )
            .expect("invalid waiting_for_input regex"),
            // Anchored: "Error:" or "Failed:" at line start, or marker symbols.
            error: Regex::new(r"(^(?i)(error:|failed:)|✗)").expect("invalid error regex"),
            // Only match Unicode checkmarks, or "successfully" as a word.
            // Avoid bare "done" and "complete" which are too common.
            success: Regex::new(r"(✓|✅|\bsuccessfully\b)").expect("invalid success regex"),
        }
    }
}

/// Adapter for Claude Code AI agent.
pub struct ClaudeCodeAdapter {
    patterns: StatePatterns,
}

impl ClaudeCodeAdapter {
    /// Create a new Claude Code adapter with default patterns.
    pub fn new() -> Self {
        Self {
            patterns: StatePatterns::compile(),
        }
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentAdapter for ClaudeCodeAdapter {
    fn name(&self) -> &str {
        "Claude Code"
    }

    fn analyze_line(&self, line: &str) -> Option<AgentState> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Order matters: more specific patterns first.
        // Error/success are terminal states and should take priority.
        if self.patterns.error.is_match(trimmed) {
            return Some(AgentState::Error);
        }
        if self.patterns.success.is_match(trimmed) {
            return Some(AgentState::Success);
        }
        if self.patterns.waiting_for_input.is_match(trimmed) {
            return Some(AgentState::WaitingForInput);
        }
        if self.patterns.running_command.is_match(trimmed) {
            return Some(AgentState::RunningCommand);
        }
        if self.patterns.writing_code.is_match(trimmed) {
            return Some(AgentState::WritingCode);
        }
        if self.patterns.thinking.is_match(trimmed) {
            return Some(AgentState::Thinking);
        }
        None
    }

    fn is_agent_command(&self, command: &str) -> bool {
        let first_token = command.split_whitespace().next().unwrap_or("");
        first_token == "claude"
            || first_token == "claude-code"
            || first_token.ends_with("/claude")
            || first_token.ends_with("/claude-code")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> ClaudeCodeAdapter {
        ClaudeCodeAdapter::new()
    }

    #[test]
    fn test_name() {
        assert_eq!(adapter().name(), "Claude Code");
    }

    #[test]
    fn test_thinking_patterns() {
        let a = adapter();
        assert_eq!(a.analyze_line("⏳ Thinking..."), Some(AgentState::Thinking));
        assert_eq!(
            a.analyze_line("> Analyzing the codebase"),
            Some(AgentState::Thinking)
        );
        assert_eq!(
            a.analyze_line("> Reading src/main.rs"),
            Some(AgentState::Thinking)
        );
        assert_eq!(
            a.analyze_line("> Searching for files"),
            Some(AgentState::Thinking)
        );
    }

    #[test]
    fn test_thinking_no_false_positive() {
        let a = adapter();
        // Mid-line "reading" or "searching" should NOT match
        assert_eq!(a.analyze_line("I was reading the docs"), None);
        assert_eq!(a.analyze_line("Already searching..."), None);
    }

    #[test]
    fn test_writing_code_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Writing to src/lib.rs"),
            Some(AgentState::WritingCode)
        );
        assert_eq!(
            a.analyze_line("Creating new file config.toml"),
            Some(AgentState::WritingCode)
        );
        assert_eq!(
            a.analyze_line("Updating Cargo.toml"),
            Some(AgentState::WritingCode)
        );
        assert_eq!(
            a.analyze_line("Edited src/main.rs"),
            Some(AgentState::WritingCode)
        );
    }

    #[test]
    fn test_running_command_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Running: cargo test"),
            Some(AgentState::RunningCommand)
        );
        assert_eq!(
            a.analyze_line("Executing: npm install"),
            Some(AgentState::RunningCommand)
        );
        assert_eq!(
            a.analyze_line("$ cargo build"),
            Some(AgentState::RunningCommand)
        );
    }

    #[test]
    fn test_waiting_for_input_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Would you like me to proceed?"),
            Some(AgentState::WaitingForInput)
        );
        assert_eq!(
            a.analyze_line("Do you want to continue? (y/n)"),
            Some(AgentState::WaitingForInput)
        );
        assert_eq!(a.analyze_line("[Y/n]"), Some(AgentState::WaitingForInput));
    }

    #[test]
    fn test_error_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Error: compilation failed"),
            Some(AgentState::Error)
        );
        assert_eq!(
            a.analyze_line("Failed: test suite"),
            Some(AgentState::Error)
        );
        assert_eq!(a.analyze_line("✗ Build failed"), Some(AgentState::Error));
    }

    #[test]
    fn test_error_no_false_positive() {
        let a = adapter();
        // Compiler error lines mid-stream should not match (not line-start)
        assert_eq!(a.analyze_line("  --> src/main.rs:5:10"), None);
        // "error" in the middle of a line should not match
        assert_eq!(a.analyze_line("no error found"), None);
    }

    #[test]
    fn test_success_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("✓ All tests passed"),
            Some(AgentState::Success)
        );
        assert_eq!(
            a.analyze_line("✅ Task completed"),
            Some(AgentState::Success)
        );
        assert_eq!(
            a.analyze_line("Build completed successfully"),
            Some(AgentState::Success)
        );
    }

    #[test]
    fn test_success_no_false_positive() {
        let a = adapter();
        // Bare "done" should NOT match
        assert_eq!(a.analyze_line("Done processing files"), None);
        // "complete" without checkmark should NOT match
        assert_eq!(a.analyze_line("Download complete"), None);
    }

    #[test]
    fn test_no_match() {
        let a = adapter();
        assert_eq!(a.analyze_line("hello world"), None);
        assert_eq!(a.analyze_line(""), None);
        assert_eq!(a.analyze_line("normal output line"), None);
    }

    #[test]
    fn test_error_takes_priority_over_thinking() {
        let a = adapter();
        let state = a.analyze_line("Error: analyzing failed");
        assert_eq!(state, Some(AgentState::Error));
    }

    #[test]
    fn test_is_agent_command() {
        let a = adapter();
        assert!(a.is_agent_command("claude"));
        assert!(a.is_agent_command("claude code review"));
        assert!(a.is_agent_command("  claude  "));
        assert!(a.is_agent_command("/usr/local/bin/claude"));
        assert!(a.is_agent_command("claude-code"));
        assert!(a.is_agent_command("/usr/bin/claude-code"));

        assert!(!a.is_agent_command("bash"));
        assert!(!a.is_agent_command("vim"));
        assert!(!a.is_agent_command("echo claude"));
        assert!(!a.is_agent_command("grep claude-code logs"));
    }

    #[test]
    fn test_analyze_multi_line_output() {
        let a = adapter();
        let output = "⏳ Thinking...\nWriting to src/lib.rs\n✓ Done";
        let state = a.analyze_output(output);
        assert_eq!(state, Some(AgentState::Success));
    }
}
