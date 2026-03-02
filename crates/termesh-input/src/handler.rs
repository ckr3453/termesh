//! Input handler: translates key events into actions using the keymap.

use crate::action::Action;
use crate::keymap::{Key, Keybinding, Keymap, Modifiers};
use serde::Deserialize;
use std::path::Path;

/// A TOML keybinding entry.
#[derive(Debug, Clone, Deserialize)]
struct BindingEntry {
    /// Key combo string like "Cmd+T", "Cmd+Shift+T".
    keys: String,
    /// Action name matching the Action enum variant.
    action: String,
}

/// TOML config file structure.
#[derive(Debug, Clone, Deserialize)]
struct KeybindingsConfig {
    #[serde(default)]
    bindings: Vec<BindingEntry>,
}

/// Input handler that processes key events and dispatches actions.
#[derive(Debug, Clone)]
pub struct InputHandler {
    keymap: Keymap,
}

impl InputHandler {
    /// Create an input handler with the default keymap.
    pub fn new() -> Self {
        Self {
            keymap: Keymap::default_keymap(),
        }
    }

    /// Create an input handler with a custom keymap.
    pub fn with_keymap(keymap: Keymap) -> Self {
        Self { keymap }
    }

    /// Load custom keybindings from a TOML file, merging with defaults.
    ///
    /// Unknown keys or actions are skipped with a warning log.
    pub fn load_config(&mut self, path: &Path) -> Result<usize, InputConfigError> {
        let content = std::fs::read_to_string(path)?;
        self.load_config_str(&content)
    }

    /// Load custom keybindings from a TOML string, merging with defaults.
    ///
    /// Returns the number of bindings successfully applied.
    pub fn load_config_str(&mut self, toml_str: &str) -> Result<usize, InputConfigError> {
        let config: KeybindingsConfig = toml::from_str(toml_str)?;

        let mut applied = 0;
        for entry in &config.bindings {
            let binding = match Keymap::parse_binding(&entry.keys) {
                Some(b) => b,
                None => {
                    log::warn!("Skipping unknown key combo: {}", entry.keys);
                    continue;
                }
            };
            let action = match parse_action(&entry.action) {
                Some(a) => a,
                None => {
                    log::warn!("Skipping unknown action: {}", entry.action);
                    continue;
                }
            };
            self.keymap.bind(binding, action);
            applied += 1;
        }

        Ok(applied)
    }

    /// Handle a key event: look up the action for the given key combo.
    pub fn handle_key(&self, modifiers: Modifiers, key: Key) -> Option<Action> {
        let binding = Keybinding::new(modifiers, key);
        self.keymap.lookup(&binding).cloned()
    }

    /// Get a reference to the underlying keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Get a mutable reference to the underlying keymap.
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from loading input configuration.
#[derive(Debug)]
pub enum InputConfigError {
    /// File I/O error.
    Io(std::io::Error),
    /// TOML parse error.
    Parse(toml::de::Error),
}

impl std::fmt::Display for InputConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read keybindings file: {e}"),
            Self::Parse(e) => write!(f, "failed to parse keybindings TOML: {e}"),
        }
    }
}

impl std::error::Error for InputConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for InputConfigError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for InputConfigError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

/// Parse an action name string into an Action enum variant.
///
/// Matching is case-insensitive to be forgiving in user-facing TOML configs.
fn parse_action(s: &str) -> Option<Action> {
    match s.to_lowercase().as_str() {
        "splithorizontal" => Some(Action::SplitHorizontal),
        "splitvertical" => Some(Action::SplitVertical),
        "closepane" => Some(Action::ClosePane),
        "togglesidepanel" => Some(Action::ToggleSidePanel),
        "focuspane1" => Some(Action::FocusPane1),
        "focuspane2" => Some(Action::FocusPane2),
        "focuspane3" => Some(Action::FocusPane3),
        "focuspane4" => Some(Action::FocusPane4),
        "focuspane5" => Some(Action::FocusPane5),
        "focuspane6" => Some(Action::FocusPane6),
        "focuspane7" => Some(Action::FocusPane7),
        "focuspane8" => Some(Action::FocusPane8),
        "focuspane9" => Some(Action::FocusPane9),
        "togglemode" => Some(Action::ToggleMode),
        "focusnext" => Some(Action::FocusNext),
        "focusprev" => Some(Action::FocusPrev),
        "spawnsession" => Some(Action::SpawnSession),
        "renamesession" => Some(Action::RenameSession),
        "copy" => Some(Action::Copy),
        "paste" => Some(Action::Paste),
        "togglesessionlist" => Some(Action::ToggleSessionList),
        "sidepanelscrollup" => Some(Action::SidePanelScrollUp),
        "sidepanelscrolldown" => Some(Action::SidePanelScrollDown),
        "sidepanelselect" => Some(Action::SidePanelSelect),
        "sidepanelback" => Some(Action::SidePanelBack),
        "togglediffmode" => Some(Action::ToggleDiffMode),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_handler_has_bindings() {
        let handler = InputHandler::new();
        assert!(!handler.keymap().is_empty());
    }

    #[test]
    fn test_handle_key_primary_t() {
        let handler = InputHandler::new();
        let action = handler.handle_key(Keymap::PRIMARY, Key::Char('t'));
        assert_eq!(action, Some(Action::SplitHorizontal));
    }

    #[test]
    fn test_handle_key_primary_w() {
        let handler = InputHandler::new();
        let action = handler.handle_key(Keymap::PRIMARY, Key::Char('w'));
        assert_eq!(action, Some(Action::ClosePane));
    }

    #[test]
    fn test_handle_key_no_match() {
        let handler = InputHandler::new();
        let action = handler.handle_key(Modifiers::NONE, Key::Char('z'));
        assert_eq!(action, None);
    }

    #[test]
    fn test_load_config_str() {
        let toml = r#"
[[bindings]]
keys = "Ctrl+X"
action = "ClosePane"

[[bindings]]
keys = "Ctrl+N"
action = "SplitHorizontal"
"#;
        let mut handler = InputHandler::new();
        let applied = handler.load_config_str(toml).unwrap();
        assert_eq!(applied, 2);

        // Custom binding should work
        let action = handler.handle_key(Modifiers::CTRL, Key::Char('x'));
        assert_eq!(action, Some(Action::ClosePane));
    }

    #[test]
    fn test_load_config_skips_unknown() {
        let toml = r#"
[[bindings]]
keys = "Ctrl+X"
action = "ClosePane"

[[bindings]]
keys = "BadMod+Z"
action = "ClosePane"

[[bindings]]
keys = "Ctrl+Y"
action = "NonExistentAction"
"#;
        let mut handler = InputHandler::new();
        let applied = handler.load_config_str(toml).unwrap();
        assert_eq!(applied, 1); // Only the first valid binding
    }

    #[test]
    fn test_load_config_preserves_defaults() {
        let toml = r#"
[[bindings]]
keys = "Ctrl+X"
action = "ClosePane"
"#;
        let mut handler = InputHandler::new();
        handler.load_config_str(toml).unwrap();

        // Default binding should still work
        let action = handler.handle_key(Keymap::PRIMARY, Key::Char('t'));
        assert_eq!(action, Some(Action::SplitHorizontal));
    }

    #[test]
    fn test_load_config_overrides_default() {
        let toml = r#"
[[bindings]]
keys = "Cmd+T"
action = "ClosePane"
"#;
        let mut handler = InputHandler::new();
        handler.load_config_str(toml).unwrap();

        // Cmd+T should now be ClosePane instead of SplitHorizontal
        let action = handler.handle_key(Modifiers::LOGO, Key::Char('t'));
        assert_eq!(action, Some(Action::ClosePane));
    }

    #[test]
    fn test_load_config_invalid_toml() {
        let mut handler = InputHandler::new();
        let result = handler.load_config_str("not valid {{ toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_empty() {
        let mut handler = InputHandler::new();
        let applied = handler.load_config_str("").unwrap();
        assert_eq!(applied, 0);
    }

    #[test]
    fn test_parse_action_all_variants() {
        assert_eq!(
            parse_action("SplitHorizontal"),
            Some(Action::SplitHorizontal)
        );
        assert_eq!(parse_action("SplitVertical"), Some(Action::SplitVertical));
        assert_eq!(parse_action("ClosePane"), Some(Action::ClosePane));
        assert_eq!(
            parse_action("ToggleSidePanel"),
            Some(Action::ToggleSidePanel)
        );
        assert_eq!(parse_action("FocusPane1"), Some(Action::FocusPane1));
        assert_eq!(parse_action("FocusPane2"), Some(Action::FocusPane2));
        assert_eq!(parse_action("FocusPane3"), Some(Action::FocusPane3));
        assert_eq!(parse_action("FocusPane4"), Some(Action::FocusPane4));
        assert_eq!(parse_action("FocusPane5"), Some(Action::FocusPane5));
        assert_eq!(parse_action("FocusPane6"), Some(Action::FocusPane6));
        assert_eq!(parse_action("FocusPane7"), Some(Action::FocusPane7));
        assert_eq!(parse_action("FocusPane8"), Some(Action::FocusPane8));
        assert_eq!(parse_action("FocusPane9"), Some(Action::FocusPane9));
        assert_eq!(parse_action("ToggleMode"), Some(Action::ToggleMode));
        assert_eq!(parse_action("FocusNext"), Some(Action::FocusNext));
        assert_eq!(parse_action("FocusPrev"), Some(Action::FocusPrev));
        assert_eq!(parse_action("SpawnSession"), Some(Action::SpawnSession));
        assert_eq!(parse_action("RenameSession"), Some(Action::RenameSession));
        assert_eq!(parse_action("Copy"), Some(Action::Copy));
        assert_eq!(parse_action("Paste"), Some(Action::Paste));
        assert_eq!(
            parse_action("ToggleSessionList"),
            Some(Action::ToggleSessionList)
        );
        assert_eq!(
            parse_action("SidePanelScrollUp"),
            Some(Action::SidePanelScrollUp)
        );
        assert_eq!(
            parse_action("SidePanelScrollDown"),
            Some(Action::SidePanelScrollDown)
        );
        assert_eq!(
            parse_action("SidePanelSelect"),
            Some(Action::SidePanelSelect)
        );
        assert_eq!(parse_action("SidePanelBack"), Some(Action::SidePanelBack));
        assert_eq!(parse_action("ToggleDiffMode"), Some(Action::ToggleDiffMode));
        assert_eq!(parse_action("Unknown"), None);
    }

    #[test]
    fn test_parse_action_case_insensitive() {
        assert_eq!(parse_action("closepane"), Some(Action::ClosePane));
        assert_eq!(parse_action("CLOSEPANE"), Some(Action::ClosePane));
        assert_eq!(parse_action("closePane"), Some(Action::ClosePane));
        assert_eq!(parse_action("toggleMode"), Some(Action::ToggleMode));
    }

    #[test]
    fn test_with_custom_keymap() {
        let mut keymap = Keymap::new();
        keymap.bind(
            Keybinding::new(Modifiers::CTRL, Key::Char('q')),
            Action::ClosePane,
        );
        let handler = InputHandler::with_keymap(keymap);
        let action = handler.handle_key(Modifiers::CTRL, Key::Char('q'));
        assert_eq!(action, Some(Action::ClosePane));
    }
}
