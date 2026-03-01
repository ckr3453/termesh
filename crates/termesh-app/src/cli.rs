//! Command-line interface definition.

use clap::{Parser, Subcommand};

/// Termesh: AI agent control tower + universal terminal.
#[derive(Parser, Debug)]
#[command(name = "termesh", version, about)]
pub struct Cli {
    /// Start directly with a specific agent (claude, codex, gemini, shell).
    #[arg(long)]
    pub agent: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Open a workspace preset.
    Open {
        /// Workspace preset name (without .toml extension).
        name: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_parse_no_args() {
        let cli = Cli::parse_from(["termesh"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_parse_open() {
        let cli = Cli::parse_from(["termesh", "open", "fullstack"]);
        match cli.command {
            Some(Command::Open { name }) => assert_eq!(name, "fullstack"),
            _ => panic!("expected Open command"),
        }
    }

    #[test]
    fn test_parse_agent_flag() {
        let cli = Cli::parse_from(["termesh", "--agent", "claude"]);
        assert_eq!(cli.agent.as_deref(), Some("claude"));
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_parse_version() {
        let result = Cli::try_parse_from(["termesh", "--version"]);
        // --version causes early exit, so it returns Err with DisplayVersion
        assert!(result.is_err());
    }
}
