//! Authentication client for subscription-based license validation.
//!
//! Communicates with the termesh auth server to verify user subscriptions.
//! Token storage is handled separately by the `license` module.

use crate::error::AuthError;
use serde::{Deserialize, Serialize};

/// API endpoint paths (appended to base URL).
pub mod endpoints {
    /// POST — authenticate with email/password, returns tokens.
    pub const LOGIN: &str = "/v1/auth/login";
    /// POST — refresh an expired access token using a refresh token.
    pub const REFRESH: &str = "/v1/auth/refresh";
    /// GET — validate the current access token and return license status.
    pub const VERIFY: &str = "/v1/auth/verify";
}

/// Login request body.
#[derive(Debug, Serialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Token pair returned by login/refresh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

/// License status returned by the verify endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LicenseStatus {
    pub valid: bool,
    pub plan: String,
    pub expires_at: Option<String>,
}

/// Refresh request body.
#[derive(Debug, Serialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Decoded JWT claims (minimal subset needed for expiration check).
#[derive(Debug, Deserialize)]
pub struct JwtClaims {
    /// Expiration time (Unix timestamp).
    pub exp: u64,
    /// Subject (user ID).
    pub sub: Option<String>,
}

/// Check if a JWT access token is expired (or will expire within the given margin).
///
/// Returns `Ok(claims)` if the token is still valid, `Err(AuthError::TokenExpired)` if not.
pub fn check_token_expiry(access_token: &str, margin_secs: u64) -> Result<JwtClaims, AuthError> {
    let claims = decode_jwt_claims(access_token)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if claims.exp <= now + margin_secs {
        return Err(AuthError::TokenExpired);
    }
    Ok(claims)
}

/// Decode the payload of a JWT without verifying the signature.
///
/// We only need to read expiration claims client-side. Actual signature
/// verification happens server-side during the verify call.
pub fn decode_jwt_claims(token: &str) -> Result<JwtClaims, AuthError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(AuthError::InvalidToken {
            reason: "JWT must have 3 parts".to_string(),
        });
    }

    #[cfg(feature = "auth")]
    {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let payload_bytes =
            URL_SAFE_NO_PAD
                .decode(parts[1])
                .map_err(|e| AuthError::InvalidToken {
                    reason: format!("base64 decode failed: {e}"),
                })?;

        serde_json::from_slice(&payload_bytes).map_err(|e| AuthError::InvalidToken {
            reason: format!("JSON parse failed: {e}"),
        })
    }

    #[cfg(not(feature = "auth"))]
    {
        Err(AuthError::InvalidToken {
            reason: "auth feature not enabled".to_string(),
        })
    }
}

/// HTTP client for the termesh auth API.
#[cfg(feature = "auth")]
pub struct AuthClient {
    base_url: String,
    client: reqwest::Client,
}

#[cfg(feature = "auth")]
impl AuthClient {
    /// Create a new auth client pointing to the given base URL.
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Authenticate with email and password.
    pub async fn login(&self, email: &str, password: &str) -> Result<TokenPair, AuthError> {
        let url = format!("{}{}", self.base_url, endpoints::LOGIN);
        let resp = self
            .client
            .post(&url)
            .json(&LoginRequest {
                email: email.to_string(),
                password: password.to_string(),
            })
            .send()
            .await
            .map_err(|e| AuthError::Network {
                reason: e.to_string(),
            })?;

        if resp.status().is_success() {
            resp.json::<TokenPair>()
                .await
                .map_err(|e| AuthError::InvalidToken {
                    reason: format!("failed to parse token response: {e}"),
                })
        } else {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            if status == 401 {
                Err(AuthError::AuthFailed { reason: body })
            } else {
                Err(AuthError::ServerError { status, body })
            }
        }
    }

    /// Refresh an expired access token.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenPair, AuthError> {
        let url = format!("{}{}", self.base_url, endpoints::REFRESH);
        let resp = self
            .client
            .post(&url)
            .json(&RefreshRequest {
                refresh_token: refresh_token.to_string(),
            })
            .send()
            .await
            .map_err(|e| AuthError::Network {
                reason: e.to_string(),
            })?;

        if resp.status().is_success() {
            resp.json::<TokenPair>()
                .await
                .map_err(|e| AuthError::InvalidToken {
                    reason: format!("failed to parse refresh response: {e}"),
                })
        } else {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(AuthError::ServerError { status, body })
        }
    }

    /// Verify the current access token and get license status.
    pub async fn verify(&self, access_token: &str) -> Result<LicenseStatus, AuthError> {
        let url = format!("{}{}", self.base_url, endpoints::VERIFY);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AuthError::Network {
                reason: e.to_string(),
            })?;

        if resp.status().is_success() {
            resp.json::<LicenseStatus>()
                .await
                .map_err(|e| AuthError::InvalidToken {
                    reason: format!("failed to parse verify response: {e}"),
                })
        } else {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            if status == 401 {
                Err(AuthError::TokenExpired)
            } else {
                Err(AuthError::ServerError { status, body })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_jwt(exp: u64) -> String {
        #[cfg(feature = "auth")]
        {
            use base64::engine::general_purpose::URL_SAFE_NO_PAD;
            use base64::Engine;

            let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
            let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{exp},"sub":"user123"}}"#));
            let sig = URL_SAFE_NO_PAD.encode("fakesig");
            format!("{header}.{payload}.{sig}")
        }
        #[cfg(not(feature = "auth"))]
        {
            let _ = exp;
            String::new()
        }
    }

    #[test]
    fn test_decode_jwt_claims() {
        let token = make_jwt(9999999999);
        let claims = decode_jwt_claims(&token);
        #[cfg(feature = "auth")]
        {
            let claims = claims.unwrap();
            assert_eq!(claims.exp, 9999999999);
            assert_eq!(claims.sub.as_deref(), Some("user123"));
        }
        #[cfg(not(feature = "auth"))]
        {
            assert!(claims.is_err());
        }
    }

    #[test]
    fn test_invalid_jwt_format() {
        let result = decode_jwt_claims("not.a.valid.jwt");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_jwt_two_parts() {
        let result = decode_jwt_claims("only.two");
        assert!(result.is_err());
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_check_token_not_expired() {
        let token = make_jwt(9999999999);
        let result = check_token_expiry(&token, 60);
        assert!(result.is_ok());
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_check_token_expired() {
        let token = make_jwt(1000000000); // 2001 — long expired
        let result = check_token_expiry(&token, 0);
        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }

    #[test]
    fn test_endpoints() {
        assert_eq!(endpoints::LOGIN, "/v1/auth/login");
        assert_eq!(endpoints::REFRESH, "/v1/auth/refresh");
        assert_eq!(endpoints::VERIFY, "/v1/auth/verify");
    }

    #[test]
    fn test_license_status_serde() {
        let status = LicenseStatus {
            valid: true,
            plan: "pro".to_string(),
            expires_at: Some("2026-12-31".to_string()),
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: LicenseStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn test_token_pair_serde() {
        let pair = TokenPair {
            access_token: "abc".to_string(),
            refresh_token: "def".to_string(),
        };
        let json = serde_json::to_string(&pair).unwrap();
        let parsed: TokenPair = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "abc");
        assert_eq!(parsed.refresh_token, "def");
    }

    #[test]
    fn test_auth_error_display() {
        let err = AuthError::Network {
            reason: "connection refused".to_string(),
        };
        assert!(err.to_string().contains("network error"));

        let err = AuthError::TokenExpired;
        assert!(err.to_string().contains("token expired"));
    }
}
