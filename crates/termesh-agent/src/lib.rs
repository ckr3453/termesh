//! Agent adapter and state inference for termesh.
//!
//! Provides tools for detecting and tracking AI agent states
//! from terminal output streams.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use termesh_agent::registry::AdapterRegistry;
//!
//! let registry = AdapterRegistry::with_defaults();
//! if let Some(adapter) = registry.get("claude") {
//!     println!("Adapter: {}", adapter.name());
//! }
//! ```

pub mod adapter;
pub mod claude_code;
pub mod codex_cli;
pub mod gemini_cli;
pub mod preset;
pub mod registry;
pub mod workspace;
