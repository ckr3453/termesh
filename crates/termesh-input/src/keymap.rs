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

    /// Create the default keymap.
    ///
    /// Uses Logo (Cmd on macOS, Win on Windows) as the primary modifier.
    pub fn default_keymap() -> Self {
        let mut map = Self::new();

        // Cmd+T: split horizontal
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char('t')),
            Action::SplitHorizontal,
        );
        // Cmd+Shift+T: split vertical
        map.bind(
            Keybinding::new(Modifiers::LOGO_SHIFT, Key::Char('t')),
            Action::SplitVertical,
        );
        // Cmd+W: close pane
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char('w')),
            Action::ClosePane,
        );
        // Cmd+E: toggle side panel
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char('e')),
            Action::ToggleSidePanel,
        );
        // Cmd+H/J/K/L: vim-like navigation
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char('h')),
            Action::NavigateLeft,
        );
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char('j')),
            Action::NavigateDown,
        );
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char('k')),
            Action::NavigateUp,
        );
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char('l')),
            Action::NavigateRight,
        );
        // Cmd+Enter: toggle mode
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Enter),
            Action::ToggleMode,
        );
        // Cmd+]: focus next
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char(']')),
            Action::FocusNext,
        );
        // Cmd+[: focus prev
        map.bind(
            Keybinding::new(Modifiers::LOGO, Key::Char('[')),
            Action::FocusPrev,
        );
        // Ctrl+Shift+C: copy selection
        map.bind(
            Keybinding::new(Modifiers::CTRL_SHIFT, Key::Char('c')),
            Action::Copy,
        );
        // Ctrl+Shift+V: paste clipboard
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
        assert!(keymap.len() >= 11);
    }

    #[test]
    fn test_lookup_cmd_t() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Modifiers::LOGO, Key::Char('t'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::SplitHorizontal));
    }

    #[test]
    fn test_lookup_cmd_shift_t() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Modifiers::LOGO_SHIFT, Key::Char('t'));
        assert_eq!(keymap.lookup(&binding), Some(&Action::SplitVertical));
    }

    #[test]
    fn test_lookup_cmd_enter() {
        let keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Modifiers::LOGO, Key::Enter);
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
    fn test_parse_invalid() {
        assert!(Keymap::parse_binding("").is_none());
        assert!(Keymap::parse_binding("InvalidMod+X").is_none());
    }

    #[test]
    fn test_override_binding() {
        let mut keymap = Keymap::default_keymap();
        let binding = Keybinding::new(Modifiers::LOGO, Key::Char('t'));
        keymap.bind(binding.clone(), Action::ClosePane);
        assert_eq!(keymap.lookup(&binding), Some(&Action::ClosePane));
    }
}
