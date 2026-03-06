//! Keybinding mapping table.

use crate::action::Action;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Modifier keys for a keybinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        logo: false,
    };

    pub const LOGO: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        logo: true,
    };

    pub const LOGO_SHIFT: Self = Self {
        ctrl: false,
        alt: false,
        shift: true,
        logo: true,
    };

    pub const CTRL: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
        logo: false,
    };

    pub const CTRL_SHIFT: Self = Self {
        ctrl: true,
        alt: false,
        shift: true,
        logo: false,
    };
}

/// A key identifier (simplified, platform-independent).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
}

/// A keybinding: modifier + key combination.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Keybinding {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Keybinding {
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }
}

/// Keybinding map from key combos to actions.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: HashMap<Keybinding, Action>,
}

impl Keymap {
    /// Create an empty keymap.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Platform primary modifier: Ctrl on Windows/Linux, Logo (Cmd) on macOS.
    #[cfg(target_os = "macos")]
    pub const PRIMARY: Modifiers = Modifiers::LOGO;
    #[cfg(not(target_os = "macos"))]
    pub const PRIMARY: Modifiers = Modifiers::CTRL;

    /// Platform primary + shift modifier.
    #[cfg(target_os = "macos")]
    pub const PRIMARY_SHIFT: Modifiers = Modifiers::LOGO_SHIFT;
    #[cfg(not(target_os = "macos"))]
    pub const PRIMARY_SHIFT: Modifiers = Modifiers::CTRL_SHIFT;

    /// Create the default keymap.
    ///
    /// Uses Ctrl on Windows/Linux, Cmd on macOS as the primary modifier.
    pub fn default_keymap() -> Self {
        let mut map = Self::new();
        let p = Self::PRIMARY;
        let ps = Self::PRIMARY_SHIFT;

        // Primary+C: copy selection
        map.bind(Keybinding::new(p, Key::Char('c')), Action::Copy);
        // Primary+V: paste clipboard
        map.bind(Keybinding::new(p, Key::Char('v')), Action::Paste);
        // Primary+A: select all
        map.bind(Keybinding::new(p, Key::Char('a')), Action::SelectAll);
        // Primary+T: new tab
        map.bind(Keybinding::new(p, Key::Char('t')), Action::NewTab);
        // Primary+Shift+T: split vertical
        map.bind(Keybinding::new(ps, Key::Char('t')), Action::SplitVertical);
        // Primary+W: close tab
        map.bind(Keybinding::new(p, Key::Char('w')), Action::CloseTab);
        // Primary+Q: quit
        map.bind(Keybinding::new(p, Key::Char('q')), Action::Quit);
        // Primary+F: find
        map.bind(Keybinding::new(p, Key::Char('f')), Action::Find);
        // Primary+E: toggle side panel
        map.bind(Keybinding::new(p, Key::Char('e')), Action::ToggleSidePanel);
        // Primary+1/2/3/4: direct pane focus
        map.bind(Keybinding::new(p, Key::Char('1')), Action::FocusPane1);
        map.bind(Keybinding::new(p, Key::Char('2')), Action::FocusPane2);
        map.bind(Keybinding::new(p, Key::Char('3')), Action::FocusPane3);
        map.bind(Keybinding::new(p, Key::Char('4')), Action::FocusPane4);
        // Primary+5/6/7/8/9: session focus
        map.bind(Keybinding::new(p, Key::Char('5')), Action::FocusPane5);
        map.bind(Keybinding::new(p, Key::Char('6')), Action::FocusPane6);
        map.bind(Keybinding::new(p, Key::Char('7')), Action::FocusPane7);
        map.bind(Keybinding::new(p, Key::Char('8')), Action::FocusPane8);
        map.bind(Keybinding::new(p, Key::Char('9')), Action::FocusPane9);
        // Primary+Enter: toggle mode
        map.bind(Keybinding::new(p, Key::Enter), Action::ToggleMode);
        // Primary+]: focus next
        map.bind(Keybinding::new(p, Key::Char(']')), Action::FocusNext);
        // Primary+[: focus prev
        map.bind(Keybinding::new(p, Key::Char('[')), Action::FocusPrev);
        // Primary+N: spawn new session
        map.bind(Keybinding::new(p, Key::Char('n')), Action::SpawnSession);
        // Primary+R: rename session
        map.bind(Keybinding::new(p, Key::Char('r')), Action::RenameSession);
        // Primary+B: toggle session list
        map.bind(
            Keybinding::new(p, Key::Char('b')),
            Action::ToggleSessionList,
        );
        // Primary+Shift+Up: side panel scroll up
        map.bind(Keybinding::new(ps, Key::Up), Action::SidePanelScrollUp);
        // Primary+Shift+Down: side panel scroll down
        map.bind(Keybinding::new(ps, Key::Down), Action::SidePanelScrollDown);
        // Primary+Shift+Enter: select file in side panel
        map.bind(Keybinding::new(ps, Key::Enter), Action::SidePanelSelect);
        // Primary+Shift+Escape: go back to file list in side panel
        map.bind(Keybinding::new(ps, Key::Escape), Action::SidePanelBack);
        // Primary+Shift+D: toggle unified/side-by-side diff mode
        map.bind(Keybinding::new(ps, Key::Char('d')), Action::ToggleDiffMode);
        // Primary+Tab: cycle focus region (SessionList ↔ Terminal ↔ SidePanel)
        map.bind(Keybinding::new(p, Key::Tab), Action::CycleFocusRegion);
        // Primary+S: swap session in focused pane (Split mode)
        map.bind(Keybinding::new(p, Key::Char('s')), Action::SwapSession);
        // Ctrl+Shift+C: copy selection (Linux convention)
        map.bind(
            Keybinding::new(Modifiers::CTRL_SHIFT, Key::Char('c')),
            Action::Copy,
        );
        // Ctrl+Shift+V: paste clipboard (Linux convention)
        map.bind(
            Keybinding::new(Modifiers::CTRL_SHIFT, Key::Char('v')),
            Action::Paste,
        );

        map
    }

    /// Add or overwrite a keybinding.
    pub fn bind(&mut self, keybinding: Keybinding, action: Action) {
        self.bindings.insert(keybinding, action);
    }

    /// Remove a keybinding.
    pub fn unbind(&mut self, keybinding: &Keybinding) {
        self.bindings.remove(keybinding);
    }

    /// Look up the action for a key combo.
    pub fn lookup(&self, keybinding: &Keybinding) -> Option<&Action> {
        self.bindings.get(keybinding)
    }

    /// Number of bindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether the keymap is empty.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Parse a keybinding string like "Cmd+T", "Cmd+Shift+T", "Ctrl+Enter".
    pub fn parse_binding(s: &str) -> Option<Keybinding> {
        let parts: Vec<&str> = s.split('+').collect();
        if parts.is_empty() {
            return None;
        }

        let mut modifiers = Modifiers::NONE;
        let key_str = parts.last()?;

        for &part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "cmd" | "logo" | "super" | "win" => modifiers.logo = true,
                "ctrl" | "control" => modifiers.ctrl = true,
                "alt" | "option" | "opt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                _ => return None,
            }
        }

        let key = match key_str.to_lowercase().as_str() {
            "enter" | "return" => Key::Enter,
            "escape" | "esc" => Key::Escape,
            "tab" => Key::Tab,
            "backspace" => Key::Backspace,
            "delete" | "del" => Key::Delete,
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            s if s.len() == 1 => Key::Char(s.chars().next()?),
            _ => return None,
        };

        Some(Keybinding::new(modifiers, key))
    }
}

impl Default for Keymap {
    fn default() -> Self {
        Self::default_keymap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keymap_has_bindings() {
        let keymap = Keymap::default_keymap();
        assert!(!keymap.is_empty());
        assert!(keymap.len() >= 31);
    }

    #[test]
    fn test_lookup_primary_t() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('t'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::NewTab));
    }

    #[test]
    fn test_lookup_primary_shift_t() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY_SHIFT, Key::Char('t'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::SplitVertical));
    }

    #[test]
    fn test_lookup_primary_enter() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Enter);
        assert_eq!(keymap.lookup(&binding), Some(&Action::ToggleMode));
    }

    #[test]
    fn test_lookup_missing() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Modifiers::NONE, Key::Char('z'));
        assert_eq!(keymap.lookup(&binding), None);
    }

    #[test]
    fn test_bind_and_unbind() {
        let mut keymap = Keymap::new();
        let binding = Keybinding::new(Modifiers::CTRL, Key::Char('x'));
        keymap.bind(binding.clone(), Action::ClosePane);
        assert_eq!(keymap.lookup(&binding), Some(&Action::ClosePane));

        keymap.unbind(&binding);
        assert_eq!(keymap.lookup(&binding), None);
    }

    #[test]
    fn test_parse_simple_binding() {
        let binding = Keymap::parse_binding("Cmd+T").unwrap();
        assert!(binding.modifiers.logo);
        assert_eq!(binding.key, Key::Char('t'));
    }

    #[test]
    fn test_parse_compound_binding() {
        let binding = Keymap::parse_binding("Cmd+Shift+T").unwrap();
        assert!(binding.modifiers.logo);
        assert!(binding.modifiers.shift);
        assert_eq!(binding.key, Key::Char('t'));
    }

    #[test]
    fn test_parse_enter() {
        let binding = Keymap::parse_binding("Cmd+Enter").unwrap();
        assert!(binding.modifiers.logo);
        assert_eq!(binding.key, Key::Enter);
    }

    #[test]
    fn test_parse_ctrl() {
        let binding = Keymap::parse_binding("Ctrl+W").unwrap();
        assert!(binding.modifiers.ctrl);
        assert_eq!(binding.key, Key::Char('w'));
    }

    #[test]
    fn test_lookup_primary_n_spawn_session() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('n'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::SpawnSession));
    }

    #[test]
    fn test_parse_invalid() {
        assert!(Keymap::parse_binding("").is_none());
        assert!(Keymap::parse_binding("InvalidMod+X").is_none());
    }

    #[test]
    fn test_override_binding() {
        let mut keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('t'));
        keymap.bind(binding.clone(), Action::ClosePane);
        assert_eq!(keymap.lookup(&binding), Some(&Action::ClosePane));
    }

    #[test]
    fn test_lookup_primary_c_copy() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('c'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::Copy));
    }

    #[test]
    fn test_lookup_primary_v_paste() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('v'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::Paste));
    }

    #[test]
    fn test_lookup_primary_a_select_all() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('a'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::SelectAll));
    }

    #[test]
    fn test_lookup_primary_w_close_tab() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('w'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::CloseTab));
    }

    #[test]
    fn test_lookup_primary_q_quit() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('q'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::Quit));
    }

    #[test]
    fn test_lookup_primary_f_find() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Keymap::PRIMARY, Key::Char('f'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::Find));
    }
}
