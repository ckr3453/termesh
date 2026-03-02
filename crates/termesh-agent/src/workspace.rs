//! Workspace loader: discovers and loads workspace presets from disk.

use crate::preset::{load_preset, PresetError, WorkspacePreset};
use std::path::{Path, PathBuf};

/// Default workspace config directory relative to user config.
const WORKSPACE_DIR: &str = "workspaces";

/// Discovers and manages workspace presets.
pub struct WorkspaceLoader {
    /// Base directory for workspace TOML files.
    config_dir: PathBuf,
}

impl WorkspaceLoader {
    /// Create a loader pointing to `~/.config/termesh/workspaces/`.
    pub fn default_dir() -> Option<Self> {
        let config = dirs_path()?;
        Some(Self { config_dir: config })
    }

    /// Create a loader with a custom directory.
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// List available workspace names (filename stems).
    pub fn list(&self) -> Vec<String> {
        let mut names = Vec::new();
        let entries = match std::fs::read_dir(&self.config_dir) {
            Ok(e) => e,
            Err(_) => return names,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        names
    }

    /// Load a workspace by name (without .toml extension).
    pub fn load(&self, name: &str) -> Result<WorkspacePreset, PresetError> {
        let path = self.config_dir.join(format!("{name}.toml"));
        if !path.exists() {
            return Err(PresetError::Validation(format!(
                "workspace '{name}' not found"
            )));
        }
        load_preset(&path)
    }

    /// Load a workspace from a project-local `.termesh.toml` file.
    pub fn load_local(project_root: &Path) -> Result<WorkspacePreset, PresetError> {
        let path = project_root.join(".termesh.toml");
        if !path.exists() {
            return Err(PresetError::Validation(format!(
                "no .termesh.toml found at {}",
                path.display()
            )));
        }
        load_preset(&path)
    }

    /// Get the config directory path.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Ensure the config directory exists.
    pub fn ensure_dir(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.config_dir)
    }
}

/// Get the default workspace config directory.
fn dirs_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("termesh").join(WORKSPACE_DIR))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .map(|p| PathBuf::from(p).join(".config/termesh").join(WORKSPACE_DIR))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn setup_test_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tid = std::thread::current().id();
        let dir = std::env::temp_dir().join(format!("termesh_ws_test_{tid:?}_{id}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_preset(dir: &Path, name: &str, content: &str) {
        let path = dir.join(format!("{name}.toml"));
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_list_empty() {
        let dir = setup_test_dir();
        let loader = WorkspaceLoader::new(dir.clone());
        assert!(loader.list().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_list_workspaces() {
        let dir = setup_test_dir();
        let preset = r#"
name = "Test"
[[panes]]
label = "Shell"
"#;
        write_preset(&dir, "alpha", preset);
        write_preset(&dir, "beta", preset);

        let loader = WorkspaceLoader::new(dir.clone());
        let names = loader.list();
        assert_eq!(names, vec!["alpha", "beta"]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_by_name() {
        let dir = setup_test_dir();
        write_preset(
            &dir,
            "mywork",
            r#"
name = "My Work"
default_mode = "split"
[[panes]]
label = "Agent"
command = "claude"
role = "Backend developer"
"#,
        );

        let loader = WorkspaceLoader::new(dir.clone());
        let preset = loader.load("mywork").unwrap();
        assert_eq!(preset.name, "My Work");
        assert_eq!(preset.default_mode, "split");
        assert_eq!(preset.panes[0].role.as_deref(), Some("Backend developer"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = setup_test_dir();
        let loader = WorkspaceLoader::new(dir.clone());
        let result = loader.load("nope");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_local() {
        let dir = setup_test_dir();
        let content = r#"
name = "Local Project"
[[panes]]
label = "Dev"
"#;
        std::fs::write(dir.join(".termesh.toml"), content).unwrap();

        let preset = WorkspaceLoader::load_local(&dir).unwrap();
        assert_eq!(preset.name, "Local Project");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_local_missing() {
        let dir = setup_test_dir();
        let result = WorkspaceLoader::load_local(&dir);
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_default_mode_defaults_to_focus() {
        let dir = setup_test_dir();
        write_preset(
            &dir,
            "nomode",
            r#"
name = "NoMode"
[[panes]]
label = "Shell"
"#,
        );
        let loader = WorkspaceLoader::new(dir.clone());
        let preset = loader.load("nomode").unwrap();
        assert_eq!(preset.default_mode, "focus");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_invalid_default_mode() {
        let dir = setup_test_dir();
        write_preset(
            &dir,
            "badmode",
            r#"
name = "Bad"
default_mode = "invalid"
[[panes]]
label = "Shell"
"#,
        );
        let loader = WorkspaceLoader::new(dir.clone());
        let result = loader.load("badmode");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("default_mode"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_ensure_dir() {
        let dir = std::env::temp_dir().join("termesh_ensure_test");
        let _ = std::fs::remove_dir_all(&dir);
        let loader = WorkspaceLoader::new(dir.clone());
        loader.ensure_dir().unwrap();
        assert!(dir.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_list_ignores_non_toml() {
        let dir = setup_test_dir();
        std::fs::write(dir.join("readme.md"), "# hi").unwrap();
        write_preset(
            &dir,
            "valid",
            r#"
name = "V"
[[panes]]
label = "S"
"#,
        );

        let loader = WorkspaceLoader::new(dir.clone());
        let names = loader.list();
        assert_eq!(names, vec!["valid"]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
