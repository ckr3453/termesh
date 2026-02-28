//! Agent adapter trait for inferring agent state from terminal output.
//!
//! # Adding a new agent adapter
//!
//! 1. Create a new module (e.g., `my_agent.rs`) in this crate.
//! 2. Implement [`AgentAdapter`] for your struct:
//!    - `id()` — short identifier used in config files (e.g., `"my-agent"`)
//!    - `name()` — human-readable display name
//!    - `analyze_line()` — detect state transitions from terminal output
//!    - `is_agent_command()` — recognize agent invocation commands
//! 3. Register it in [`AdapterRegistry::with_defaults()`](crate::registry::AdapterRegistry::with_defaults).

use termesh_core::types::AgentState;

/// Adapter that analyzes terminal output to infer an AI agent's current state.
///
/// Each adapter corresponds to a specific AI coding agent (Claude Code,
/// Gemini CLI, etc.) and knows how to parse its terminal output into
/// structured state transitions.
pub trait AgentAdapter: Send + Sync {
    /// Short identifier for config/CLI use (e.g., "claude", "gemini").
    fn id(&self) -> &str;

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
        fn id(&self) -> &str {
            "dummy"
        }

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
