//! Project management: discovery, naming, and recent project tracking.

use crate::types::ProjectId;
use std::path::{Path, PathBuf};

/// A discovered project, identified by its root path.
#[derive(Debug, Clone)]
pub struct Project {
    /// Deterministic ID derived from the canonical path.
    pub id: ProjectId,
    /// Display name (e.g. `"termesh"`, `"my-fork (termesh)"`).
    pub name: String,
    /// Canonical project root path (git root or cwd).
    pub path: PathBuf,
    /// Git repository root, if the project is inside a git repo.
    pub git_root: Option<PathBuf>,
}

impl Project {
    /// Discover a project from a working directory.
    ///
    /// 1. Canonicalize the path (fall back to original on failure).
    /// 2. Search upward for a `.git` directory.
    /// 3. If found, project root = git root; otherwise project root = cwd.
    /// 4. Name follows the `build_group_label` convention.
    pub fn from_path(cwd: PathBuf) -> Self {
        let canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
        let git_root = find_git_root(&canonical);
        let project_path = git_root.as_deref().unwrap_or(&canonical).to_path_buf();
        let id = ProjectId::from_path(&project_path);

        let folder_name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let git_name = git_root
            .as_ref()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = build_group_label(&folder_name, &git_name);

        Self {
            id,
            name,
            path: project_path,
            git_root,
        }
    }
}

/// Search upward from `start` for a directory containing `.git`.
fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = start;
    loop {
        if current.join(".git").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

/// Build a display label from folder and git names.
///
/// - Both same → `"termesh"`
/// - Different → `"my-fork (termesh)"`
/// - Folder only → `"Documents"`
/// - Git only → `"termesh"`
/// - Neither → `""`
fn build_group_label(folder_name: &str, git_name: &str) -> String {
    match (folder_name.is_empty(), git_name.is_empty()) {
        (true, true) => String::new(),
        (true, false) => git_name.to_string(),
        (false, true) => folder_name.to_string(),
        (false, false) => {
            if folder_name == git_name {
                folder_name.to_string()
            } else {
                format!("{folder_name} ({git_name})")
            }
        }
    }
}

/// Serializable entry for a recent project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RecentEntry {
    path: PathBuf,
    name: String,
}

/// Maximum number of recent projects to keep.
const MAX_RECENT: usize = 20;

/// Tracks recently opened projects, persisted to disk as JSON.
#[derive(Debug, Clone)]
pub struct RecentProjects {
    projects: Vec<RecentEntry>,
    file_path: PathBuf,
}

impl RecentProjects {
    /// Load recent projects from the default data directory.
    ///
    /// Returns an empty list if the file doesn't exist or can't be parsed.
    pub fn load() -> Self {
        let file_path = Self::default_path();
        let projects = match std::fs::read_to_string(&file_path) {
            Ok(s) => serde_json::from_str::<Vec<RecentEntry>>(&s).unwrap_or_else(|e| {
                log::warn!("failed to parse {}: {e}", file_path.display());
                Vec::new()
            }),
            Err(_) => Vec::new(), // file doesn't exist yet — not an error
        };
        Self {
            projects,
            file_path,
        }
    }

    /// Create an instance backed by a custom file path (for testing).
    pub fn with_path(file_path: PathBuf) -> Self {
        let projects = match std::fs::read_to_string(&file_path) {
            Ok(s) => serde_json::from_str::<Vec<RecentEntry>>(&s).unwrap_or_else(|e| {
                log::warn!("failed to parse {}: {e}", file_path.display());
                Vec::new()
            }),
            Err(_) => Vec::new(),
        };
        Self {
            projects,
            file_path,
        }
    }

    /// Save the recent projects list to disk.
    pub fn save(&self) {
        if let Some(parent) = self.file_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!("failed to create data dir {}: {e}", parent.display());
                return;
            }
        }
        match serde_json::to_string_pretty(&self.projects) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.file_path, json) {
                    log::warn!("failed to save {}: {e}", self.file_path.display());
                }
            }
            Err(e) => {
                log::warn!("failed to serialize recent projects: {e}");
            }
        }
    }

    /// Move a project to the front of the list (most recent).
    ///
    /// De-duplicates by path, and truncates to [`MAX_RECENT`] entries.
    pub fn touch(&mut self, project: &Project) {
        self.projects.retain(|e| e.path != project.path);
        self.projects.insert(
            0,
            RecentEntry {
                path: project.path.clone(),
                name: project.name.clone(),
            },
        );
        self.projects.truncate(MAX_RECENT);
    }

    /// Return all tracked project paths (most recent first).
    pub fn paths(&self) -> Vec<PathBuf> {
        self.projects.iter().map(|e| e.path.clone()).collect()
    }

    fn default_path() -> PathBuf {
        crate::platform::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("recent_projects.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── Project tests ─────────────────────────────────────────────────

    #[test]
    fn test_project_from_path_with_git() {
        let tmp = TempDir::new().unwrap();
        let git_root = tmp.path().join("my-repo");
        std::fs::create_dir_all(git_root.join(".git")).unwrap();
        let sub = git_root.join("packages/frontend");
        std::fs::create_dir_all(&sub).unwrap();

        let project = Project::from_path(sub);
        assert_eq!(project.path, git_root.canonicalize().unwrap());
        assert!(project.git_root.is_some());
        assert_eq!(project.name, "frontend (my-repo)");
    }

    #[test]
    fn test_project_from_path_no_git() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("Documents");
        std::fs::create_dir_all(&dir).unwrap();

        let project = Project::from_path(dir.clone());
        assert_eq!(project.path, dir.canonicalize().unwrap());
        assert!(project.git_root.is_none());
        assert_eq!(project.name, "Documents");
    }

    #[test]
    fn test_project_from_path_git_root_equals_cwd() {
        let tmp = TempDir::new().unwrap();
        let git_root = tmp.path().join("termesh");
        std::fs::create_dir_all(git_root.join(".git")).unwrap();

        let project = Project::from_path(git_root.clone());
        assert_eq!(project.name, "termesh");
    }

    // ── RecentProjects tests ──────────────────────────────────────────

    fn make_project(path: &str, name: &str) -> Project {
        Project {
            id: ProjectId::from_path(Path::new(path)),
            name: name.to_string(),
            path: PathBuf::from(path),
            git_root: None,
        }
    }

    #[test]
    fn test_recent_projects_touch_adds() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("recent.json");
        let mut recent = RecentProjects::with_path(file);

        let p = make_project("/a", "A");
        recent.touch(&p);
        assert_eq!(recent.paths(), vec![PathBuf::from("/a")]);
    }

    #[test]
    fn test_recent_projects_touch_deduplicates() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("recent.json");
        let mut recent = RecentProjects::with_path(file);

        let p1 = make_project("/a", "A");
        let p2 = make_project("/b", "B");
        recent.touch(&p1);
        recent.touch(&p2);
        recent.touch(&p1); // move /a back to front

        assert_eq!(
            recent.paths(),
            vec![PathBuf::from("/a"), PathBuf::from("/b")]
        );
    }

    #[test]
    fn test_recent_projects_max_limit() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("recent.json");
        let mut recent = RecentProjects::with_path(file);

        for i in 0..25 {
            let p = make_project(&format!("/p{i}"), &format!("P{i}"));
            recent.touch(&p);
        }
        assert_eq!(recent.paths().len(), MAX_RECENT);
        // Most recent is /p24
        assert_eq!(recent.paths()[0], PathBuf::from("/p24"));
    }

    #[test]
    fn test_recent_projects_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("recent.json");

        {
            let mut recent = RecentProjects::with_path(file.clone());
            recent.touch(&make_project("/x", "X"));
            recent.touch(&make_project("/y", "Y"));
            recent.save();
        }

        let loaded = RecentProjects::with_path(file);
        assert_eq!(
            loaded.paths(),
            vec![PathBuf::from("/y"), PathBuf::from("/x")]
        );
    }

    // ── build_group_label tests ───────────────────────────────────────

    #[test]
    fn test_build_group_label_same() {
        assert_eq!(build_group_label("termesh", "termesh"), "termesh");
    }

    #[test]
    fn test_build_group_label_different() {
        assert_eq!(build_group_label("my-fork", "termesh"), "my-fork (termesh)");
    }

    #[test]
    fn test_build_group_label_folder_only() {
        assert_eq!(build_group_label("Documents", ""), "Documents");
    }

    #[test]
    fn test_build_group_label_git_only() {
        assert_eq!(build_group_label("", "termesh"), "termesh");
    }

    #[test]
    fn test_build_group_label_both_empty() {
        assert_eq!(build_group_label("", ""), "");
    }
}
