//! Session list panel: data model for the left sidebar in Focus mode.

use termesh_core::types::{AgentState, SessionId};

/// Maximum number of sessions displayed in the list.
const MAX_SESSIONS: usize = 32;

/// A single entry in the session list.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    /// Session identifier.
    pub id: SessionId,
    /// Display label (e.g., "Backend", "Frontend").
    pub label: String,
    /// Whether this session runs an AI agent (vs plain shell).
    pub is_agent: bool,
    /// Current agent state (or `AgentState::None` for shells).
    pub state: AgentState,
}

/// Manages the session list panel state.
#[derive(Debug, Clone)]
pub struct SessionList {
    /// Ordered list of session entries.
    entries: Vec<SessionEntry>,
    /// Currently selected index.
    selected: usize,
}

impl SessionList {
    /// Create an empty session list.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: 0,
        }
    }

    /// Add a session entry. Returns `false` if the list is full.
    pub fn add(&mut self, entry: SessionEntry) -> bool {
        if self.entries.len() >= MAX_SESSIONS {
            return false;
        }
        self.entries.push(entry);
        true
    }

    /// Remove a session by ID. Returns `true` if removed.
    pub fn remove(&mut self, id: SessionId) -> bool {
        if let Some(idx) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(idx);
            if self.entries.is_empty() {
                self.selected = 0;
            } else if self.selected >= self.entries.len() {
                self.selected = self.entries.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Update the agent state of a session.
    pub fn update_state(&mut self, id: SessionId, state: AgentState) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.state = state;
        }
    }

    /// Select the next session (wrapping around).
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    /// Select the previous session (wrapping around).
    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            self.selected = if self.selected == 0 {
                self.entries.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Select a session by ID.
    pub fn select_by_id(&mut self, id: SessionId) {
        if let Some(idx) = self.entries.iter().position(|e| e.id == id) {
            self.selected = idx;
        }
    }

    /// Get the currently selected entry.
    pub fn selected_entry(&self) -> Option<&SessionEntry> {
        self.entries.get(self.selected)
    }

    /// Get the selected index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Get all entries.
    pub fn entries(&self) -> &[SessionEntry] {
        &self.entries
    }

    /// Number of sessions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the selected session ID.
    pub fn selected_id(&self) -> Option<SessionId> {
        self.selected_entry().map(|e| e.id)
    }
}

impl Default for SessionList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: u64, label: &str, is_agent: bool) -> SessionEntry {
        SessionEntry {
            id: SessionId(id),
            label: label.to_string(),
            is_agent,
            state: if is_agent {
                AgentState::Idle
            } else {
                AgentState::None
            },
        }
    }

    #[test]
    fn test_add_and_entries() {
        let mut list = SessionList::new();
        assert!(list.add(make_entry(1, "Backend", true)));
        assert!(list.add(make_entry(2, "Shell", false)));
        assert_eq!(list.len(), 2);
        assert_eq!(list.entries()[0].label, "Backend");
        assert_eq!(list.entries()[1].label, "Shell");
    }

    #[test]
    fn test_add_max_sessions() {
        let mut list = SessionList::new();
        for i in 0..MAX_SESSIONS {
            assert!(list.add(make_entry(i as u64, "S", false)));
        }
        assert!(!list.add(make_entry(99, "Overflow", false)));
        assert_eq!(list.len(), MAX_SESSIONS);
    }

    #[test]
    fn test_remove() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "A", false));
        list.add(make_entry(2, "B", false));
        list.add(make_entry(3, "C", false));

        assert!(list.remove(SessionId(2)));
        assert_eq!(list.len(), 2);
        assert_eq!(list.entries()[0].id, SessionId(1));
        assert_eq!(list.entries()[1].id, SessionId(3));
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "A", false));
        assert!(!list.remove(SessionId(99)));
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_remove_adjusts_selected() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "A", false));
        list.add(make_entry(2, "B", false));
        list.select_next(); // selected = 1
        list.remove(SessionId(2)); // remove selected
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_select_next_wraps() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "A", false));
        list.add(make_entry(2, "B", false));

        assert_eq!(list.selected_index(), 0);
        list.select_next();
        assert_eq!(list.selected_index(), 1);
        list.select_next();
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_select_prev_wraps() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "A", false));
        list.add(make_entry(2, "B", false));

        assert_eq!(list.selected_index(), 0);
        list.select_prev();
        assert_eq!(list.selected_index(), 1);
        list.select_prev();
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_select_by_id() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "A", false));
        list.add(make_entry(2, "B", false));
        list.add(make_entry(3, "C", false));

        list.select_by_id(SessionId(3));
        assert_eq!(list.selected_index(), 2);
    }

    #[test]
    fn test_update_state() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "Agent", true));

        list.update_state(SessionId(1), AgentState::Thinking);
        assert_eq!(list.entries()[0].state, AgentState::Thinking);
    }

    #[test]
    fn test_selected_entry() {
        let mut list = SessionList::new();
        assert!(list.selected_entry().is_none());

        list.add(make_entry(1, "A", false));
        assert_eq!(list.selected_entry().unwrap().id, SessionId(1));
    }

    #[test]
    fn test_selected_id() {
        let mut list = SessionList::new();
        assert!(list.selected_id().is_none());

        list.add(make_entry(5, "X", false));
        assert_eq!(list.selected_id(), Some(SessionId(5)));
    }

    #[test]
    fn test_empty() {
        let list = SessionList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_select_on_empty_no_panic() {
        let mut list = SessionList::new();
        list.select_next();
        list.select_prev();
        list.select_by_id(SessionId(1));
        assert_eq!(list.selected_index(), 0);
    }
}
