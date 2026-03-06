//! Claude Code adapter: infers agent state from Claude Code terminal output.
//!
//! This adapter is **stateful** — it tracks the current agent state to suppress
//! false transitions. For example, when the agent is running a command, `error:`
//! lines in command output are ignored (they are the command's output, not the
//! agent's state).

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
    /// Claude Code idle prompt — indicates agent is ready for input.
    idle_prompt: Regex,
}

impl StatePatterns {
    fn compile() -> Self {
        Self {
            // Claude Code uses spinner chars (✶✻✽✢·*) followed by "Working…"
            // or "(thinking)" during active processing.
            thinking: Regex::new(r"(?i)(\(thinking\)|[✶✻✽✢·\*]\s*Working)")
                .expect("invalid thinking regex"),
            // File write indicators from Claude Code output.
            writing_code: Regex::new(
                r"(?i)^(Writing to |Creating |Updating |Edit(ing|ed) |Wrote )",
            )
            .expect("invalid writing_code regex"),
            // "Running:" and "Executing:" are Claude Code prefixes.
            // Also match "$ " prompt at line start.
            running_command: Regex::new(r"(?i)^(Running: |Executing: |\$\s+\S)")
                .expect("invalid running_command regex"),
            // Claude Code permission prompts and user input requests.
            waiting_for_input: Regex::new(
                r"(?i)(would you like|do you want|\by/n\b|\[Y/n\]|\[y/N\]|Allow |Deny )",
            )
            .expect("invalid waiting_for_input regex"),
            // Anchored: "Error:" or "Failed:" at line start, or marker symbols.
            error: Regex::new(r"(^(?i)(error:|failed:)|✗)").expect("invalid error regex"),
            // Only match Unicode checkmarks, or "successfully" as a word.
            // Avoid bare "done" and "complete" which are too common.
            success: Regex::new(r"(✓|✅|\bsuccessfully\b)").expect("invalid success regex"),
            // Claude Code shows "❯" alone when ready for user input.
            idle_prompt: Regex::new(r"^❯\s*$").expect("invalid idle_prompt regex"),
        }
    }
}

/// Adapter for Claude Code AI agent.
pub struct ClaudeCodeAdapter {
    patterns: StatePatterns,
    /// Current tracked state for suppressing false transitions.
    current_state: AgentState,
}

impl ClaudeCodeAdapter {
    /// Create a new Claude Code adapter with default patterns.
    pub fn new() -> Self {
        Self {
            patterns: StatePatterns::compile(),
            current_state: AgentState::Idle,
        }
    }

    /// Analyze a line with state context.
    ///
    /// When the agent is in `RunningCommand` or `WritingCode` state,
    /// error/success patterns from command output are suppressed —
    /// only idle prompt or new agent-level transitions are accepted.
    fn analyze_line_stateful(&self, line: &str) -> Option<AgentState> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Idle prompt always transitions back to Idle regardless of state.
        if self.patterns.idle_prompt.is_match(trimmed) {
            return Some(AgentState::Idle);
        }

        // Thinking pattern always takes effect (agent started processing).
        if self.patterns.thinking.is_match(trimmed) {
            return Some(AgentState::Thinking);
        }

        // WaitingForInput always takes effect (permission prompt).
        if self.patterns.waiting_for_input.is_match(trimmed) {
            return Some(AgentState::WaitingForInput);
        }

        // When in RunningCommand or WritingCode, suppress error/success
        // patterns because they come from the command's output, not the
        // agent itself. Only agent-level patterns (thinking, idle, waiting)
        // can transition out.
        let in_command_output = matches!(
            self.current_state,
            AgentState::RunningCommand | AgentState::WritingCode
        );

        if !in_command_output {
            if self.patterns.error.is_match(trimmed) {
                return Some(AgentState::Error);
            }
            if self.patterns.success.is_match(trimmed) {
                return Some(AgentState::Success);
            }
        }

        if self.patterns.running_command.is_match(trimmed) {
            return Some(AgentState::RunningCommand);
        }
        if self.patterns.writing_code.is_match(trimmed) {
            return Some(AgentState::WritingCode);
        }

        None
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentAdapter for ClaudeCodeAdapter {
    fn id(&self) -> &str {
        "claude"
    }

    fn name(&self) -> &str {
        "Claude Code"
    }

    fn analyze_line(&self, line: &str) -> Option<AgentState> {
        // Stateless fallback (used by default analyze_output trait impl).
        // The stateful path is used via analyze_output_stateful.
        self.analyze_line_stateful(line)
    }

    fn analyze_output(&self, output: &str) -> Option<AgentState> {
        // Use stateful analysis: each line is evaluated in context
        // of the current state (set by the previous line in this chunk).
        let mut last_state = None;
        let mut effective_state = self.current_state;
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Idle prompt always accepted.
            if self.patterns.idle_prompt.is_match(trimmed) {
                last_state = Some(AgentState::Idle);
                effective_state = AgentState::Idle;
                continue;
            }

            if self.patterns.thinking.is_match(trimmed) {
                last_state = Some(AgentState::Thinking);
                effective_state = AgentState::Thinking;
                continue;
            }

            if self.patterns.waiting_for_input.is_match(trimmed) {
                last_state = Some(AgentState::WaitingForInput);
                effective_state = AgentState::WaitingForInput;
                continue;
            }

            let in_command_output = matches!(
                effective_state,
                AgentState::RunningCommand | AgentState::WritingCode
            );

            if !in_command_output {
                if self.patterns.error.is_match(trimmed) {
                    last_state = Some(AgentState::Error);
                    effective_state = AgentState::Error;
                    continue;
                }
                if self.patterns.success.is_match(trimmed) {
                    last_state = Some(AgentState::Success);
                    effective_state = AgentState::Success;
                    continue;
                }
            }

            if self.patterns.running_command.is_match(trimmed) {
                last_state = Some(AgentState::RunningCommand);
                effective_state = AgentState::RunningCommand;
                continue;
            }
            if self.patterns.writing_code.is_match(trimmed) {
                last_state = Some(AgentState::WritingCode);
                effective_state = AgentState::WritingCode;
            }
        }
        last_state
    }

    fn update_state(&mut self, state: AgentState) {
        self.current_state = state;
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
        // Claude Code spinner + Working patterns
        assert_eq!(a.analyze_line("✶ Working…"), Some(AgentState::Thinking));
        assert_eq!(a.analyze_line("✻ Working…"), Some(AgentState::Thinking));
        assert_eq!(
            a.analyze_line("* Working… (thought for 1s)"),
            Some(AgentState::Thinking)
        );
        assert_eq!(a.analyze_line("(thinking)"), Some(AgentState::Thinking));
        assert_eq!(a.analyze_line("✢(thinking)"), Some(AgentState::Thinking));
    }

    #[test]
    fn test_thinking_no_false_positive() {
        let a = adapter();
        // Normal text should NOT match
        assert_eq!(a.analyze_line("I was reading the docs"), None);
        assert_eq!(a.analyze_line("hello world"), None);
    }

    #[test]
    fn test_idle_prompt() {
        let a = adapter();
        assert_eq!(a.analyze_line("❯"), Some(AgentState::Idle));
        assert_eq!(a.analyze_line("❯ "), Some(AgentState::Idle));
        // Prompt with text is user typing, not idle
        assert_eq!(a.analyze_line("❯ hello"), None);
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
        let state = a.analyze_line("Error: working failed");
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
        let output = "✶ Working…\nWriting to src/lib.rs\n✓ Done";
        let state = a.analyze_output(output);
        // ✓ appears while in WritingCode state → suppressed
        // Last effective state is WritingCode
        assert_eq!(state, Some(AgentState::WritingCode));
    }

    // ── Stateful analysis tests ─────────────────────────────────────────

    #[test]
    fn test_stateful_suppress_error_during_running_command() {
        let mut a = adapter();
        // Agent starts running a command
        a.update_state(AgentState::RunningCommand);

        // Command output contains "error:" — should be suppressed
        let state = a.analyze_output("error: unused variable `x`");
        assert_eq!(state, None);
    }

    #[test]
    fn test_stateful_suppress_success_during_running_command() {
        let mut a = adapter();
        a.update_state(AgentState::RunningCommand);

        // Command output contains "✓" — should be suppressed
        let state = a.analyze_output("✓ test passed");
        assert_eq!(state, None);
    }

    #[test]
    fn test_stateful_idle_prompt_exits_running_command() {
        let mut a = adapter();
        a.update_state(AgentState::RunningCommand);

        // Idle prompt should always transition
        let state = a.analyze_output("❯");
        assert_eq!(state, Some(AgentState::Idle));
    }

    #[test]
    fn test_stateful_thinking_exits_running_command() {
        let mut a = adapter();
        a.update_state(AgentState::RunningCommand);

        // Agent starts thinking again — always accepted
        let state = a.analyze_output("✶ Working…");
        assert_eq!(state, Some(AgentState::Thinking));
    }

    #[test]
    fn test_stateful_error_shown_when_not_in_command() {
        let mut a = adapter();
        a.update_state(AgentState::Thinking);

        // Error not inside command output → shown
        let state = a.analyze_output("Error: compilation failed");
        assert_eq!(state, Some(AgentState::Error));
    }

    #[test]
    fn test_stateful_multi_line_command_with_errors() {
        let a = adapter();
        // Agent runs command, command outputs errors, then agent goes back to thinking
        let output = "Running: cargo test\n\
                       error: unused variable\n\
                       Error: test failed\n\
                       ✶ Working…";
        let state = a.analyze_output(output);
        // Running → (error suppressed) → (error suppressed) → Thinking
        assert_eq!(state, Some(AgentState::Thinking));
    }

    #[test]
    fn test_stateful_writing_suppresses_success() {
        let a = adapter();
        let output = "Writing to src/lib.rs\n✓ Wrote 42 lines";
        let state = a.analyze_output(output);
        // WritingCode → (✓ suppressed during WritingCode)
        assert_eq!(state, Some(AgentState::WritingCode));
    }
}
