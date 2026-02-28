//! Adapter registry for looking up agent adapters by name.

use crate::adapter::AgentAdapter;
use crate::claude_code::ClaudeCodeAdapter;
use std::collections::HashMap;

/// Registry that maps agent identifiers to their adapter implementations.
///
/// Use [`AdapterRegistry::with_defaults()`] to get a registry pre-loaded
/// with all built-in adapters.
pub struct AdapterRegistry {
    adapters: HashMap<String, Box<dyn AgentAdapter>>,
}

impl AdapterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all built-in adapters.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(ClaudeCodeAdapter::new());
        registry
    }

    /// Register a new adapter. Replaces any existing adapter with the same id.
    pub fn register(&mut self, adapter: impl AgentAdapter + 'static) {
        self.adapters
            .insert(adapter.id().to_string(), Box::new(adapter));
    }

    /// Look up an adapter by its identifier (e.g., "claude").
    pub fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        self.adapters.get(id).map(|a| a.as_ref())
    }

    /// List all registered adapter identifiers.
    pub fn list_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.adapters.keys().map(|s| s.as_str()).collect();
        ids.sort();
        ids
    }

    /// Check if a command matches any registered agent adapter.
    ///
    /// Returns the adapter id if a match is found.
    pub fn detect_agent(&self, command: &str) -> Option<&str> {
        for (id, adapter) in &self.adapters {
            if adapter.is_agent_command(command) {
                return Some(id.as_str());
            }
        }
        None
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termesh_core::types::AgentState;

    #[test]
    fn test_with_defaults_has_claude() {
        let registry = AdapterRegistry::with_defaults();
        assert!(registry.get("claude").is_some());
        assert_eq!(registry.get("claude").unwrap().name(), "Claude Code");
    }

    #[test]
    fn test_get_nonexistent() {
        let registry = AdapterRegistry::with_defaults();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_ids() {
        let registry = AdapterRegistry::with_defaults();
        let ids = registry.list_ids();
        assert!(ids.contains(&"claude"));
    }

    #[test]
    fn test_detect_agent() {
        let registry = AdapterRegistry::with_defaults();
        assert_eq!(registry.detect_agent("claude"), Some("claude"));
        assert_eq!(registry.detect_agent("claude code review"), Some("claude"));
        assert_eq!(registry.detect_agent("bash"), None);
    }

    #[test]
    fn test_register_custom_adapter() {
        use crate::adapter::AgentAdapter;

        struct CustomAdapter;

        impl AgentAdapter for CustomAdapter {
            fn id(&self) -> &str {
                "custom"
            }
            fn name(&self) -> &str {
                "Custom Agent"
            }
            fn analyze_line(&self, line: &str) -> Option<AgentState> {
                if line.contains("working") {
                    Some(AgentState::Thinking)
                } else {
                    None
                }
            }
            fn is_agent_command(&self, command: &str) -> bool {
                command.starts_with("custom-agent")
            }
        }

        let mut registry = AdapterRegistry::with_defaults();
        registry.register(CustomAdapter);

        assert!(registry.get("custom").is_some());
        assert_eq!(registry.get("custom").unwrap().name(), "Custom Agent");
        assert_eq!(registry.detect_agent("custom-agent run"), Some("custom"));
    }

    #[test]
    fn test_empty_registry() {
        let registry = AdapterRegistry::new();
        assert!(registry.list_ids().is_empty());
        assert!(registry.get("claude").is_none());
    }
}
