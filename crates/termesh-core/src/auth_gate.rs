//! App startup authentication gate.
//!
//! Checks stored credentials at launch and determines whether the user
//! has a valid subscription. Handles token refresh and offline grace periods.

use crate::auth;
use crate::error::AuthError;
use crate::license::{LicenseStore, StoredCredentials};
use serde::{Deserialize, Serialize};

/// Grace period in seconds for offline usage (72 hours).
const OFFLINE_GRACE_SECS: u64 = 72 * 60 * 60;

/// Token refresh margin — refresh if expiring within 5 minutes.
const REFRESH_MARGIN_SECS: u64 = 5 * 60;

/// Result of the startup authentication check.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// User is authenticated with a valid subscription.
    Authenticated { plan: String, email: Option<String> },
    /// Token was verified offline within the grace period.
    OfflineGrace {
        email: Option<String>,
        /// Seconds remaining in the grace period.
        remaining_secs: u64,
    },
    /// User needs to log in.
    NeedsLogin,
    /// Authentication failed with a specific reason.
    Failed(String),
}

/// Metadata stored alongside credentials for offline grace period tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMetadata {
    /// Unix timestamp of last successful online verification.
    pub last_verified: u64,
    /// License plan from last verification.
    pub plan: String,
}

impl AuthMetadata {
    /// Get the path for the metadata file.
    fn path(store: &LicenseStore) -> std::path::PathBuf {
        store.path().with_extension("meta.json")
    }

    /// Save metadata alongside credentials.
    pub fn save(&self, store: &LicenseStore) -> Result<(), AuthError> {
        let path = Self::path(store);
        let json = serde_json::to_string(self).map_err(|e| AuthError::InvalidToken {
            reason: format!("failed to serialize metadata: {e}"),
        })?;
        std::fs::write(&path, json).map_err(|e| AuthError::Network {
            reason: format!("failed to write metadata: {e}"),
        })
    }

    /// Load metadata.
    pub fn load(store: &LicenseStore) -> Option<Self> {
        let path = Self::path(store);
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Delete metadata.
    pub fn delete(store: &LicenseStore) {
        let path = Self::path(store);
        if let Err(e) = std::fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("failed to delete auth metadata: {e}");
            }
        }
    }
}

/// Check authentication state at app startup (non-async, for offline checks).
///
/// This performs local-only checks:
/// 1. Load stored credentials
/// 2. Check token expiry
/// 3. If expired, check offline grace period
///
/// For online verification and token refresh, use `check_auth_online()`.
pub fn check_auth_local(store: &LicenseStore) -> AuthState {
    let creds = match store.load() {
        Ok(c) => c,
        Err(_) => return AuthState::NeedsLogin,
    };

    // Check if the access token is still valid
    match auth::check_token_expiry(&creds.access_token, REFRESH_MARGIN_SECS) {
        Ok(_claims) => {
            // Token looks valid locally — but we haven't verified with server yet
            // Check if we have cached metadata for the plan
            if let Some(meta) = AuthMetadata::load(store) {
                AuthState::Authenticated {
                    plan: meta.plan,
                    email: creds.email.clone(),
                }
            } else {
                // Token valid but no cached plan — needs online verification
                AuthState::Authenticated {
                    plan: "unknown".to_string(),
                    email: creds.email.clone(),
                }
            }
        }
        Err(AuthError::TokenExpired) => {
            // Token expired — check offline grace period
            check_offline_grace(store, &creds)
        }
        Err(_) => {
            // Invalid token format
            AuthState::NeedsLogin
        }
    }
}

/// Check if the offline grace period is still active.
fn check_offline_grace(store: &LicenseStore, creds: &StoredCredentials) -> AuthState {
    let meta = match AuthMetadata::load(store) {
        Some(m) => m,
        None => return AuthState::NeedsLogin,
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let elapsed = now.saturating_sub(meta.last_verified);
    if elapsed < OFFLINE_GRACE_SECS {
        AuthState::OfflineGrace {
            email: creds.email.clone(),
            remaining_secs: OFFLINE_GRACE_SECS - elapsed,
        }
    } else {
        AuthState::NeedsLogin
    }
}

/// Perform online authentication check with token refresh.
///
/// 1. Try to verify the current access token
/// 2. If expired, try to refresh
/// 3. Update stored credentials and metadata on success
#[cfg(feature = "auth")]
pub async fn check_auth_online(store: &LicenseStore, base_url: &str) -> AuthState {
    let creds = match store.load() {
        Ok(c) => c,
        Err(_) => return AuthState::NeedsLogin,
    };

    let client = auth::AuthClient::new(base_url);

    // Try verifying current token
    match client.verify(&creds.access_token).await {
        Ok(status) if status.valid => {
            update_metadata(store, &status);
            return AuthState::Authenticated {
                plan: status.plan,
                email: creds.email,
            };
        }
        Ok(_) => {
            // Token valid but subscription inactive
            return AuthState::Failed("subscription inactive".to_string());
        }
        Err(AuthError::TokenExpired) | Err(AuthError::AuthFailed { .. }) => {
            // Try refresh
        }
        Err(AuthError::Network { .. }) => {
            // Network error — fall back to offline grace
            return check_offline_grace(store, &creds);
        }
        Err(e) => return AuthState::Failed(e.to_string()),
    }

    // Attempt token refresh
    match client.refresh(&creds.refresh_token).await {
        Ok(new_tokens) => {
            let new_creds = StoredCredentials::from(new_tokens)
                .with_email(creds.email.as_deref().unwrap_or(""));
            if let Err(e) = store.save(&new_creds) {
                log::warn!("failed to save refreshed tokens: {e}");
            }

            // Verify the new token
            match client.verify(&new_creds.access_token).await {
                Ok(status) if status.valid => {
                    update_metadata(store, &status);
                    AuthState::Authenticated {
                        plan: status.plan,
                        email: new_creds.email,
                    }
                }
                _ => AuthState::Failed("refresh succeeded but verification failed".to_string()),
            }
        }
        Err(AuthError::Network { .. }) => check_offline_grace(store, &creds),
        Err(e) => AuthState::Failed(format!("token refresh failed: {e}")),
    }
}

/// Update the on-disk metadata after a successful online verification.
#[cfg(feature = "auth")]
fn update_metadata(store: &LicenseStore, status: &auth::LicenseStatus) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let meta = AuthMetadata {
        last_verified: now,
        plan: status.plan.clone(),
    };
    if let Err(e) = meta.save(store) {
        log::warn!("failed to save auth metadata: {e}");
    }
}

/// Clear all authentication state (logout).
pub fn logout(store: &LicenseStore) {
    let _ = store.delete();
    AuthMetadata::delete(store);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (LicenseStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = LicenseStore::new(dir.path().to_path_buf());
        (store, dir)
    }

    #[cfg(feature = "auth")]
    fn make_jwt(exp: u64) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{exp},"sub":"user123"}}"#));
        let sig = URL_SAFE_NO_PAD.encode("fakesig");
        format!("{header}.{payload}.{sig}")
    }

    #[test]
    fn test_no_credentials_needs_login() {
        let (store, _dir) = temp_store();
        assert_eq!(check_auth_local(&store), AuthState::NeedsLogin);
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_valid_token_authenticated() {
        let (store, _dir) = temp_store();

        let creds = StoredCredentials {
            access_token: make_jwt(9999999999),
            refresh_token: "refresh".to_string(),
            email: Some("test@test.com".to_string()),
        };
        store.save(&creds).unwrap();

        let meta = AuthMetadata {
            last_verified: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            plan: "pro".to_string(),
        };
        meta.save(&store).unwrap();

        match check_auth_local(&store) {
            AuthState::Authenticated { plan, email } => {
                assert_eq!(plan, "pro");
                assert_eq!(email.as_deref(), Some("test@test.com"));
            }
            other => panic!("expected Authenticated, got {other:?}"),
        }
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_expired_token_with_grace_period() {
        let (store, _dir) = temp_store();

        let creds = StoredCredentials {
            access_token: make_jwt(1000000000), // long expired
            refresh_token: "refresh".to_string(),
            email: Some("test@test.com".to_string()),
        };
        store.save(&creds).unwrap();

        // Last verified 1 hour ago — within grace period
        let meta = AuthMetadata {
            last_verified: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 3600,
            plan: "pro".to_string(),
        };
        meta.save(&store).unwrap();

        match check_auth_local(&store) {
            AuthState::OfflineGrace { remaining_secs, .. } => {
                assert!(remaining_secs > 0);
            }
            other => panic!("expected OfflineGrace, got {other:?}"),
        }
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_expired_token_grace_period_exceeded() {
        let (store, _dir) = temp_store();

        let creds = StoredCredentials {
            access_token: make_jwt(1000000000),
            refresh_token: "refresh".to_string(),
            email: None,
        };
        store.save(&creds).unwrap();

        // Last verified 100 hours ago — exceeds 72h grace period
        let meta = AuthMetadata {
            last_verified: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - (100 * 3600),
            plan: "pro".to_string(),
        };
        meta.save(&store).unwrap();

        assert_eq!(check_auth_local(&store), AuthState::NeedsLogin);
    }

    #[test]
    fn test_logout_clears_all() {
        let (store, _dir) = temp_store();

        let creds = StoredCredentials {
            access_token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            email: None,
        };
        store.save(&creds).unwrap();

        let meta = AuthMetadata {
            last_verified: 1000,
            plan: "pro".to_string(),
        };
        meta.save(&store).unwrap();

        logout(&store);
        assert!(!store.exists());
        assert!(AuthMetadata::load(&store).is_none());
    }

    #[test]
    fn test_auth_metadata_save_and_load() {
        let (store, _dir) = temp_store();

        let meta = AuthMetadata {
            last_verified: 12345,
            plan: "team".to_string(),
        };
        meta.save(&store).unwrap();

        let loaded = AuthMetadata::load(&store).unwrap();
        assert_eq!(loaded.last_verified, 12345);
        assert_eq!(loaded.plan, "team");
    }
}
