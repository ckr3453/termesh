//! Platform-specific utilities.

use std::path::PathBuf;

/// Get the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                let drive = std::env::var("HOMEDRIVE").ok()?;
                let path = std::env::var("HOMEPATH").ok()?;
                Some(PathBuf::from(format!("{drive}{path}")))
            })
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

/// Get the default shell command for the current platform.
pub fn default_shell() -> String {
    #[cfg(windows)]
    {
        // Prefer PowerShell if available, fall back to cmd.exe
        if which("pwsh.exe") {
            "pwsh.exe".to_string()
        } else if which("powershell.exe") {
            "powershell.exe".to_string()
        } else {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
        }
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

/// Get the termesh configuration directory.
///
/// - Windows: `%APPDATA%/termesh`
/// - macOS: `~/Library/Application Support/termesh`
/// - Linux: `~/.config/termesh`
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("termesh"))
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library/Application Support/termesh"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".config")))
            .map(|p| p.join("termesh"))
    }
}

/// Get the termesh data directory (for sockets, cache, etc.).
///
/// - Windows: `%LOCALAPPDATA%/termesh`
/// - macOS: `~/Library/Application Support/termesh`
/// - Linux: `~/.local/share/termesh`
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("termesh"))
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library/Application Support/termesh"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".local/share")))
            .map(|p| p.join("termesh"))
    }
}

/// Normalize a path for display (replace backslashes with forward slashes on Windows).
pub fn normalize_path_display(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Expand `~` in a path to the home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    } else if path == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

/// Ensure common tool directories are in PATH.
///
/// When launched as a macOS `.app` bundle from Finder, the process inherits
/// a minimal PATH (`/usr/bin:/bin:/usr/sbin:/sbin`). This function appends
/// well-known directories so that CLI tools like `claude`, `codex`, etc. are
/// discoverable by both termesh and spawned child processes.
///
/// # Safety
/// Must be called once at startup in `main()` before any threads are spawned.
#[cfg(target_os = "macos")]
pub fn ensure_path() {
    let current = std::env::var("PATH").unwrap_or_default();
    let home = home_dir();

    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Some(ref h) = home {
        candidates.push(h.join(".local/bin"));
        candidates.push(h.join(".cargo/bin"));
        candidates.push(h.join(".npm/bin"));
    }

    let current_dirs: std::collections::HashSet<&str> = current.split(':').collect();

    let extra: Vec<String> = candidates
        .into_iter()
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !current_dirs.contains(p.as_str()))
        .collect();

    if !extra.is_empty() {
        let extra_str = extra.join(":");
        let new_path = if current.is_empty() {
            extra_str
        } else {
            format!("{}:{}", current, extra_str)
        };
        // SAFETY: Called once at startup in main() before any threads are spawned.
        unsafe { std::env::set_var("PATH", new_path) };
    }
}

/// No-op on non-macOS platforms.
#[cfg(not(target_os = "macos"))]
pub fn ensure_path() {}

/// Check if a command is available in PATH.
pub fn which(cmd: &str) -> bool {
    let sep = if cfg!(windows) { ';' } else { ':' };
    std::env::var("PATH")
        .unwrap_or_default()
        .split(sep)
        .any(|dir| {
            let base = std::path::Path::new(dir).join(cmd);
            if base.exists() {
                return true;
            }
            if cfg!(windows) {
                base.with_extension("exe").exists()
                    || base.with_extension("cmd").exists()
                    || base.with_extension("bat").exists()
            } else {
                false
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_dir() {
        let home = home_dir();
        assert!(home.is_some(), "home directory should be detectable");
        assert!(home.unwrap().exists(), "home directory should exist");
    }

    #[test]
    fn test_default_shell() {
        let shell = default_shell();
        assert!(!shell.is_empty(), "default shell should not be empty");
    }

    #[test]
    fn test_config_dir() {
        let dir = config_dir();
        assert!(dir.is_some(), "config dir should be detectable");
    }

    #[test]
    fn test_data_dir() {
        let dir = data_dir();
        assert!(dir.is_some(), "data dir should be detectable");
    }

    #[test]
    fn test_expand_tilde_home() {
        let expanded = expand_tilde("~");
        assert!(expanded.exists(), "expanded ~ should exist");
    }

    #[test]
    fn test_expand_tilde_subpath() {
        let expanded = expand_tilde("~/some/path");
        let home = home_dir().unwrap();
        assert!(expanded.starts_with(&home));
    }

    #[test]
    fn test_expand_tilde_no_tilde() {
        let expanded = expand_tilde("/absolute/path");
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_normalize_path_display() {
        let path = std::path::Path::new("C:\\Users\\david\\projects");
        let normalized = normalize_path_display(path);
        assert!(!normalized.contains('\\'));
    }

    #[cfg(windows)]
    #[test]
    fn test_default_shell_windows() {
        let shell = default_shell();
        // Should be one of: pwsh.exe, powershell.exe, cmd.exe
        assert!(
            shell.contains("pwsh") || shell.contains("powershell") || shell.contains("cmd"),
            "unexpected shell: {shell}"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn test_default_shell_unix() {
        let shell = default_shell();
        // Should start with / or be a simple name
        assert!(
            shell.starts_with('/') || !shell.contains('\\'),
            "unexpected shell: {shell}"
        );
    }
}
