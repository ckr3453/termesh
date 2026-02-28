//! Codex CLI adapter: infers agent state from OpenAI Codex CLI terminal output.
//!
//! Codex CLI (OpenAI's coding assistant) outputs patterns like:
//! - Reasoning/thinking indicators
//! - File write notifications
//! - Command execution markers
//! - Approval prompts
//! - Error/success markers

use crate::adapter::AgentAdapter;
use regex::Regex;
use termesh_core::types::AgentState;

/// Pattern set for detecting Codex CLI state transitions.
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
            thinking: Regex::new(
                r"(?i)^(reasoning|thinking|planning|reading |analyzing |searching )",
            )
            .expect("invalid thinking regex"),
            writing_code: Regex::new(
                r"(?i)^(wrote |writing |creating |patching |applied patch|edit(ing|ed) )",
            )
            .expect("invalid writing_code regex"),
            running_command: Regex::new(r"(?i)^(running:|exec:|>\s+\S|\$\s+\S)")
                .expect("invalid running_command regex"),
            waiting_for_input: Regex::new(
                r"(?i)(approve\?|do you want|proceed\?|\by/n\b|\[yes/no\]|allow\?)",
            )
            .expect("invalid waiting_for_input regex"),
            error: Regex::new(r"(^(?i)(error:|failed:|aborted:)|❌|✗)")
                .expect("invalid error regex"),
            success: Regex::new(r"(✓|✅|\bdone\b.*successfully|\bcompleted\b)")
                .expect("invalid success regex"),
        }
    }
}

/// Adapter for OpenAI Codex CLI agent.
pub struct CodexCliAdapter {
    patterns: StatePatterns,
}

impl CodexCliAdapter {
    /// Create a new Codex CLI adapter with default patterns.
    pub fn new() -> Self {
        Self {
            patterns: StatePatterns::compile(),
        }
    }
}

impl Default for CodexCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentAdapter for CodexCliAdapter {
    fn id(&self) -> &str {
        "codex"
    }

    fn name(&self) -> &str {
        "Codex CLI"
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
        first_token == "codex"
            || first_token == "openai-codex"
            || first_token.ends_with("/codex")
            || first_token.ends_with("/openai-codex")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> CodexCliAdapter {
        CodexCliAdapter::new()
    }

    #[test]
    fn test_id_and_name() {
        let a = adapter();
        assert_eq!(a.id(), "codex");
        assert_eq!(a.name(), "Codex CLI");
    }

    #[test]
    fn test_thinking_patterns() {
        let a = adapter();
        assert_eq!(a.analyze_line("Reasoning..."), Some(AgentState::Thinking));
        assert_eq!(
            a.analyze_line("Thinking about approach"),
            Some(AgentState::Thinking)
        );
        assert_eq!(
            a.analyze_line("Reading src/main.rs"),
            Some(AgentState::Thinking)
        );
        assert_eq!(
            a.analyze_line("Searching for files"),
            Some(AgentState::Thinking)
        );
    }

    #[test]
    fn test_writing_code_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Wrote src/lib.rs"),
            Some(AgentState::WritingCode)
        );
        assert_eq!(
            a.analyze_line("Creating config.toml"),
            Some(AgentState::WritingCode)
        );
        assert_eq!(
            a.analyze_line("Applied patch to main.rs"),
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
            a.analyze_line("$ cargo build"),
            Some(AgentState::RunningCommand)
        );
        assert_eq!(
            a.analyze_line("> npm install"),
            Some(AgentState::RunningCommand)
        );
    }

    #[test]
    fn test_waiting_for_input_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Approve? (y/n)"),
            Some(AgentState::WaitingForInput)
        );
        assert_eq!(
            a.analyze_line("Do you want to proceed?"),
            Some(AgentState::WaitingForInput)
        );
        assert_eq!(
            a.analyze_line("[yes/no]"),
            Some(AgentState::WaitingForInput)
        );
    }

    #[test]
    fn test_error_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("Error: compilation failed"),
            Some(AgentState::Error)
        );
        assert_eq!(
            a.analyze_line("Aborted: user cancelled"),
            Some(AgentState::Error)
        );
        assert_eq!(a.analyze_line("❌ Failed"), Some(AgentState::Error));
    }

    #[test]
    fn test_success_patterns() {
        let a = adapter();
        assert_eq!(
            a.analyze_line("✅ All tests passed"),
            Some(AgentState::Success)
        );
        assert_eq!(a.analyze_line("Task completed"), Some(AgentState::Success));
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
        assert!(a.is_agent_command("codex"));
        assert!(a.is_agent_command("codex fix this bug"));
        assert!(a.is_agent_command("openai-codex"));
        assert!(a.is_agent_command("/usr/local/bin/codex"));

        assert!(!a.is_agent_command("bash"));
        assert!(!a.is_agent_command("echo codex"));
    }

    #[test]
    fn test_multi_line_analysis() {
        let a = adapter();
        let output = "Reasoning...\nWrote src/lib.rs\n✅ Done";
        let state = a.analyze_output(output);
        assert_eq!(state, Some(AgentState::Success));
    }
}
