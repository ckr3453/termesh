//! Gemini CLI adapter: infers agent state from Gemini CLI terminal output.
//!
//! Gemini CLI (Google's AI coding assistant) outputs patterns like:
//! - Thinking/analyzing indicators
//! - File editing notifications
//! - Shell command execution
//! - User confirmation prompts
//! - Error/success markers

use crate::adapter::AgentAdapter;
use regex::Regex;
use termesh_core::types::AgentState;

/// Pattern set for detecting Gemini CLI state transitions.
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
            thinking: Regex::new(r"(?i)^(\[thinking\]|>\s*(thinking|analyzing|reading|looking))")
                .expect("invalid thinking regex"),
            writing_code: Regex::new(
                r"(?i)^(editing |writing |creating |modifying |updated |patching )",
            )
            .expect("invalid writing_code regex"),
            running_command: Regex::new(r"(?i)^(running |executing |shell:|❯\s+\S)")
                .expect("invalid running_command regex"),
            waiting_for_input: Regex::new(
                r"(?i)(do you approve|proceed\?|confirm\?|\by/n\b|\[Y/n\]|\[y/N\]|allow this)",
            )
            .expect("invalid waiting_for_input regex"),
            error: Regex::new(r"(^(?i)(error:|failed:|✗)|❌)").expect("invalid error regex"),
            success: Regex::new(r"(✓|✅|🎉|\bcompleted successfully\b)")
                .expect("invalid success regex"),
        }
    }
}

/// Adapter for Google Gemini CLI agent.
pub struct GeminiCliAdapter {
    patterns: StatePatterns,
}

impl GeminiCliAdapter {
    /// Create a new Gemini CLI adapter with default patterns.
    pub fn new() -> Self {
        Self {
            patterns: StatePatterns::compile(),
        }
    }
}

impl Default for GeminiCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentAdapter for GeminiCliAdapter {
    fn id(&self) -> &str {
        "gemini"
    }

    fn name(&self) -> &str {
        "Gemini CLI"
    }

    fn analyze_line(&self, line: &str) -> Option<AgentState> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

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
        first_token == "gemini"
            || first_token == "gemini-cli"
            || first_token.ends_with("/gemini")
            || first_token.ends_with("/gemini-cli")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> GeminiCliAdapter {
        GeminiCliAdapter::new()
    }

    #[test]
    fn test_id_and_name() {
        let a = adapter();
        assert_eq!(a.id(), "gemini");
        assert_eq!(a.name(), "Gemini CLI");
    }

    #[test]
    fn test_thinking_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("[thinking] about the problem"),
            Some(AgentState::Thinking)
        );
        assert_eq!(
            a.analyze_line("> Analyzing codebase"),
            Some(AgentState::Thinking)
        );
        assert_eq!(
            a.analyze_line("> Looking at files"),
            Some(AgentState::Thinking)
        );
    }

    #[test]
    fn test_writing_code_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Editing src/main.rs"),
            Some(AgentState::WritingCode)
        );
        assert_eq!(
            a.analyze_line("Creating new file utils.rs"),
            Some(AgentState::WritingCode)
        );
        assert_eq!(
            a.analyze_line("Updated Cargo.toml"),
            Some(AgentState::WritingCode)
        );
    }

    #[test]
    fn test_running_command_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Running cargo test"),
            Some(AgentState::RunningCommand)
        );
        assert_eq!(
            a.analyze_line("Shell: npm install"),
            Some(AgentState::RunningCommand)
        );
        assert_eq!(
            a.analyze_line("❯ cargo build"),
            Some(AgentState::RunningCommand)
        );
    }

    #[test]
    fn test_waiting_for_input_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Do you approve this change?"),
            Some(AgentState::WaitingForInput)
        );
        assert_eq!(
            a.analyze_line("Proceed? (y/n)"),
            Some(AgentState::WaitingForInput)
        );
        assert_eq!(
            a.analyze_line("Allow this action? [Y/n]"),
            Some(AgentState::WaitingForInput)
        );
    }

    #[test]
    fn test_error_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Error: build failed"),
            Some(AgentState::Error)
        );
        assert_eq!(
            a.analyze_line("❌ Operation failed"),
            Some(AgentState::Error)
        );
    }

    #[test]
    fn test_success_patterns() {
        let a = adapter();
        assert_eq!(a.analyze_line("✅ All done!"), Some(AgentState::Success));
        assert_eq!(
            a.analyze_line("🎉 Task completed"),
            Some(AgentState::Success)
        );
        assert_eq!(
            a.analyze_line("Build completed successfully"),
            Some(AgentState::Success)
        );
    }

    #[test]
    fn test_no_match() {
        let a = adapter();
        assert_eq!(a.analyze_line("hello world"), None);
        assert_eq!(a.analyze_line(""), None);
    }

    #[test]
    fn test_is_agent_command() {
        let a = adapter();
        assert!(a.is_agent_command("gemini"));
        assert!(a.is_agent_command("gemini help me code"));
        assert!(a.is_agent_command("gemini-cli"));
        assert!(a.is_agent_command("/usr/local/bin/gemini"));

        assert!(!a.is_agent_command("bash"));
        assert!(!a.is_agent_command("echo gemini"));
    }

    #[test]
    fn test_multi_line_analysis() {
        let a = adapter();
        let output = "[thinking] analyzing\nEditing src/lib.rs\n✅ Done";
        let state = a.analyze_output(output);
        assert_eq!(state, Some(AgentState::Success));
    }
}
