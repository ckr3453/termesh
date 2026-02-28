//! Local license token storage.
//!
//! Stores authentication tokens in a JSON file within the termesh data
//! directory. The file is created with restrictive permissions where
//! possible.

use crate::auth::TokenPair;
use crate::error::AuthError;
use crate::platform;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const TOKEN_FILE: &str = "credentials.json";

/// Stored credentials including tokens and optional metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub access_token: String,
    pub refresh_token: String,
    /// User email (for display purposes only).
    #[serde(default)]
    pub email: Option<String>,
}

impl From<TokenPair> for StoredCredentials {
    fn from(pair: TokenPair) -> Self {
        Self {
            access_token: pair.access_token,
            refresh_token: pair.refresh_token,
            email: None,
        }
    }
}

impl StoredCredentials {
    /// Set the email on this credentials.
    pub fn with_email(mut self, email: &str) -> Self {
        self.email = Some(email.to_string());
        self
    }

    /// Convert back to a TokenPair.
    pub fn to_token_pair(&self) -> TokenPair {
        TokenPair {
            access_token: self.access_token.clone(),
            refresh_token: self.refresh_token.clone(),
        }
    }
}

/// File-based token storage in the termesh data directory.
pub struct LicenseStore {
    path: PathBuf,
}

impl LicenseStore {
    /// Create a store using the default data directory.
    pub fn default_store() -> Result<Self, AuthError> {
        let data_dir = platform::data_dir().ok_or_else(|| AuthError::Network {
            reason: "cannot determine data directory".to_string(),
        })?;
        Ok(Self::new(data_dir))
    }

    /// Create a store at a custom directory path.
    pub fn new(dir: PathBuf) -> Self {
        Self {
            path: dir.join(TOKEN_FILE),
        }
    }

    /// Save tokens to disk.
    pub fn save(&self, creds: &StoredCredentials) -> Result<(), AuthError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AuthError::Network {
                reason: format!("failed to create directory: {e}"),
            })?;
        }

        let json = serde_json::to_string_pretty(creds).map_err(|e| AuthError::InvalidToken {
            reason: format!("failed to serialize credentials: {e}"),
        })?;

        std::fs::write(&self.path, json).map_err(|e| AuthError::Network {
            reason: format!("failed to write credentials: {e}"),
        })?;

        // Best-effort: set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&self.path, perms);
        }

        Ok(())
    }

    /// Load tokens from disk.
    pub fn load(&self) -> Result<StoredCredentials, AuthError> {
        let content = std::fs::read_to_string(&self.path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AuthError::AuthFailed {
                    reason: "no stored credentials".to_string(),
                }
            } else {
                AuthError::Network {
                    reason: format!("failed to read credentials: {e}"),
                }
            }
        })?;

        serde_json::from_str(&content).map_err(|e| AuthError::InvalidToken {
            reason: format!("failed to parse credentials: {e}"),
        })
    }

    /// Delete stored tokens.
    pub fn delete(&self) -> Result<(), AuthError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AuthError::Network {
                reason: format!("failed to delete credentials: {e}"),
            }),
        }
    }

    /// Check if credentials exist on disk.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Get the path to the credentials file.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn temp_store() -> (LicenseStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = LicenseStore::new(dir.path().to_path_buf());
        (store, dir)
    }

    fn sample_creds() -> StoredCredentials {
        StoredCredentials {
            access_token: "access123".to_string(),
            refresh_token: "refresh456".to_string(),
            email: Some("user@example.com".to_string()),
        }
    }

    #[test]
    fn test_save_and_load() {
        let (store, _dir) = temp_store();
        let creds = sample_creds();

        store.save(&creds).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded.access_token, "access123");
        assert_eq!(loaded.refresh_token, "refresh456");
        assert_eq!(loaded.email.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn test_load_nonexistent() {
        let (store, _dir) = temp_store();
        let result = store.load();
        assert!(matches!(result, Err(AuthError::AuthFailed { .. })));
    }

    #[test]
    fn test_delete() {
        let (store, _dir) = temp_store();
        store.save(&sample_creds()).unwrap();
        assert!(store.exists());

        store.delete().unwrap();
        assert!(!store.exists());
    }

    #[test]
    fn test_delete_nonexistent() {
        let (store, _dir) = temp_store();
        // Should not error
        store.delete().unwrap();
    }

    #[test]
    fn test_exists() {
        let (store, _dir) = temp_store();
        assert!(!store.exists());
        store.save(&sample_creds()).unwrap();
        assert!(store.exists());
    }

    #[test]
    fn test_from_token_pair() {
        let pair = TokenPair {
            access_token: "a".to_string(),
            refresh_token: "r".to_string(),
        };
        let creds = StoredCredentials::from(pair).with_email("test@test.com");
        assert_eq!(creds.access_token, "a");
        assert_eq!(creds.email.as_deref(), Some("test@test.com"));
    }

    #[test]
    fn test_to_token_pair() {
        let creds = sample_creds();
        let pair = creds.to_token_pair();
        assert_eq!(pair.access_token, "access123");
        assert_eq!(pair.refresh_token, "refresh456");
    }

    #[test]
    fn test_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("sub").join("dir");
        let store = LicenseStore::new(nested);
        store.save(&sample_creds()).unwrap();
        assert!(store.exists());
    }

    #[test]
    fn test_file_path() {
        let store = LicenseStore::new(PathBuf::from("/tmp/termesh"));
        assert_eq!(
            store.path(),
            &Path::new("/tmp/termesh/credentials.json").to_path_buf()
        );
    }
}
