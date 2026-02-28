//! Agent adapter trait for inferring agent state from terminal output.

use termesh_core::types::AgentState;

/// Adapter that analyzes terminal output to infer an AI agent's current state.
pub trait AgentAdapter: Send + Sync {
    /// Human-readable name for this adapter (e.g., "Claude Code").
    fn name(&self) -> &str;

    /// Analyze a line of terminal output and return the inferred state.
    ///
    /// Returns `None` if the line does not trigger a state change.
    fn analyze_line(&self, line: &str) -> Option<AgentState>;

    /// Analyze a chunk of terminal output (potentially multiple lines).
    ///
    /// Returns the last state change found, or `None` if no state change.
    fn analyze_output(&self, output: &str) -> Option<AgentState> {
        let mut last_state = None;
        for line in output.lines() {
            if let Some(state) = self.analyze_line(line) {
                last_state = Some(state);
            }
        }
        last_state
    }

    /// Check if a given command is likely an agent invocation.
    ///
    /// Used to auto-detect when a user starts an agent in a pane.
    fn is_agent_command(&self, command: &str) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyAdapter;

    impl AgentAdapter for DummyAdapter {
        fn name(&self) -> &str {
            "Dummy"
        }

        fn analyze_line(&self, line: &str) -> Option<AgentState> {
            if line.contains("thinking") {
                Some(AgentState::Thinking)
            } else {
                None
            }
        }

        fn is_agent_command(&self, command: &str) -> bool {
            command == "dummy-agent"
        }
    }

    #[test]
    fn test_analyze_output_returns_last_state() {
        let adapter = DummyAdapter;
        let output = "hello\nthinking about it\nnormal line\nthinking again";
        let state = adapter.analyze_output(output);
        assert_eq!(state, Some(AgentState::Thinking));
    }

    #[test]
    fn test_analyze_output_no_match() {
        let adapter = DummyAdapter;
        let state = adapter.analyze_output("hello world");
        assert_eq!(state, None);
    }

    #[test]
    fn test_is_agent_command() {
        let adapter = DummyAdapter;
        assert!(adapter.is_agent_command("dummy-agent"));
        assert!(!adapter.is_agent_command("bash"));
    }
}
