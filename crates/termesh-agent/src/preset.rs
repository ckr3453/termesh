//! Workspace preset: TOML-based configuration for agent sessions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A workspace preset defining how to arrange agents on startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePreset {
    /// Human-readable name for this preset.
    pub name: String,
    /// Default view mode ("focus" or "split"). Defaults to "focus".
    #[serde(default = "default_mode")]
    pub default_mode: String,
    /// Default side panel tab (if any).
    #[serde(default)]
    pub side_panel: Option<String>,
    /// Pane definitions.
    #[serde(default)]
    pub panes: Vec<PanePreset>,
}

fn default_mode() -> String {
    "focus".to_string()
}

/// A single pane within a workspace preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanePreset {
    /// Display label for the pane.
    pub label: String,
    /// Working directory for the shell/agent.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Command to run on startup (e.g., "claude", "bash").
    #[serde(default)]
    pub command: Option<String>,
    /// Description of this pane's role (displayed in UI).
    #[serde(default)]
    pub role: Option<String>,
    /// Environment variables to set.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Errors from loading preset configuration.
#[derive(Debug)]
pub enum PresetError {
    /// File I/O error.
    Io(std::io::Error),
    /// TOML parse error.
    Parse(toml::de::Error),
    /// Validation error.
    Validation(String),
}

impl std::fmt::Display for PresetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read preset file: {e}"),
            Self::Parse(e) => write!(f, "failed to parse preset TOML: {e}"),
            Self::Validation(msg) => write!(f, "invalid preset: {msg}"),
        }
    }
}

impl std::error::Error for PresetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::Validation(_) => None,
        }
    }
}

impl From<std::io::Error> for PresetError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for PresetError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

/// Load a workspace preset from a TOML file.
pub fn load_preset(path: &Path) -> Result<WorkspacePreset, PresetError> {
    let content = std::fs::read_to_string(path)?;
    load_preset_str(&content)
}

/// Parse a workspace preset from a TOML string.
pub fn load_preset_str(toml_str: &str) -> Result<WorkspacePreset, PresetError> {
    let preset: WorkspacePreset = toml::from_str(toml_str)?;
    validate_preset(&preset)?;
    Ok(preset)
}

/// Validate a preset for common issues.
fn validate_preset(preset: &WorkspacePreset) -> Result<(), PresetError> {
    if preset.name.trim().is_empty() {
        return Err(PresetError::Validation(
            "preset name cannot be empty".into(),
        ));
    }
    if !["focus", "split"].contains(&preset.default_mode.as_str()) {
        return Err(PresetError::Validation(format!(
            "default_mode must be 'focus' or 'split', got '{}'",
            preset.default_mode
        )));
    }
    if preset.panes.is_empty() {
        return Err(PresetError::Validation(
            "preset must have at least one pane".into(),
        ));
    }
    if preset.panes.len() > 16 {
        return Err(PresetError::Validation(
            "preset cannot have more than 16 panes".into(),
        ));
    }
    for (i, pane) in preset.panes.iter().enumerate() {
        if pane.label.trim().is_empty() {
            return Err(PresetError::Validation(format!(
                "pane {i} label cannot be empty"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_basic_preset() {
        let toml = r#"
name = "Full Stack Dev"

[[panes]]
label = "Frontend"
cwd = "~/projects/app/frontend"
command = "claude"

[[panes]]
label = "Backend"
cwd = "~/projects/app/backend"
command = "claude"

[[panes]]
label = "Shell"
command = "bash"
"#;
        let preset = load_preset_str(toml).unwrap();
        assert_eq!(preset.name, "Full Stack Dev");
        assert_eq!(preset.panes.len(), 3);
        assert_eq!(preset.panes[0].label, "Frontend");
        assert_eq!(
            preset.panes[0].cwd.as_deref(),
            Some("~/projects/app/frontend")
        );
        assert_eq!(preset.panes[0].command.as_deref(), Some("claude"));
        assert_eq!(preset.panes[2].label, "Shell");
    }

    #[test]
    fn test_preset_with_env() {
        let toml = r#"
name = "Test"

[[panes]]
label = "Agent"
command = "claude"

[panes.env]
CUSTOM_VAR = "some-value"
MODEL = "opus"
"#;
        let preset = load_preset_str(toml).unwrap();
        let env = &preset.panes[0].env;
        assert_eq!(env.get("MODEL"), Some(&"opus".to_string()));
    }

    #[test]
    fn test_preset_empty_name() {
        let toml = r#"
name = ""

[[panes]]
label = "Shell"
"#;
        let result = load_preset_str(toml);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("name cannot be empty"));
    }

    #[test]
    fn test_preset_no_panes() {
        let toml = r#"
name = "Empty"
"#;
        let result = load_preset_str(toml);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one pane"));
    }

    #[test]
    fn test_preset_empty_pane_label() {
        let toml = r#"
name = "Bad"

[[panes]]
label = ""
"#;
        let result = load_preset_str(toml);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("label cannot be empty"));
    }

    #[test]
    fn test_preset_too_many_panes() {
        let mut toml = String::from("name = \"Big\"\n");
        for i in 0..17 {
            toml.push_str(&format!("\n[[panes]]\nlabel = \"Pane {i}\"\n"));
        }
        let result = load_preset_str(&toml);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("more than 16 panes"));
    }

    #[test]
    fn test_preset_minimal() {
        let toml = r#"
name = "Minimal"

[[panes]]
label = "Main"
"#;
        let preset = load_preset_str(toml).unwrap();
        assert_eq!(preset.panes[0].cwd, None);
        assert_eq!(preset.panes[0].command, None);
        assert!(preset.panes[0].env.is_empty());
    }

    #[test]
    fn test_preset_invalid_toml() {
        let result = load_preset_str("not valid {{ toml");
        assert!(result.is_err());
    }
}
