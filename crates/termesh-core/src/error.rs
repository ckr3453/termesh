//! Custom error types for termesh.

use std::path::PathBuf;

/// Top-level error type for termesh.
#[derive(Debug, thiserror::Error)]
pub enum TermeshError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("pty error: {0}")]
    Pty(#[from] PtyError),

    #[error("render error: {0}")]
    Render(#[from] RenderError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors related to configuration loading and parsing.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config file not found: {path}")]
    NotFound { path: PathBuf },

    #[error("failed to parse config: {source}")]
    Parse {
        #[source]
        source: toml::de::Error,
    },

    #[error("invalid config value: {field} — {reason}")]
    InvalidValue { field: String, reason: String },
}

/// Errors related to PTY operations.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("failed to spawn PTY process: {reason}")]
    SpawnFailed { reason: String },

    #[error("session not found: {0}")]
    SessionNotFound(crate::types::SessionId),

    #[error("PTY I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors related to authentication.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("network error: {reason}")]
    Network { reason: String },

    #[error("authentication failed: {reason}")]
    AuthFailed { reason: String },

    #[error("token expired")]
    TokenExpired,

    #[error("invalid token: {reason}")]
    InvalidToken { reason: String },

    #[error("server error: {status} — {body}")]
    ServerError { status: u16, body: String },
}

/// Errors related to GPU rendering.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("failed to initialize GPU: {reason}")]
    GpuInitFailed { reason: String },

    #[error("shader compilation failed: {reason}")]
    ShaderError { reason: String },

    #[error("font loading failed: {path}")]
    FontLoadFailed { path: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::NotFound {
            path: PathBuf::from("/home/user/.config/termesh/config.toml"),
        };
        assert!(err.to_string().contains("config file not found"));
    }

    #[test]
    fn test_pty_error_display() {
        let err = PtyError::SpawnFailed {
            reason: "invalid shell path".to_string(),
        };
        assert!(err.to_string().contains("failed to spawn PTY process"));
    }

    #[test]
    fn test_render_error_display() {
        let err = RenderError::GpuInitFailed {
            reason: "no compatible adapter".to_string(),
        };
        assert!(err.to_string().contains("failed to initialize GPU"));
    }

    #[test]
    fn test_termesh_error_from_config() {
        let config_err = ConfigError::InvalidValue {
            field: "font_size".to_string(),
            reason: "must be between 8 and 72".to_string(),
        };
        let err = TermeshError::from(config_err);
        assert!(err.to_string().contains("config error"));
    }
}
