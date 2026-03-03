//! Session list panel: data model for the left sidebar in Focus mode.

use termesh_core::types::{AgentState, ProjectId, SessionId};

/// Maximum number of sessions displayed in the list.
const MAX_SESSIONS: usize = 32;

/// Inline editing state for renaming a session.
#[derive(Debug, Clone)]
pub struct EditState {
    /// Text buffer being edited.
    buffer: Vec<char>,
    /// Cursor position within the buffer.
    cursor: usize,
    /// Original label (for cancel/restore).
    original: String,
}

impl EditState {
    /// Create a new edit state from an existing label.
    pub fn new(label: &str) -> Self {
        let buffer: Vec<char> = label.chars().collect();
        let cursor = buffer.len();
        Self {
            buffer,
            cursor,
            original: label.to_string(),
        }
    }

    /// Insert a character at the cursor position.
    pub fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Delete the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.buffer.remove(self.cursor);
        }
    }

    /// Delete the character at the cursor position.
    pub fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
        }
    }

    /// Get the current buffer contents as a string.
    pub fn text(&self) -> String {
        self.buffer.iter().collect()
    }

    /// Get the cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the original label.
    pub fn original(&self) -> &str {
        &self.original
    }
}

/// A single entry in the session list.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    /// Session identifier.
    pub id: SessionId,
    /// Display label (e.g., "a3f1b2c9").
    pub label: String,
    /// Whether this session runs an AI agent (vs plain shell).
    pub is_agent: bool,
    /// Current agent state (or `AgentState::None` for shells).
    pub state: AgentState,
    /// Associated project (sessions with the same project are grouped together).
    pub project_id: Option<ProjectId>,
}

/// Manages the session list panel state.
#[derive(Debug, Clone)]
pub struct SessionList {
    /// Ordered list of session entries.
    entries: Vec<SessionEntry>,
    /// Currently selected index.
    selected: usize,
    /// Inline editing state (active when renaming a session).
    editing: Option<EditState>,
}

impl SessionList {
    /// Create an empty session list.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: 0,
            editing: None,
        }
    }

    /// Add a session entry. Returns `false` if the list is full.
    ///
    /// If the entry has a `project_id`, it is inserted after the last entry
    /// with the same project so that sessions are grouped by project.
    pub fn add(&mut self, entry: SessionEntry) -> bool {
        if self.entries.len() >= MAX_SESSIONS {
            return false;
        }
        if let Some(pid) = entry.project_id {
            if let Some(last_idx) = self.entries.iter().rposition(|e| e.project_id == Some(pid)) {
                self.entries.insert(last_idx + 1, entry);
                return true;
            }
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

    /// Update the `is_agent` flag for a session.
    pub fn update_is_agent(&mut self, id: SessionId, is_agent: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.is_agent = is_agent;
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

    /// Start editing the currently selected session's label.
    pub fn start_editing(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            self.editing = Some(EditState::new(&entry.label));
        }
    }

    /// Confirm editing: apply the buffer as the new label.
    pub fn confirm_editing(&mut self) {
        if let Some(edit) = self.editing.take() {
            let text = edit.text();
            if !text.is_empty() {
                if let Some(entry) = self.entries.get_mut(self.selected) {
                    entry.label = text;
                }
            }
        }
    }

    /// Cancel editing: discard changes and restore original label.
    pub fn cancel_editing(&mut self) {
        self.editing = None;
    }

    /// Whether a session is currently being edited.
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// Get a reference to the edit state.
    pub fn edit_state(&self) -> Option<&EditState> {
        self.editing.as_ref()
    }

    /// Get a mutable reference to the edit state.
    pub fn edit_state_mut(&mut self) -> Option<&mut EditState> {
        self.editing.as_mut()
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
            project_id: None,
        }
    }

    fn make_entry_with_project(id: u64, label: &str, project_id: ProjectId) -> SessionEntry {
        SessionEntry {
            id: SessionId(id),
            label: label.to_string(),
            is_agent: true,
            state: AgentState::Idle,
            project_id: Some(project_id),
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

    // ── EditState tests ───────────────────────────────────────────────────

    #[test]
    fn test_edit_state_new() {
        let edit = super::EditState::new("Hello");
        assert_eq!(edit.text(), "Hello");
        assert_eq!(edit.cursor(), 5); // cursor at end
        assert_eq!(edit.original(), "Hello");
    }

    #[test]
    fn test_edit_insert_at_cursor() {
        let mut edit = super::EditState::new("AB");
        edit.move_left(); // cursor at 1
        edit.insert('X');
        assert_eq!(edit.text(), "AXB");
        assert_eq!(edit.cursor(), 2);
    }

    #[test]
    fn test_edit_backspace() {
        let mut edit = super::EditState::new("ABC");
        edit.backspace();
        assert_eq!(edit.text(), "AB");
        assert_eq!(edit.cursor(), 2);
    }

    #[test]
    fn test_edit_backspace_at_beginning() {
        let mut edit = super::EditState::new("A");
        edit.move_left();
        assert_eq!(edit.cursor(), 0);
        edit.backspace(); // no-op
        assert_eq!(edit.text(), "A");
        assert_eq!(edit.cursor(), 0);
    }

    #[test]
    fn test_edit_delete() {
        let mut edit = super::EditState::new("ABC");
        edit.move_left();
        edit.move_left(); // cursor at 1
        edit.delete();
        assert_eq!(edit.text(), "AC");
        assert_eq!(edit.cursor(), 1);
    }

    #[test]
    fn test_edit_delete_at_end() {
        let mut edit = super::EditState::new("AB");
        edit.delete(); // no-op (cursor at end)
        assert_eq!(edit.text(), "AB");
    }

    #[test]
    fn test_edit_move_left_at_zero() {
        let mut edit = super::EditState::new("A");
        edit.move_left();
        assert_eq!(edit.cursor(), 0);
        edit.move_left(); // no-op
        assert_eq!(edit.cursor(), 0);
    }

    #[test]
    fn test_edit_move_right_at_end() {
        let mut edit = super::EditState::new("AB");
        assert_eq!(edit.cursor(), 2);
        edit.move_right(); // no-op
        assert_eq!(edit.cursor(), 2);
    }

    // ── Editing lifecycle tests ───────────────────────────────────────────

    #[test]
    fn test_start_editing() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "Backend", true));
        list.start_editing();
        assert!(list.is_editing());
        assert_eq!(list.edit_state().unwrap().text(), "Backend");
    }

    #[test]
    fn test_start_editing_on_empty_list() {
        let mut list = SessionList::new();
        list.start_editing();
        assert!(!list.is_editing());
    }

    #[test]
    fn test_confirm_editing_applies_label() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "Old", true));
        list.start_editing();
        list.edit_state_mut().unwrap().backspace();
        list.edit_state_mut().unwrap().backspace();
        list.edit_state_mut().unwrap().backspace();
        list.edit_state_mut().unwrap().insert('N');
        list.edit_state_mut().unwrap().insert('e');
        list.edit_state_mut().unwrap().insert('w');
        list.confirm_editing();
        assert!(!list.is_editing());
        assert_eq!(list.entries()[0].label, "New");
    }

    #[test]
    fn test_confirm_editing_rejects_empty() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "Keep", true));
        list.start_editing();
        // Clear all characters
        for _ in 0..4 {
            list.edit_state_mut().unwrap().backspace();
        }
        list.confirm_editing();
        assert_eq!(list.entries()[0].label, "Keep"); // unchanged
    }

    // ── Project grouping tests ─────────────────────────────────────────

    #[test]
    fn test_add_groups_by_project() {
        let mut list = SessionList::new();
        let px = ProjectId(100);
        let py = ProjectId(200);

        list.add(make_entry_with_project(1, "A", px)); // A(X)
        list.add(make_entry_with_project(2, "B", py)); // B(Y)
        list.add(make_entry_with_project(3, "C", px)); // C(X) → inserted after A

        assert_eq!(list.entries()[0].id, SessionId(1)); // A
        assert_eq!(list.entries()[1].id, SessionId(3)); // C (grouped with A)
        assert_eq!(list.entries()[2].id, SessionId(2)); // B
    }

    #[test]
    fn test_add_no_project_appends() {
        let mut list = SessionList::new();
        let px = ProjectId(100);

        list.add(make_entry_with_project(1, "A", px));
        list.add(make_entry(2, "B", false)); // no project → appended
        list.add(make_entry_with_project(3, "C", px));

        assert_eq!(list.entries()[0].id, SessionId(1)); // A
        assert_eq!(list.entries()[1].id, SessionId(3)); // C (grouped with A)
        assert_eq!(list.entries()[2].id, SessionId(2)); // B (no project, at end)
    }

    #[test]
    fn test_cancel_editing() {
        let mut list = SessionList::new();
        list.add(make_entry(1, "Original", true));
        list.start_editing();
        list.edit_state_mut().unwrap().insert('X');
        list.cancel_editing();
        assert!(!list.is_editing());
        assert_eq!(list.entries()[0].label, "Original"); // unchanged
    }
}
